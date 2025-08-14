use anchor_lang::prelude::*;
use crate::{constants::*, error::ErrorCode, state::*};

#[derive(Accounts)]
pub struct ClaimYield<'info> {
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

    /// CHECK: Jito vault for claiming yield
    #[account(
        mut,
        seeds = [JITO_VAULT_SEED.as_bytes()],
        bump
    )]
    pub jito_vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> ClaimYield<'info> {
    pub fn claim_yield(&mut self, bumps: &ClaimYieldBumps) -> Result<()> {
        let user_account = &mut self.user_account;
        let stake_account = &mut self.stake_account;

        let current_time = Clock::get()?.unix_timestamp;
        let time_since_last_claim = current_time - stake_account.last_yield_claim;

        // Only allow claiming if enough time has passed (24 hours)
        require!(
            time_since_last_claim >= YIELD_CALCULATION_PERIOD,
            ErrorCode::PaymentNotDue
        );

        // Calculate yield (simplified - 5% APY)
        // yield = staked_amount * 0.05 * (time_since_last_claim / 31536000) // seconds in a year
        let yield_amount = stake_account.staked_amount
            .checked_mul(5)
            .and_then(|x| x.checked_div(100))
            .and_then(|x| x.checked_mul(time_since_last_claim as u64))
            .and_then(|x| x.checked_div(31536000)) // seconds in a year
            .unwrap_or(0);

        if yield_amount > 0 {
            // Transfer yield from Jito vault to user vault (simplified)
            let jito_vault_bump = bumps.jito_vault;
            let signer_seeds: &[&[&[u8]]] = &[&[
                JITO_VAULT_SEED.as_bytes(),
                &[jito_vault_bump],
            ]];

            let transfer_ix = anchor_lang::system_program::Transfer {
                from: self.jito_vault.to_account_info(),
                to: self.sol_vault.to_account_info(),
            };

            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    self.system_program.to_account_info(),
                    transfer_ix,
                    signer_seeds,
                ),
                yield_amount,
            )?;

            // Update accounts
            stake_account.total_yield_earned = stake_account.total_yield_earned
                .checked_add(yield_amount)
                .unwrap();
            stake_account.last_yield_claim = current_time;

            user_account.deposited_sol = user_account.deposited_sol
                .checked_add(yield_amount)
                .unwrap();

            msg!(
                "User {} claimed {} SOL yield (total earned: {} SOL)",
                self.user.key(),
                yield_amount as f64 / 1_000_000_000.0,
                stake_account.total_yield_earned as f64 / 1_000_000_000.0
            );
        } else {
            msg!("No yield available to claim");
        }

        Ok(())
    }
}
