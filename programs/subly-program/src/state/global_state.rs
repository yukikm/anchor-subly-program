use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GlobalState {
    pub authority: Pubkey,
    pub protocol_fee_bps: u16, // Basis points (100 = 1%)
    pub is_paused: bool,
    // Jito configuration - can be changed for different networks
    pub jito_stake_pool: Pubkey,
    pub jito_sol_mint: Pubkey,
    pub spl_stake_pool_program: Pubkey,
    // Pyth price feed configuration
    pub sol_usd_price_feed: Pubkey, // SOL/USD price feed account
    // USDC configuration for payments
    pub usdc_mint: Pubkey, // USDC mint address
    // Global service counter for unique service IDs
    pub total_services: u64,
    pub last_payment_processed: i64, // Timestamp of last payment processing
    pub bump: u8,
}
