use crate::{constants::*, error::ErrorCode, state::*};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};

#[derive(Accounts)]
#[instruction(name: String)]
pub struct RegisterProvider<'info> {
    #[account(mut)]
    pub provider: Signer<'info>,

    #[account(
        mut,
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,

    #[account(
        init,
        payer = provider,
        space = Provider::INIT_SPACE,
        seeds = [PROVIDER_SEED.as_bytes(), provider.key().as_ref()],
        bump
    )]
    pub provider_account: Account<'info, Provider>,

    // Provider verification NFT
    #[account(
        init,
        payer = provider,
        mint::decimals = 0,
        mint::authority = provider,
        mint::freeze_authority = provider,
    )]
    pub provider_nft_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = provider,
        associated_token::mint = provider_nft_mint,
        associated_token::authority = provider,
    )]
    pub provider_nft_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> RegisterProvider<'info> {
    pub fn register_provider(
        &mut self,
        name: String,
        description: String,
        bumps: &RegisterProviderBumps,
    ) -> Result<()> {
        require!(!self.global_state.is_paused, ErrorCode::ProtocolPaused);
        require!(name.len() <= MAX_NAME_LENGTH, ErrorCode::NameTooLong);
        require!(
            description.len() <= MAX_DESCRIPTION_LENGTH,
            ErrorCode::DescriptionTooLong
        );

        let provider_account = &mut self.provider_account;

        provider_account.wallet = self.provider.key();
        provider_account.name = name.clone();
        provider_account.description = description;
        provider_account.total_subscribers = 0;
        provider_account.is_verified = false;
        provider_account.created_at = Clock::get()?.unix_timestamp;
        provider_account.bump = bumps.provider_account;

        // Mint provider verification NFT
        let cpi_accounts = MintTo {
            mint: self.provider_nft_mint.to_account_info(),
            to: self.provider_nft_token_account.to_account_info(),
            authority: self.provider.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        mint_to(cpi_ctx, 1)?;

        msg!(
            "Provider '{}' registered with NFT: {}",
            name,
            self.provider_nft_mint.key()
        );

        Ok(())
    }
}
