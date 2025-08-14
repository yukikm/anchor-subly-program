use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum PaymentType {
    Subscription,
    ProtocolFee,
}

impl anchor_lang::Space for PaymentType {
    const INIT_SPACE: usize = 1; // 1 byte for enum discriminator
}

#[account]
#[derive(InitSpace)]
pub struct PaymentRecord {
    pub user: Pubkey,
    pub provider: Pubkey,
    pub subscription_id: u64,
    pub amount: u64, // In lamports
    pub payment_date: i64,
    pub payment_type: PaymentType,
    pub bump: u8,
}
