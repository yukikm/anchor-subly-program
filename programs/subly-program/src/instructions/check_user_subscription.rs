use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(provider: Pubkey, service_id: u64)]
pub struct CheckUserSubscription<'info> {
    /// The user whose subscription status we're checking
    pub user: Signer<'info>,

    #[account(
        seeds = [USER_SEED.as_bytes(), user.key().as_ref()],
        bump = user_account.bump,
        constraint = user_account.wallet == user.key() @ ErrorCode::UnauthorizedUser
    )]
    pub user_account: Account<'info, User>,

    /// User's subscription account (optional - may not exist if user never subscribed)
    #[account(
        seeds = [
            USER_SUBSCRIPTION_SEED.as_bytes(),
            user.key().as_ref(),
            provider.as_ref(),
            &service_id.to_le_bytes(),
        ],
        bump,
    )]
    pub user_subscription: Option<Account<'info, UserSubscription>>,
}

impl<'info> CheckUserSubscription<'info> {
    pub fn check_user_subscription(&self, provider: Pubkey, service_id: u64) -> Result<bool> {
        // Check if user has a subscription account for this provider and service
        if let Some(subscription) = &self.user_subscription {
            // Verify the subscription matches the requested provider and service
            require!(
                subscription.provider == provider,
                ErrorCode::InvalidProvider
            );
            require!(
                subscription.service_id == service_id,
                ErrorCode::InvalidServiceId
            );
            require!(
                subscription.user == self.user.key(),
                ErrorCode::UnauthorizedUser
            );

            // Check if subscription is active
            let is_active = subscription.is_active;
            
            msg!(
                "User {} subscription to provider {} service {}: {}",
                self.user.key(),
                provider,
                service_id,
                if is_active { "ACTIVE" } else { "INACTIVE" }
            );

            Ok(is_active)
        } else {
            // No subscription account exists
            msg!(
                "User {} has NO subscription to provider {} service {}",
                self.user.key(),
                provider,
                service_id
            );
            Ok(false)
        }
    }
}
