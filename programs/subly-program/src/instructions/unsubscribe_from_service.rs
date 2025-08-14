use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{burn, Burn, Mint, Token, TokenAccount},
};
use pyth_sdk_solana::state::SolanaPriceAccount;

#[derive(Accounts)]
#[instruction(provider: Pubkey, service_id: u64)]
pub struct UnsubscribeFromService<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [USER_SEED.as_bytes(), user.key().as_ref()],
        bump = user_account.bump,
        constraint = user_account.wallet == user.key() @ ErrorCode::UnauthorizedUser
    )]
    pub user_account: Account<'info, User>,

    #[account(
        mut,
        seeds = [
            USER_SUBSCRIPTION_SEED.as_bytes(),
            user.key().as_ref(),
            provider.as_ref(),
            service_id.to_le_bytes().as_ref()
        ],
        bump = user_subscription.bumps,
        constraint = user_subscription.user == user.key() @ ErrorCode::UnauthorizedUser,
        constraint = user_subscription.provider == provider @ ErrorCode::InvalidProvider,
        constraint = user_subscription.service_id == service_id @ ErrorCode::InvalidServiceId
    )]
    pub user_subscription: Account<'info, UserSubscription>,

    #[account(
        mut,
        seeds = [
            SUBSCRIPTION_SERVICE_SEED.as_bytes(),
            provider.as_ref(),
            service_id.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub subscription_service: Account<'info, SubscriptionService>,

    #[account(
        mut,
        seeds = [PROVIDER_SEED.as_bytes(), provider.as_ref()],
        bump
    )]
    pub provider_account: Account<'info, Provider>,

    #[account(
        mut,
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,

    // Pyth price feed for SOL/USD conversion
    /// CHECK: This account is validated in the instruction method to match the price feed in GlobalState
    pub sol_usd_price_feed: AccountInfo<'info>,

    // Subscription certificate NFT to burn
    #[account(mut)]
    pub certificate_nft_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = certificate_nft_mint,
        associated_token::authority = user,
        constraint = certificate_nft_token_account.amount > 0 @ ErrorCode::NoCertificateToDestroy
    )]
    pub certificate_nft_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> UnsubscribeFromService<'info> {
    pub fn unsubscribe_from_service(&mut self, _provider: Pubkey, _service_id: u64) -> Result<()> {
        require!(!self.global_state.is_paused, ErrorCode::ProtocolPaused);

        // Verify the Pyth price feed account matches the one in GlobalState
        require!(
            self.sol_usd_price_feed.key() == self.global_state.sol_usd_price_feed,
            ErrorCode::InvalidPriceFeed
        );

        let user_subscription = &mut self.user_subscription;
        let user_account = &mut self.user_account;
        let subscription_service = &mut self.subscription_service;
        let provider_account = &mut self.provider_account;

        // Validate that the subscription is active
        require!(
            user_subscription.is_active,
            ErrorCode::SubscriptionNotActive
        );

        // Calculate the amount to unlock (remaining subscription fees)
        let current_time = Clock::get()?.unix_timestamp;
        let subscription_days = subscription_service.billing_frequency_days;
        let seconds_per_day = 86400i64;
        let billing_period_seconds = subscription_days as i64 * seconds_per_day;

        // Calculate how much time is left in current billing period
        let time_since_subscription = current_time - user_subscription.subscribed_at;
        let _full_periods_passed = time_since_subscription / billing_period_seconds;
        let time_in_current_period = time_since_subscription % billing_period_seconds;
        let _remaining_time_in_period = billing_period_seconds - time_in_current_period;

        // Get real SOL/USD price from Pyth
        let sol_usd_price_cents = Self::get_sol_usd_price_from_pyth(&self.sol_usd_price_feed)?;

        // Calculate monthly fee in lamports using real Pyth price
        let monthly_fee_lamports =
            Self::convert_usd_to_sol_lamports(subscription_service.fee_usd, sol_usd_price_cents)?;

        // Calculate how much SOL to unlock
        // Unlock all remaining locked funds for this subscription (since user is canceling)
        let locked_amount_for_subscription = monthly_fee_lamports
            .checked_mul(12)
            .ok_or(ErrorCode::ArithmeticOverflow)?; // We locked 12 months initially

        // Free up locked SOL
        user_account.locked_sol = user_account
            .locked_sol
            .checked_sub(locked_amount_for_subscription)
            .unwrap_or(0);

        // Burn the subscription certificate NFT
        let cpi_accounts = Burn {
            mint: self.certificate_nft_mint.to_account_info(),
            from: self.certificate_nft_token_account.to_account_info(),
            authority: self.user.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        burn(cpi_ctx, 1)?;

        // Deactivate subscription
        user_subscription.is_active = false;
        user_subscription.unsubscribed_at = Some(current_time);

        // Update counters
        subscription_service.current_subscribers =
            subscription_service.current_subscribers.saturating_sub(1);
        provider_account.total_subscribers = provider_account.total_subscribers.saturating_sub(1);

        msg!(
            "User {} successfully unsubscribed from service '{}' (Provider: {})",
            self.user.key(),
            subscription_service.name,
            user_subscription.provider
        );

        msg!(
            "Unlocked {} lamports from subscription. Certificate NFT burned: {}",
            locked_amount_for_subscription,
            self.certificate_nft_mint.key()
        );

        // Check if less than one month has passed since last payment for prorated access
        if let Some(last_payment) = user_subscription.last_payment_at {
            let time_since_payment = current_time - last_payment;
            if time_since_payment < billing_period_seconds {
                msg!(
                    "User retains access until next billing cycle ({} days remaining)",
                    (billing_period_seconds - time_since_payment) / seconds_per_day
                );
            }
        } else {
            // First billing period - user retains access until next payment would be due
            let time_until_next_payment = user_subscription.next_payment_due - current_time;
            if time_until_next_payment > 0 {
                msg!(
                    "User retains access until next billing cycle ({} days remaining)",
                    time_until_next_payment / seconds_per_day
                );
            }
        }

        Ok(())
    }

    /// Get SOL/USD price from Pyth Network - REAL IMPLEMENTATION
    fn get_sol_usd_price_from_pyth(price_feed_account: &AccountInfo) -> Result<u64> {
        // Load price feed from Pyth account using the correct API
        let price_feed = SolanaPriceAccount::account_info_to_feed(price_feed_account)
            .map_err(|_| ErrorCode::InvalidPriceFeed)?;

        // Get current price with staleness check
        let current_time = Clock::get()?.unix_timestamp;
        let max_age = 3600; // 1 hour in seconds

        let price = price_feed
            .get_price_no_older_than(current_time, max_age)
            .ok_or(ErrorCode::PriceNotAvailable)?;

        // Validate price data
        require!(price.price > 0, ErrorCode::InvalidPrice);

        msg!(
            "Pyth price data: price={}, conf={}, expo={}, timestamp={}",
            price.price,
            price.conf,
            price.expo,
            price.publish_time
        );

        // Convert price to USD cents
        // Pyth SOL/USD typically has exponent of -8, meaning price is in units of 10^-8 USD
        let price_cents = if price.expo >= 0 {
            // Positive exponent: multiply
            (price.price as u64)
                .checked_mul(10_u64.pow(price.expo as u32))
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_mul(100) // Convert to cents
                .ok_or(ErrorCode::ArithmeticOverflow)?
        } else {
            // Negative exponent: divide
            let divisor = 10_u64.pow((-price.expo) as u32);
            (price.price as u64)
                .checked_mul(100) // Convert to cents first
                .ok_or(ErrorCode::ArithmeticOverflow)?
                .checked_div(divisor)
                .ok_or(ErrorCode::ArithmeticOverflow)?
        };

        // Validate reasonable price range ($10 - $1000 per SOL)
        require!(
            price_cents >= 1000 && price_cents <= 100000,
            ErrorCode::InvalidPrice
        );

        msg!(
            "Real SOL/USD price from Pyth: ${:.2} (account: {})",
            price_cents as f64 / 100.0,
            price_feed_account.key()
        );

        Ok(price_cents)
    }

    /// Convert USD cents to SOL lamports using real Pyth price
    fn convert_usd_to_sol_lamports(usd_cents: u64, sol_usd_price_cents: u64) -> Result<u64> {
        // Calculate SOL lamports needed for the USD amount
        // usd_cents / (sol_usd_price_cents / 100) * LAMPORTS_PER_SOL
        // = (usd_cents * 100 * LAMPORTS_PER_SOL) / sol_usd_price_cents

        const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

        let numerator = usd_cents
            .checked_mul(100)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_mul(LAMPORTS_PER_SOL)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        let sol_lamports = numerator
            .checked_div(sol_usd_price_cents)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        msg!(
            "Converting ${:.2} to {} lamports at ${:.2}/SOL",
            usd_cents as f64 / 100.0,
            sol_lamports,
            sol_usd_price_cents as f64 / 100.0
        );

        Ok(sol_lamports)
    }
}
