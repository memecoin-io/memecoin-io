use anchor_lang::prelude::*;

use crate::errors::MemecoinError;
use crate::events::ConfigUpdated;
use crate::state::Config;

/// Optional fields — only `Some(_)` values are applied.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct UpdateConfigParams {
    pub new_admin: Option<Pubkey>,
    pub dino_buyback_wallet: Option<Pubkey>,
    pub treasury_wallet: Option<Pubkey>,
    pub graduation_authority: Option<Pubkey>,
    pub trade_fee_bps: Option<u16>,
    pub trade_split_creator_bps: Option<u16>,
    pub trade_split_dino_bps: Option<u16>,
    pub trade_split_treasury_bps: Option<u16>,
    pub creation_fee_lamports: Option<u64>,
    pub creation_split_dino_bps: Option<u16>,
    pub graduation_fee_lamports: Option<u64>,
    pub graduation_split_dino_bps: Option<u16>,
    pub graduation_threshold_lamports: Option<u64>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED],
        bump = config.bump,
        has_one = admin @ MemecoinError::Unauthorized,
    )]
    pub config: Box<Account<'info, Config>>,
}

pub fn handler(ctx: Context<UpdateConfig>, params: UpdateConfigParams) -> Result<()> {
    let config = &mut ctx.accounts.config;

    if let Some(new_admin) = params.new_admin {
        config.admin = new_admin;
    }
    if let Some(w) = params.dino_buyback_wallet {
        require!(w != Pubkey::default(), MemecoinError::InvalidWallet);
        config.dino_buyback_wallet = w;
    }
    if let Some(w) = params.treasury_wallet {
        require!(w != Pubkey::default(), MemecoinError::InvalidWallet);
        config.treasury_wallet = w;
    }
    if let Some(w) = params.graduation_authority {
        require!(w != Pubkey::default(), MemecoinError::InvalidWallet);
        config.graduation_authority = w;
    }
    if let Some(v) = params.trade_fee_bps {
        config.trade_fee_bps = v;
    }
    if let Some(v) = params.trade_split_creator_bps {
        config.trade_split_creator_bps = v;
    }
    if let Some(v) = params.trade_split_dino_bps {
        config.trade_split_dino_bps = v;
    }
    if let Some(v) = params.trade_split_treasury_bps {
        config.trade_split_treasury_bps = v;
    }
    if let Some(v) = params.creation_fee_lamports {
        config.creation_fee_lamports = v;
    }
    if let Some(v) = params.creation_split_dino_bps {
        config.creation_split_dino_bps = v;
    }
    if let Some(v) = params.graduation_fee_lamports {
        config.graduation_fee_lamports = v;
    }
    if let Some(v) = params.graduation_split_dino_bps {
        config.graduation_split_dino_bps = v;
    }
    if let Some(v) = params.graduation_threshold_lamports {
        config.graduation_threshold_lamports = v;
    }

    config.validate().map_err(|e| error!(e))?;

    emit!(ConfigUpdated {
        admin: ctx.accounts.admin.key(),
    });

    Ok(())
}
