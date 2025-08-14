use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{TokenAccount, Mint, Token},
};
use spl_stake_pool::instruction as spl_instruction;

#[derive(Accounts)]
#[instruction(amount: u64, jito_apy_bps: u16)]
pub struct Withdraw<'info> {
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

    /// User's stake account (optional - may not exist if user never staked)
    #[account(
        mut,
        seeds = [
            STAKE_ACCOUNT_SEED.as_bytes(),
            user.key().as_ref(),
        ],
        bump,
        constraint = stake_account.user == user.key() @ ErrorCode::UnauthorizedUser
    )]
    pub stake_account: Option<Account<'info, StakeAccount>>,

    /// Protocol's JitoSOL vault (ATA owned by protocol PDA) - optional
    #[account(
        mut,
        associated_token::mint = jito_sol_mint,
        associated_token::authority = protocol_authority
    )]
    pub protocol_jito_vault: Option<Account<'info, TokenAccount>>,

    /// CHECK: Protocol authority PDA that owns JitoSOL vault - optional
    #[account(
        seeds = [b"protocol_authority"],
        bump
    )]
    pub protocol_authority: Option<UncheckedAccount<'info>>,

    // ===== Optional Jito/SPL Stake Pool Accounts for Withdrawal =====
    
    /// CHECK: SPL Stake Pool program (read from GlobalState) - required for unstaking
    #[account(address = global_state.spl_stake_pool_program)]
    pub stake_pool_program: Option<UncheckedAccount<'info>>,

    /// CHECK: Jito Stake Pool account (read from GlobalState) - required for unstaking
    #[account(
        mut,
        address = global_state.jito_stake_pool
    )]
    pub jito_stake_pool: Option<UncheckedAccount<'info>>,

    /// CHECK: Stake pool withdraw authority (PDA derived from stake pool) - required for unstaking
    pub stake_pool_withdraw_authority: Option<UncheckedAccount<'info>>,

    /// JitoSOL mint (read from GlobalState) - required for unstaking
    #[account(
        mut,
        address = global_state.jito_sol_mint
    )]
    pub jito_sol_mint: Option<Account<'info, Mint>>,

    /// CHECK: Jito manager fee account - required for unstaking
    #[account(mut)]
    pub manager_fee_account: Option<UncheckedAccount<'info>>,

    // ===== Programs =====
    pub token_program: Option<Program<'info, Token>>,
    pub associated_token_program: Option<Program<'info, AssociatedToken>>,
    pub system_program: Program<'info, System>,
}

