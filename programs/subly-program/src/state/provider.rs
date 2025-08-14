use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Provider {
    pub wallet: Pubkey,
    #[max_len(64)]
    pub name: String,
    #[max_len(200)]
    pub description: String,
    pub total_subscribers: u64,
    pub is_verified: bool,
    pub created_at: i64,
    pub bump: u8,
}
