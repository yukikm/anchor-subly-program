use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct User {
    pub wallet: Pubkey,
    pub deposited_sol: u64, // lamports
    pub locked_sol: u64,    // lamports locked for active subscriptions
    pub staked_sol: u64,    // lamports staked for yield generation
    pub created_at: i64,
    pub bump: u8,
}
