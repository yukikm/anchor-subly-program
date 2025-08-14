use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};
use spl_stake_pool::instruction as spl_instruction;

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct StakeSol<'info> {
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
        init_if_needed,
        payer = user,
        space = StakeAccount::INIT_SPACE,
        seeds = [
            STAKE_ACCOUNT_SEED.as_bytes(),
            user.key().as_ref(),
        ],
        bump
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
        init_if_needed,
        payer = user,
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

    // ===== Jito/SPL Stake Pool Accounts =====
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

    /// CHECK: Jito reserve stake account
    #[account(mut)]
    pub reserve_stake: UncheckedAccount<'info>,

    /// JitoSOL mint (read from GlobalState)
    #[account(
        mut,
        address = global_state.jito_sol_mint
    )]
    pub jito_sol_mint: Account<'info, Mint>,

    /// CHECK: Jito manager fee account
    #[account(mut)]
    pub manager_fee_account: UncheckedAccount<'info>,

    /// CHECK: Referrer pool tokens (can be same as protocol vault)
    #[account(mut)]
    pub referrer_pool_tokens: UncheckedAccount<'info>,

    // ===== Programs =====
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    /// CHECK: Stake program
    #[account(address = anchor_lang::solana_program::stake::program::ID)]
    pub stake_program: UncheckedAccount<'info>,
}

impl<'info> StakeSol<'info> {
    pub fn stake_sol(&mut self, amount: u64, bumps: &StakeSolBumps) -> Result<()> {
        require!(amount >= MIN_STAKE_AMOUNT, ErrorCode::MinimumStakeNotMet);

        let user_account = &mut self.user_account;
        let stake_account = &mut self.stake_account;

        // Check if user has sufficient available balance
        let available_balance = user_account
            .deposited_sol
            .checked_sub(user_account.locked_sol)
            .unwrap_or(0);

        require!(
            available_balance >= amount,
            ErrorCode::InsufficientAvailableBalance
        );

        // Transfer SOL from user vault to Jito stake pool via CPI
        let vault_bump = bumps.sol_vault;
        let user_key = self.user.key();
        let signer_seeds: &[&[&[u8]]] = &[&[b"vault", user_key.as_ref(), &[vault_bump]]];

        // ===== REAL JITO INTEGRATION =====
        // Using actual Jito SPL Stake Pool for liquid staking and yield generation

        // Create the deposit_sol instruction for Jito SPL Stake Pool
        let deposit_instruction = spl_instruction::deposit_sol(
            &self.stake_pool_program.key(),            // stake pool program
            &self.jito_stake_pool.key(),               // stake pool
            &self.stake_pool_withdraw_authority.key(), // withdraw authority
            &self.reserve_stake.key(),                 // reserve stake
            &self.sol_vault.key(),                     // from (SOL source)
            &self.protocol_jito_vault.key(),           // to (pool token destination)
            &self.manager_fee_account.key(),           // manager fee account
            &self.referrer_pool_tokens.key(),          // referrer pool tokens
            &self.jito_sol_mint.key(),                 // pool mint
            &self.token_program.key(),                 // token program
            amount,                                    // SOL amount
        );

        // Execute the Jito stake deposit via CPI
        anchor_lang::solana_program::program::invoke_signed(
            &deposit_instruction,
            &[
                self.stake_pool_program.to_account_info(),
                self.jito_stake_pool.to_account_info(),
                self.stake_pool_withdraw_authority.to_account_info(),
                self.reserve_stake.to_account_info(),
                self.sol_vault.to_account_info(),
                self.protocol_jito_vault.to_account_info(),
                self.manager_fee_account.to_account_info(),
                self.referrer_pool_tokens.to_account_info(),
                self.jito_sol_mint.to_account_info(),
                self.token_program.to_account_info(),
                self.system_program.to_account_info(),
            ],
            signer_seeds,
        )?;

        let current_time = Clock::get()?.unix_timestamp;

        // Initialize stake account
        stake_account.user = self.user.key();
        stake_account.staked_amount = amount;

        // Get the JitoSOL token balance after staking
        // In real implementation, we'd calculate the exact amount based on the pool's exchange rate
        // For now, we'll use a conservative estimate (Jito typically gives slightly less than 1:1)
        let estimated_jito_sol = amount.saturating_mul(98).saturating_div(100); // ~2% difference
        stake_account.jito_sol_amount = estimated_jito_sol;

        stake_account.stake_date = current_time;
        stake_account.last_yield_claim = current_time;
        stake_account.total_yield_earned = 0;
        stake_account.is_active = true;
        stake_account.bump = bumps.stake_account;

        // Update user account
        user_account.deposited_sol = user_account.deposited_sol.checked_sub(amount).unwrap();
        user_account.staked_sol = user_account.staked_sol.checked_add(amount).unwrap();

        msg!(
            "User {} staked {} SOL via Jito SPL Stake Pool ({}), received ~{} JitoSOL",
            self.user.key(),
            amount as f64 / 1_000_000_000.0,
            self.global_state.jito_stake_pool,
            estimated_jito_sol as f64 / 1_000_000_000.0
        );

        Ok(())
    }
}
