use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::errors::MemecoinError;
use crate::events::ConfigInitialized;
use crate::state::Config;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializeConfigParams {
    pub dino_buyback_wallet: Pubkey,
    pub treasury_wallet: Pubkey,
    pub graduation_authority: Pubkey,
    pub trade_fee_bps: u16,
    pub trade_split_creator_bps: u16,
    pub trade_split_dino_bps: u16,
    pub trade_split_treasury_bps: u16,
    pub creation_fee_lamports: u64,
    pub creation_split_dino_bps: u16,
    pub graduation_fee_lamports: u64,
    pub graduation_split_dino_bps: u16,
    pub graduation_threshold_lamports: u64,
}

impl Default for InitializeConfigParams {
    fn default() -> Self {
        Self {
            dino_buyback_wallet: Pubkey::default(),
            treasury_wallet: Pubkey::default(),
            graduation_authority: Pubkey::default(),
            // 1% trade fee, 33.33 / 33.33 / 33.34
            trade_fee_bps: 100,
            trade_split_creator_bps: 3333,
            trade_split_dino_bps: 3333,
            trade_split_treasury_bps: 3334,
            // 0.02 SOL creation fee, 50/50
            creation_fee_lamports: 20_000_000,
            creation_split_dino_bps: 5000,
            // 1 SOL graduation fee, 50/50
            graduation_fee_lamports: 1_000_000_000,
            graduation_split_dino_bps: 5000,
            // 5 SOL graduation threshold (beta phase — matches MAX_REAL_SOL_RESERVES).
            // Raise to 55 SOL via update_config + MAX_REAL_SOL_RESERVES bump in program upgrade post-audit.
            graduation_threshold_lamports: 5 * 1_000_000_000,
        }
    }
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + Config::INIT_SPACE,
        seeds = [Config::SEED],
        bump,
    )]
    pub config: Box<Account<'info, Config>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeConfig>,
    params: InitializeConfigParams,
) -> Result<()> {
    require!(
        params.dino_buyback_wallet != Pubkey::default()
            && params.dino_buyback_wallet != system_program::ID,
        MemecoinError::InvalidWallet
    );
    require!(
        params.treasury_wallet != Pubkey::default()
            && params.treasury_wallet != system_program::ID,
        MemecoinError::InvalidWallet
    );
    require!(
        params.graduation_authority != Pubkey::default()
            && params.graduation_authority != system_program::ID,
        MemecoinError::InvalidWallet
    );

    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.admin.key();
    config.dino_buyback_wallet = params.dino_buyback_wallet;
    config.treasury_wallet = params.treasury_wallet;
    config.graduation_authority = params.graduation_authority;
    config.trade_fee_bps = params.trade_fee_bps;
    config.trade_split_creator_bps = params.trade_split_creator_bps;
    config.trade_split_dino_bps = params.trade_split_dino_bps;
    config.trade_split_treasury_bps = params.trade_split_treasury_bps;
    config.creation_fee_lamports = params.creation_fee_lamports;
    config.creation_split_dino_bps = params.creation_split_dino_bps;
    config.graduation_fee_lamports = params.graduation_fee_lamports;
    config.graduation_split_dino_bps = params.graduation_split_dino_bps;
    config.graduation_threshold_lamports = params.graduation_threshold_lamports;
    config.paused = false;
    config.bump = ctx.bumps.config;
    config._reserved = [0u8; 96];

    config.validate().map_err(|e| error!(e))?;

    emit!(ConfigInitialized {
        admin: config.admin,
        dino_buyback_wallet: config.dino_buyback_wallet,
        treasury_wallet: config.treasury_wallet,
    });

    Ok(())
}
