use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserSubscription {
    pub user: Pubkey,
    pub provider: Pubkey,
    pub service_id: u64,
    pub subscription_id: u64, // User's subscription ID
    pub subscribed_at: i64,
    pub last_payment_at: Option<i64>,
    pub next_payment_due: i64,
    pub total_payments_made: u64,
    pub is_active: bool,
    pub unsubscribed_at: Option<i64>,
    pub bumps: u8,
}
