use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};
use spl_stake_pool::instruction as spl_instruction;

#[derive(Accounts)]
#[instruction(jito_sol_amount: u64, jito_apy_bps: u16)]
pub struct UnstakeSol<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [USER_SEED.as_bytes(), user.key().as_ref()],
        bump = user_account.bump,
        constraint = user_account.wallet == user.key() @ ErrorCode::UnauthorizedUser
    )]
    pub user_account: Account<'info, User>,

    #[account(
        mut,
        seeds = [
            STAKE_ACCOUNT_SEED.as_bytes(),
            user.key().as_ref(),
        ],
        bump = stake_account.bump,
        constraint = stake_account.user == user.key() @ ErrorCode::UnauthorizedUser,
        constraint = stake_account.is_active @ ErrorCode::StakingNotAvailable
    )]
    pub stake_account: Account<'info, StakeAccount>,

    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump,
    )]
    pub sol_vault: SystemAccount<'info>,

    /// Global state for reading Jito configuration
    #[account(
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,

    /// Protocol's JitoSOL vault (ATA owned by protocol PDA)
    #[account(
        mut,
        associated_token::mint = jito_sol_mint,
        associated_token::authority = protocol_authority
    )]
    pub protocol_jito_vault: Account<'info, TokenAccount>,

    /// CHECK: Protocol authority PDA that owns JitoSOL vault
    #[account(
        seeds = [b"protocol_authority"],
        bump
    )]
    pub protocol_authority: UncheckedAccount<'info>,

    // ===== Jito/SPL Stake Pool Accounts for Withdrawal =====
    /// CHECK: SPL Stake Pool program (read from GlobalState)
    #[account(address = global_state.spl_stake_pool_program)]
    pub stake_pool_program: UncheckedAccount<'info>,

    /// CHECK: Jito Stake Pool account (read from GlobalState)
    #[account(
        mut,
        address = global_state.jito_stake_pool
    )]
    pub jito_stake_pool: UncheckedAccount<'info>,

    /// CHECK: Stake pool withdraw authority (PDA derived from stake pool)
    pub stake_pool_withdraw_authority: UncheckedAccount<'info>,

    /// JitoSOL mint (read from GlobalState)
    #[account(
        mut,
        address = global_state.jito_sol_mint
    )]
    pub jito_sol_mint: Account<'info, Mint>,

    /// CHECK: Jito manager fee account
    #[account(mut)]
    pub manager_fee_account: UncheckedAccount<'info>,

    // ===== Programs =====
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> UnstakeSol<'info> {
    pub fn unstake_sol(
        &mut self,
        jito_sol_amount: u64,
        jito_apy_bps: u16,
        bumps: &UnstakeSolBumps,
    ) -> Result<()> {
        require!(jito_sol_amount > 0, ErrorCode::InvalidAmount);
        require!(
            self.stake_account.jito_sol_amount >= jito_sol_amount,
            ErrorCode::InsufficientStakedFunds
        );

        // Prepare signer seeds for protocol authority
        let protocol_authority_bump = bumps.protocol_authority;
        let signer_seeds: &[&[&[u8]]] = &[&[b"protocol_authority", &[protocol_authority_bump]]];

        // ===== REAL JITO UNSTAKING =====
        // Using actual Jito SPL Stake Pool withdraw_sol instruction

        // Create the withdraw_sol instruction for Jito SPL Stake Pool
        let withdraw_instruction = spl_instruction::withdraw_sol(
            &self.stake_pool_program.key(),            // stake pool program
            &self.jito_stake_pool.key(),               // stake pool
            &self.stake_pool_withdraw_authority.key(), // withdraw authority
            &self.protocol_authority.key(),            // user transfer authority (protocol)
            &self.protocol_jito_vault.key(),           // burn from (JitoSOL source)
            &self.sol_vault.key(),                     // to (SOL destination)
            &self.manager_fee_account.key(),           // manager fee account
            &self.jito_sol_mint.key(),                 // pool mint
            &self.token_program.key(),                 // token program
            &self.system_program.key(),                // system program
            jito_sol_amount,                           // JitoSOL amount to burn
        );

        // Execute the Jito unstake via CPI
        anchor_lang::solana_program::program::invoke_signed(
            &withdraw_instruction,
            &[
                self.stake_pool_program.to_account_info(),
                self.jito_stake_pool.to_account_info(),
                self.stake_pool_withdraw_authority.to_account_info(),
                self.protocol_authority.to_account_info(),
                self.protocol_jito_vault.to_account_info(),
                self.sol_vault.to_account_info(),
                self.manager_fee_account.to_account_info(),
                self.jito_sol_mint.to_account_info(),
                self.token_program.to_account_info(),
                self.system_program.to_account_info(),
            ],
            signer_seeds,
        )?;

        // Calculate estimated SOL received using dynamic APY (reverse of staking calculation)
        // APY is in basis points (bps): 10000 bps = 100%, so jito_apy_bps + 10000 gives the multiplier
        let apy_multiplier = 10000_u64 + jito_apy_bps as u64; // e.g., 200 bps = 2% -> 10200
        let estimated_sol_received = jito_sol_amount
            .saturating_mul(apy_multiplier)
            .saturating_div(10000);

        // Update stake account
        self.stake_account.jito_sol_amount = self
            .stake_account
            .jito_sol_amount
            .checked_sub(jito_sol_amount)
            .unwrap();
        self.stake_account.staked_amount = self
            .stake_account
            .staked_amount
            .checked_sub(estimated_sol_received)
            .unwrap();

        // If no more staked amount, deactivate the stake account
        if self.stake_account.staked_amount == 0 {
            self.stake_account.is_active = false;
        }

        // Update user account
        self.user_account.staked_sol = self
            .user_account
            .staked_sol
            .checked_sub(estimated_sol_received)
            .unwrap();
        self.user_account.deposited_sol = self
            .user_account
            .deposited_sol
            .checked_add(estimated_sol_received)
            .unwrap();

        msg!(
            "User {} unstaked {} JitoSOL via pool {} with {:.2}% APY, received ~{} SOL",
            self.user.key(),
            jito_sol_amount as f64 / 1_000_000_000.0,
            self.global_state.jito_stake_pool,
            jito_apy_bps as f64 / 100.0,
            estimated_sol_received as f64 / 1_000_000_000.0
        );

        Ok(())
    }
}
