use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};
use pyth_sdk_solana::state::SolanaPriceAccount;

#[derive(Accounts)]
#[instruction(provider: Pubkey, service_id: u64)]
pub struct SubscribeToService<'info> {
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
            SUBSCRIPTION_SERVICE_SEED.as_bytes(),
            provider.as_ref(),
            service_id.to_le_bytes().as_ref()
        ],
        bump,
        constraint = subscription_service.is_active @ ErrorCode::ServiceNotActive,
        constraint = subscription_service.provider != user.key() @ ErrorCode::CannotSubscribeToOwnService
    )]
    pub subscription_service: Account<'info, SubscriptionService>,

    #[account(
        mut,
        seeds = [PROVIDER_SEED.as_bytes(), provider.as_ref()],
        bump
    )]
    pub provider_account: Account<'info, Provider>,

    #[account(
        init,
        payer = user,
        space = UserSubscription::INIT_SPACE,
        seeds = [
            USER_SUBSCRIPTION_SEED.as_bytes(),
            user.key().as_ref(),
            provider.as_ref(),
            service_id.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub user_subscription: Account<'info, UserSubscription>,

    #[account(
        mut,
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,

    /// Pyth SOL/USD price feed account
    /// CHECK: Pyth price feed account
    pub sol_usd_price_feed: AccountInfo<'info>,

    // Subscription certificate NFT
    #[account(
        init,
        payer = user,
        mint::decimals = 0,
        mint::authority = user,
        mint::freeze_authority = user,
    )]
    pub certificate_nft_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = user,
        associated_token::mint = certificate_nft_mint,
        associated_token::authority = user,
    )]
    pub certificate_nft_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> SubscribeToService<'info> {
    pub fn subscribe_to_service(
        &mut self,
        provider: Pubkey,
        service_id: u64,
        bumps: &SubscribeToServiceBumps,
    ) -> Result<()> {
        require!(!self.global_state.is_paused, ErrorCode::ProtocolPaused);

        // Verify the Pyth price feed account matches the one in GlobalState
        require!(
            self.sol_usd_price_feed.key() == self.global_state.sol_usd_price_feed,
            ErrorCode::InvalidPriceFeed
        );

        let subscription_service = &mut self.subscription_service;
        let user_account = &mut self.user_account;
        let provider_account = &mut self.provider_account;

        // Get real SOL/USD price from Pyth
        let sol_usd_price_cents = Self::get_sol_usd_price_from_pyth(&self.sol_usd_price_feed)?;

        // Calculate required locked amount (12 months of subscription fees) using real price
        let monthly_fee_lamports =
            Self::convert_usd_to_sol_lamports(subscription_service.fee_usd, sol_usd_price_cents)?;
        let required_locked_amount = monthly_fee_lamports * 12; // Lock 12 months worth

        // Check if user has sufficient available balance
        let available_balance = user_account
            .deposited_sol
            .checked_sub(user_account.locked_sol)
            .unwrap_or(0);

        require!(
            available_balance >= required_locked_amount,
            ErrorCode::InsufficientAvailableBalance
        );

        let current_time = Clock::get()?.unix_timestamp;
        let next_payment_due =
            current_time + (subscription_service.billing_frequency_days as i64 * 86400);

        // Create subscription
        self.user_subscription.set_inner(UserSubscription {
            user: self.user.key(),
            provider,
            service_id,
            subscription_id: service_id, // Use service_id as subscription_id for simplicity
            subscribed_at: current_time,
            last_payment_at: None,
            next_payment_due,
            total_payments_made: 0,
            is_active: true,
            unsubscribed_at: None,
            bumps: bumps.user_subscription,
        });

        // Lock funds for subscription
        user_account.locked_sol = user_account
            .locked_sol
            .checked_add(required_locked_amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Mint subscription certificate NFT
        let cpi_accounts = MintTo {
            mint: self.certificate_nft_mint.to_account_info(),
            to: self.certificate_nft_token_account.to_account_info(),
            authority: self.user.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        mint_to(cpi_ctx, 1)?;

        // Update counters
        subscription_service.current_subscribers += 1;
        provider_account.total_subscribers += 1;

        msg!(
            "User {} subscribed to service '{}' from provider {} (Fee: ${:.2}/{} days)",
            self.user.key(),
            subscription_service.name,
            provider,
            subscription_service.fee_usd as f64 / 100.0,
            subscription_service.billing_frequency_days
        );

        msg!(
            "Subscription certificate NFT minted: {}",
            self.certificate_nft_mint.key()
        );

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
    fn convert_usd_to_sol_lamports(usd_cents: u64, sol_usd_cents: u64) -> Result<u64> {
        // lamports = (usd_cents * LAMPORTS_PER_SOL) / sol_usd_cents
        let lamports = (usd_cents as u128)
            .checked_mul(1_000_000_000) // LAMPORTS_PER_SOL
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(sol_usd_cents as u128)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        Ok(u64::try_from(lamports).map_err(|_| ErrorCode::ArithmeticOverflow)?)
    }
}
