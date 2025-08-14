use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init_if_needed,
        payer = user,
        space = User::INIT_SPACE,
        seeds = [USER_SEED.as_bytes(), user.key().as_ref()],
        bump
    )]
    pub user_account: Account<'info, User>,

    #[account(
        mut,
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,

    /// CHECK: This is the program's SOL vault
    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump,
    )]
    pub sol_vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> Deposit<'info> {
    pub fn deposit(&mut self, amount: u64, bumps: &DepositBumps) -> Result<()> {
        require!(!self.global_state.is_paused, ErrorCode::ProtocolPaused);
        require!(amount > 0, ErrorCode::InvalidAmount);

        let user_account = &mut self.user_account;

        // Initialize user account if this is the first time
        if user_account.wallet == Pubkey::default() {
            user_account.wallet = self.user.key();
            user_account.deposited_sol = 0;
            user_account.locked_sol = 0;
            user_account.staked_sol = 0;
            user_account.created_at = Clock::get()?.unix_timestamp;
            user_account.bump = bumps.user_account;
        }

        // Transfer SOL from user to vault
        let ctx = CpiContext::new(
            self.system_program.to_account_info(),
            Transfer {
                from: self.user.to_account_info(),
                to: self.sol_vault.to_account_info(),
            },
        );
        transfer(ctx, amount)?;

        // Update user account
        user_account.deposited_sol = user_account
            .deposited_sol
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        msg!(
            "User {} deposited {} SOL (total: {} SOL)",
            self.user.key(),
            amount as f64 / 1_000_000_000.0,
            user_account.deposited_sol as f64 / 1_000_000_000.0
        );

        Ok(())
    }
}
