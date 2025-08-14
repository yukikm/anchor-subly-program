pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("9MV6eJ5CfimYDv4WSqtyPx1Uc36apP1dzTMpGrobYCnc");

#[program]
pub mod subly_program {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        jito_stake_pool: Pubkey,
        jito_sol_mint: Pubkey,
        spl_stake_pool_program: Pubkey,
        sol_usd_price_feed: Pubkey,
        usdc_mint: Pubkey,
    ) -> Result<()> {
        ctx.accounts.initialize_global_state(
            jito_stake_pool,
            jito_sol_mint,
            spl_stake_pool_program,
            sol_usd_price_feed,
            usdc_mint,
            &ctx.bumps,
        )
    }

    pub fn check_subscribable_services<'info>(
        ctx: Context<'_, '_, '_, 'info, CheckSubscribableServices<'info>>,
        jito_apy_bps: u16, // Jito APY in basis points (e.g., 700 = 7%)
    ) -> Result<Vec<SubscribableServiceInfo>> {
        CheckSubscribableServices::check_subscribable_services(ctx, jito_apy_bps)
    }

    pub fn check_user_subscription(
        ctx: Context<CheckUserSubscription>,
        provider: Pubkey,
        service_id: u64,
    ) -> Result<bool> {
        ctx.accounts.check_user_subscription(provider, service_id)
    }

    pub fn register_provider(
        ctx: Context<RegisterProvider>,
        name: String,
        description: String,
    ) -> Result<()> {
        ctx.accounts
            .register_provider(name, description, &ctx.bumps)
    }

    pub fn register_subscription_service(
        ctx: Context<RegisterSubscriptionService>,
        name: String,
        description: String,
        fee_usd: u64,
        billing_frequency_days: u64,
        image_url: String,
    ) -> Result<()> {
        ctx.accounts.register_subscription_service(
            name,
            description,
            fee_usd,
            billing_frequency_days,
            image_url,
            &ctx.bumps,
        )
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        ctx.accounts.deposit(amount, &ctx.bumps)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64, jito_apy_bps: u16) -> Result<()> {
        // Sequential: unstake_sol then withdraw
        ctx.accounts
            .unstake_sol_if_needed(amount, jito_apy_bps, &ctx.bumps)?;
        ctx.accounts.withdraw(amount, jito_apy_bps, &ctx.bumps)
    }

    pub fn subscribe_to_service(
        ctx: Context<SubscribeToService>,
        provider: Pubkey,
        service_id: u64,
    ) -> Result<()> {
        ctx.accounts
            .subscribe_to_service(provider, service_id, &ctx.bumps)
    }

    pub fn unsubscribe_from_service(
        ctx: Context<UnsubscribeFromService>,
        provider: Pubkey,
        service_id: u64,
    ) -> Result<()> {
        ctx.accounts.unsubscribe_from_service(provider, service_id)
    }

    pub fn process_subscription_payments(ctx: Context<ProcessSubscriptionPayments>) -> Result<()> {
        ctx.accounts.process_subscription_payments()
    }

    pub fn execute_subscription_payment(
        ctx: Context<ExecuteSubscriptionPayment>,
        _user: Pubkey,
        _provider: Pubkey,
        _service_id: u64,
    ) -> Result<()> {
        ctx.accounts.execute_payment(&ctx.bumps)
    }

    pub fn create_payment_record(
        ctx: Context<CreatePaymentRecord>,
        amount: u64,
    ) -> Result<()> {
        ctx.accounts.create_payment_record(amount)
    }

    pub fn stake_sol(ctx: Context<StakeSol>, amount: u64) -> Result<()> {
        ctx.accounts.stake_sol(amount, &ctx.bumps)
    }

    pub fn unstake_sol(
        ctx: Context<UnstakeSol>,
        jito_sol_amount: u64,
        jito_apy_bps: u16,
    ) -> Result<()> {
        ctx.accounts
            .unstake_sol(jito_sol_amount, jito_apy_bps, &ctx.bumps)
    }

    pub fn claim_yield(ctx: Context<ClaimYield>) -> Result<()> {
        ctx.accounts.claim_yield(&ctx.bumps)
    }
}
