use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::prelude::*;
use pyth_sdk_solana::state::SolanaPriceAccount;

#[derive(Accounts)]
pub struct CheckSubscribableServices<'info> {
    pub user: Signer<'info>,

    #[account(
        seeds = [USER_SEED.as_bytes(), user.key().as_ref()],
        bump = user_account.bump,
        constraint = user_account.wallet == user.key() @ ErrorCode::UnauthorizedUser
    )]
    pub user_account: Account<'info, User>,

    /// Global state for reading Jito configuration and Pyth price feed
    #[account(
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,

    /// Pyth SOL/USD price feed account
    /// CHECK: Pyth price feed account
    pub sol_usd_price_feed: AccountInfo<'info>,

    /// Jito stake pool account for fetching real APY data
    /// CHECK: Jito stake pool account
    pub jito_stake_pool: AccountInfo<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SubscribableServiceInfo {
    pub provider: Pubkey,
    pub service_id: u64,
    pub name: String,
    pub description: String,
    pub fee_usd: u64,
    pub billing_frequency_days: u64,
    pub monthly_fee_sol: u64, // Calculated monthly fee in SOL lamports
    pub can_afford: bool,
}

impl<'info> CheckSubscribableServices<'info> {
    pub fn check_subscribable_services(
        ctx: Context<'_, '_, '_, 'info, CheckSubscribableServices<'info>>,
        jito_apy_bps: u16, // Jito APY in basis points (e.g., 700 = 7%)
    ) -> Result<Vec<SubscribableServiceInfo>> {
        let user_account = &ctx.accounts.user_account;
        let global_state = &ctx.accounts.global_state;

        // Verify the Pyth price feed account matches the one in GlobalState
        require!(
            ctx.accounts.sol_usd_price_feed.key() == global_state.sol_usd_price_feed,
            ErrorCode::InvalidPriceFeed
        );

        // Verify the Jito stake pool account matches the one in GlobalState
        require!(
            ctx.accounts.jito_stake_pool.key() == global_state.jito_stake_pool,
            ErrorCode::InvalidJitoStakePool
        );

        // Get user's deposited lamports (available for staking)
        let deposited_lamports = user_account
            .deposited_sol
            .checked_sub(user_account.locked_sol)
            .unwrap_or(0);

        msg!("User deposited SOL (available): {} lamports", deposited_lamports);

        // Step 1: Calculate expected yield per month from Jito staking
        let expected_yield_per_month = Self::calculate_expected_monthly_yield(
            deposited_lamports,
            jito_apy_bps,
        )?;

        msg!("Expected monthly yield: {} lamports", expected_yield_per_month);

        // Step 2: Get SOL/USD price from Pyth
        let sol_usd_price = Self::get_sol_usd_price_from_pyth(&ctx.accounts.sol_usd_price_feed)?;
        msg!("SOL/USD price from Pyth: ${:.2}", sol_usd_price as f64 / 100.0);

        // Step 3: Process subscription service PDAs from remaining accounts
        let mut affordable_services = Vec::new();
        
        for account_info in ctx.remaining_accounts {
            // Deserialize account data directly
            let mut data = &account_info.data.borrow()[8..]; // Skip discriminator
            let service_account = SubscriptionService::try_deserialize(&mut data)?;
            
            // Skip inactive services
            if !service_account.is_active {
                continue;
            }

            // Convert USD fee to SOL lamports using real Pyth price
            let monthly_fee_sol = Self::convert_usd_to_sol_lamports(
                service_account.fee_usd, 
                sol_usd_price
            )?;
            
            // Check if user can afford this service with expected yield
            let can_afford = expected_yield_per_month >= monthly_fee_sol;

            let service_info = SubscribableServiceInfo {
                provider: service_account.provider,
                service_id: service_account.service_id,
                name: service_account.name.clone(),
                description: service_account.description.clone(),
                fee_usd: service_account.fee_usd,
                billing_frequency_days: service_account.billing_frequency_days,
                monthly_fee_sol,
                can_afford,
            };

            affordable_services.push(service_info);

            msg!(
                "Service: {}, Fee: ${:.2}, Monthly SOL: {} lamports, Affordable: {}",
                service_account.name,
                service_account.fee_usd as f64 / 100.0,
                monthly_fee_sol,
                can_afford
            );
        }
        
        msg!("Processed {} subscription service PDAs", ctx.remaining_accounts.len());

        // Sort by affordability (affordable services first), then by price (cheaper first)
        affordable_services.sort_by(|a, b| {
            match (a.can_afford, b.can_afford) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.fee_usd.cmp(&b.fee_usd), // Same affordability, sort by price
            }
        });

        msg!(
            "Found {} services that can be covered by expected yield",
            affordable_services.iter().filter(|s| s.can_afford).count()
        );

        Ok(affordable_services)
    }

    /// Calculate expected yield per month from Jito staking
    /// Uses provided APY instead of reading from stake pool
    fn calculate_expected_monthly_yield(
        deposited_lamports: u64,
        jito_apy_bps: u16, // Jito APY in basis points received as parameter
    ) -> Result<u64> {
        msg!("Using provided Jito APY: {}bps ({}%)", jito_apy_bps, jito_apy_bps as f64 / 100.0);
        
        // Calculate annual yield
        let annual_yield = deposited_lamports
            .checked_mul(jito_apy_bps as u64)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(10000) // Convert basis points to percentage
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        // Convert to monthly yield
        let monthly_yield = annual_yield
            .checked_div(12)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        Ok(monthly_yield)
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

    /// Convert USD cents to SOL lamports
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
