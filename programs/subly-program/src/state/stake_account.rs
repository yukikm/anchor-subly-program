use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct StakeAccount {
    pub user: Pubkey,
    pub staked_amount: u64,   // In lamports
    pub jito_sol_amount: u64, // JitoSOL received
    pub stake_date: i64,
    pub last_yield_claim: i64,
    pub total_yield_earned: u64,
    pub is_active: bool,
    pub bump: u8,
}
