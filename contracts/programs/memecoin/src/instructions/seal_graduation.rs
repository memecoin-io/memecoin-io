//! Seal the graduation by recording the Meteora DLMM pool address.
//!
//! Called by the off-chain graduation authority **after** it has:
//!   1. Created the Meteora DLMM pool via `initializeCustomizablePermissionlessLbPair2`
//!   2. Seeded liquidity via `addLiquidityByStrategy2` with the SOL +
//!      reserve tokens it received from `graduate`
//!   3. Confirmed both transactions on mainnet
//!
//! Recording the pool address on-chain is what makes the migration
//! verifiable: the indexer reads `PoolSeeded` to switch its price feed
//! from our bonding-curve math to the DLMM pool's reserves, and anyone
//! can audit the chain to confirm graduated liquidity actually landed in
//! a live pool.
//!
//! Only the `graduation_authority` from `Config` can call this. Callable
//! once per launch (idempotent — second call returns `PoolAlreadySeeded`).

use anchor_lang::prelude::*;

use crate::errors::MemecoinError;
use crate::events::PoolSeeded;
use crate::state::{Config, TokenLaunch};

#[derive(Accounts)]
pub struct SealGraduation<'info> {
    /// Must equal `config.graduation_authority`.
    #[account(
        mut,
        address = config.graduation_authority @ MemecoinError::Unauthorized,
    )]
    pub graduation_authority: Signer<'info>,

    #[account(
        seeds = [Config::SEED],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [TokenLaunch::LAUNCH_SEED, launch.mint.as_ref()],
        bump = launch.bump,
    )]
    pub launch: Box<Account<'info, TokenLaunch>>,
}

pub fn handler(ctx: Context<SealGraduation>, meteora_lb_pair: Pubkey) -> Result<()> {
    require!(!ctx.accounts.config.paused, MemecoinError::Paused);

    let launch = &mut ctx.accounts.launch;

    require!(launch.graduated, MemecoinError::NotGraduated);
    require!(!launch.pool_seeded, MemecoinError::PoolAlreadySeeded);
    require!(
        meteora_lb_pair != Pubkey::default(),
        MemecoinError::InvalidPoolAddress
    );

    launch.pool_seeded = true;
    launch.meteora_lb_pair = meteora_lb_pair;

    emit!(PoolSeeded {
        mint: launch.mint,
        meteora_lb_pair,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
