use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token::Token};

#[derive(Accounts)]
#[instruction(name: String, description: String)]
pub struct RegisterSubscriptionService<'info> {
    #[account(mut)]
    pub provider: Signer<'info>,

    #[account(
        mut,
        seeds = [PROVIDER_SEED.as_bytes(), provider.key().as_ref()],
        bump = provider_account.bump,
        constraint = provider_account.wallet == provider.key() @ ErrorCode::UnauthorizedProvider
    )]
    pub provider_account: Account<'info, Provider>,

    #[account(
        mut,
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        init,
        payer = provider,
        space = SubscriptionService::INIT_SPACE,
        seeds = [
            SUBSCRIPTION_SERVICE_SEED.as_bytes(),
            provider.key().as_ref(),
            global_state.total_services.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub subscription_service: Account<'info, SubscriptionService>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> RegisterSubscriptionService<'info> {
    pub fn register_subscription_service(
        &mut self,
        name: String,
        description: String,
        fee_usd: u64,
        billing_frequency_days: u64,
        image_url: String,
        bumps: &RegisterSubscriptionServiceBumps,
    ) -> Result<()> {
        require!(!self.global_state.is_paused, ErrorCode::ProtocolPaused);
        require!(name.len() <= MAX_NAME_LENGTH, ErrorCode::NameTooLong);
        require!(
            description.len() <= MAX_DESCRIPTION_LENGTH,
            ErrorCode::DescriptionTooLong
        );
        require!(image_url.len() <= MAX_URL_LENGTH, ErrorCode::UrlTooLong);
        require!(fee_usd > 0, ErrorCode::InvalidFeeAmount);
        require!(
            billing_frequency_days >= MIN_SUBSCRIPTION_PERIOD_DAYS
                && billing_frequency_days <= MAX_SUBSCRIPTION_PERIOD_DAYS,
            ErrorCode::InvalidBillingFrequency
        );

        let provider_account = &mut self.provider_account;
        let global_state = &mut self.global_state;

        self.subscription_service.set_inner(SubscriptionService {
            provider: self.provider.key(),
            service_id: global_state.total_services,
            name: name.clone(),
            description,
            fee_usd,
            billing_frequency_days,
            image_url,
            current_subscribers: 0,
            is_active: true,
            created_at: Clock::get()?.unix_timestamp,
            bumps: bumps.subscription_service,
        });

        // Update global service count
        global_state.total_services += 1;

        msg!(
            "Subscription service '{}' registered by provider {} with fee ${:.2} per {} days",
            name,
            self.provider.key(),
            fee_usd as f64 / 100.0,
            billing_frequency_days
        );

        Ok(())
    }
}