impl<'info> Withdraw<'info> {
    pub fn withdraw(&mut self, amount: u64, _jito_apy_bps: u16, bumps: &WithdrawBumps) -> Result<()> {
        require!(amount > 0, ErrorCode::InvalidAmount);
        require!(
            self.user_account.deposited_sol >= amount,
            ErrorCode::InsufficientBalance
        );

        // Calculate available balance (deposited - locked for subscriptions)
        let available_balance = self.user_account
            .deposited_sol
            .checked_sub(self.user_account.locked_sol)
            .unwrap_or(0);

        require!(
            available_balance >= amount,
            ErrorCode::InsufficientAvailableBalance
        );

        // Simple withdrawal - unstaking is now handled separately in lib.rs
        require!(
            self.sol_vault.lamports() >= amount,
            ErrorCode::InsufficientBalance
        );

        // Get required bump and key values
        let user_key = self.user.key();
        let vault_bump = bumps.sol_vault;

        // Transfer SOL from vault to user
        let transfer_ix = anchor_lang::system_program::Transfer {
            from: self.sol_vault.to_account_info(),
            to: self.user.to_account_info(),
        };

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                self.system_program.to_account_info(),
                transfer_ix,
                &[&[b"vault", user_key.as_ref(), &[vault_bump]]],
            ),
            amount,
        )?;

        // Update user account
        self.user_account.deposited_sol = self.user_account.deposited_sol.checked_sub(amount).unwrap();

        msg!(
            "User {} withdrew {} SOL (remaining: {} SOL, staked: {} SOL)",
            self.user.key(),
            amount as f64 / 1_000_000_000.0,
            self.user_account.deposited_sol as f64 / 1_000_000_000.0,
            self.user_account.staked_sol as f64 / 1_000_000_000.0
        );

        Ok(())
    }

    /// Helper function to unstake from Jito when automatic unstaking is needed
    fn unstake_from_jito(&mut self, jito_sol_amount: u64, jito_apy_bps: u16, bumps: &WithdrawBumps) -> Result<()> {
        require!(
            self.protocol_jito_vault.is_some(),
            ErrorCode::StakingNotAvailable
        );
        require!(
            self.stake_pool_program.is_some(),
            ErrorCode::StakingNotAvailable
        );

        // Get protocol authority bump
        let protocol_authority_bump = bumps.protocol_authority.unwrap();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"protocol_authority",
            &[protocol_authority_bump],
        ]];

        // Create the withdraw_sol instruction for Jito SPL Stake Pool
        let withdraw_instruction = spl_instruction::withdraw_sol(
            &self.stake_pool_program.as_ref().unwrap().key(),
            &self.jito_stake_pool.as_ref().unwrap().key(),
            &self.stake_pool_withdraw_authority.as_ref().unwrap().key(),
            &self.protocol_authority.as_ref().unwrap().key(),
            &self.protocol_jito_vault.as_ref().unwrap().key(),
            &self.sol_vault.key(),
            &self.manager_fee_account.as_ref().unwrap().key(),
            &self.jito_sol_mint.as_ref().unwrap().key(),
            &self.token_program.as_ref().unwrap().key(),
            &self.system_program.key(),
            jito_sol_amount,
        );

        // Execute the Jito unstake via CPI
        anchor_lang::solana_program::program::invoke_signed(
            &withdraw_instruction,
            &[
                self.stake_pool_program.as_ref().unwrap().to_account_info(),
                self.jito_stake_pool.as_ref().unwrap().to_account_info(),
                self.stake_pool_withdraw_authority.as_ref().unwrap().to_account_info(),
                self.protocol_authority.as_ref().unwrap().to_account_info(),
                self.protocol_jito_vault.as_ref().unwrap().to_account_info(),
                self.sol_vault.to_account_info(),
                self.manager_fee_account.as_ref().unwrap().to_account_info(),
                self.jito_sol_mint.as_ref().unwrap().to_account_info(),
                self.token_program.as_ref().unwrap().to_account_info(),
                self.system_program.to_account_info(),
            ],
            signer_seeds,
        )?;

        // Calculate estimated SOL received using dynamic APY
        let apy_multiplier = 10000_u64 + jito_apy_bps as u64;
        let estimated_sol_received = jito_sol_amount.saturating_mul(apy_multiplier).saturating_div(10000);

        // Update stake account
        if let Some(stake_account) = &mut self.stake_account {
            stake_account.jito_sol_amount = stake_account.jito_sol_amount.checked_sub(jito_sol_amount).unwrap();
            stake_account.staked_amount = stake_account.staked_amount.checked_sub(estimated_sol_received).unwrap();

            // If no more staked amount, deactivate the stake account
            if stake_account.staked_amount == 0 {
                stake_account.is_active = false;
            }
        }

        // Update user account
        self.user_account.staked_sol = self.user_account.staked_sol.checked_sub(estimated_sol_received).unwrap();
        self.user_account.deposited_sol = self.user_account.deposited_sol.checked_add(estimated_sol_received).unwrap();

        Ok(())
    }

    /// Sequential unstaking method called before withdraw
    pub fn unstake_sol_if_needed(&mut self, withdraw_amount: u64, jito_apy_bps: u16, bumps: &WithdrawBumps) -> Result<()> {
        // Check if we have sufficient unlocked SOL for withdrawal
        let vault_balance = self.sol_vault.lamports();
        
        if vault_balance >= withdraw_amount {
            // Sufficient unlocked SOL, no need to unstake
            msg!("Sufficient unlocked SOL ({} SOL), no unstaking needed", vault_balance as f64 / 1_000_000_000.0);
            return Ok(());
        }

        // Check if user has staked SOL to unstake
        let stake_account = match &self.stake_account {
            Some(account) => account,
            None => {
                msg!("No stake account found, cannot unstake");
                return Err(ErrorCode::InsufficientBalance.into());
            }
        };

        if stake_account.jito_sol_amount == 0 {
            msg!("No staked JitoSOL to unstake");
            return Err(ErrorCode::InsufficientBalance.into());
        }

        // Calculate how much SOL we need to unstake
        let needed_sol = withdraw_amount - vault_balance;
        
        // Calculate JitoSOL amount needed (reverse of APY calculation)
        let apy_multiplier = 10000_u64 + jito_apy_bps as u64;
        let jito_sol_needed = needed_sol.saturating_mul(10000).saturating_div(apy_multiplier);
        
        // Use the minimum of what we need and what we have staked
        let jito_sol_to_unstake = jito_sol_needed.min(stake_account.jito_sol_amount);

        msg!(
            "Unstaking {} JitoSOL to get ~{} SOL for withdrawal",
            jito_sol_to_unstake as f64 / 1_000_000_000.0,
            needed_sol as f64 / 1_000_000_000.0
        );

        // Call the existing unstake helper method
        self.unstake_from_jito(jito_sol_to_unstake, jito_apy_bps, bumps)
    }
}
