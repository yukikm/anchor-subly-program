use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + GlobalState::INIT_SPACE,
        seeds = [b"global_state"],
        
        bump
    )]
    pub global_state: Account<'info, GlobalState>,

    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    pub fn initialize_global_state(
        &mut self,
        jito_stake_pool: Pubkey,
        jito_sol_mint: Pubkey,
        spl_stake_pool_program: Pubkey,
        sol_usd_price_feed: Pubkey,
        usdc_mint: Pubkey,
        bumps: &InitializeBumps,
    ) -> Result<()> {
        self.initialize(
            bumps,
            jito_stake_pool,
            jito_sol_mint,
            spl_stake_pool_program,
            sol_usd_price_feed,
            usdc_mint,
        )
    }

    pub fn initialize(
        &mut self, 
        bumps: &InitializeBumps,
        jito_stake_pool: Pubkey,
        jito_sol_mint: Pubkey,
        spl_stake_pool_program: Pubkey,
        sol_usd_price_feed: Pubkey,
        usdc_mint: Pubkey,
    ) -> Result<()> {
        let global_state = &mut self.global_state;

        global_state.authority = self.authority.key();
        global_state.protocol_fee_bps = 100; // 1% protocol fee
        global_state.is_paused = false;
        
        // Set Jito configuration (can be mainnet or devnet)
        global_state.jito_stake_pool = jito_stake_pool;
        global_state.jito_sol_mint = jito_sol_mint;
        global_state.spl_stake_pool_program = spl_stake_pool_program;
        
        // Set Pyth price feed configuration
        global_state.sol_usd_price_feed = sol_usd_price_feed;
        
        // Set USDC mint configuration
        global_state.usdc_mint = usdc_mint;
        
        // Initialize counters and timestamps
        global_state.total_services = 0;
        global_state.last_payment_processed = 0;
        
        global_state.bump = bumps.global_state;

        msg!(
            "Subly protocol initialized by authority: {}",
            self.authority.key()
        );
        msg!(
            "Jito config - Pool: {}, Mint: {}, Program: {}", 
            jito_stake_pool,
            jito_sol_mint,
            spl_stake_pool_program
        );
        msg!(
            "Pyth SOL/USD price feed: {}",
            sol_usd_price_feed
        );
        msg!(
            "USDC mint: {}",
            usdc_mint
        );

        Ok(())
    }
}
