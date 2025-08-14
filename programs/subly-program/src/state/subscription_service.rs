use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct SubscriptionService {
    pub provider: Pubkey,
    pub service_id: u64,
    #[max_len(64)]
    pub name: String,
    #[max_len(200)]
    pub description: String,
    pub fee_usd: u64, // USD cents
    pub billing_frequency_days: u64,
    #[max_len(200)]
    pub image_url: String,
    pub current_subscribers: u64,
    pub is_active: bool,
    pub created_at: i64,
    pub bumps: u8,
}
