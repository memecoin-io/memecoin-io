//! Memecoin.io — Solana memecoin launcher
//!
//! A Pump.fun-style bonding-curve launcher with cross-chain DINO buybacks.
//!
//! Fee structure (v1, locked):
//!   - Trade fee:      1.0%   split 33.33% creator / 33.33% DINO / 33.33% treasury
//!   - Creation fee:   0.02 SOL  split 50% DINO / 50% treasury
//!   - Graduation fee: 1 SOL   split 50% DINO / 50% treasury
//!
//! Bonding curve: constant-product with virtual reserves.
//! Graduation: at ~55 SOL deposited (~$42k mcap), token migrates to Meteora DLMM.
//!
//! See ARCHITECTURE.md for the full spec.

use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;
use state::*;

declare_id!("CuJx7BY4Zu9tJzbDTGGTm8tMpE1v2PTEeecB5qfCmhs3");

#[program]
pub mod memecoin {
    use super::*;

    /// One-time global config initialization. Admin-only.
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        params: InitializeConfigParams,
    ) -> Result<()> {
        instructions::initialize_config::handler(ctx, params)
    }

    /// Update any tunable config field. Admin-only.
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        params: UpdateConfigParams,
    ) -> Result<()> {
        instructions::update_config::handler(ctx, params)
    }

    /// Emergency pause / unpause. Admin-only.
    pub fn set_paused(ctx: Context<SetPaused>, paused: bool) -> Result<()> {
        instructions::set_paused::handler(ctx, paused)
    }

    /// Create a new token + bonding curve. Anyone can call.
    /// Pays the creation fee, splits 50/50 to DINO buyback / treasury.
    pub fn create_token<'info>(
        ctx: Context<'_, '_, '_, 'info, CreateToken<'info>>,
        params: Box<CreateTokenParams>,
    ) -> Result<()> {
        instructions::create_token::handler(ctx, params)
    }

    /// Buy tokens from the bonding curve with SOL.
    /// Collects 1% fee, splits 33.33/33.33/33.33 (creator/DINO/treasury).
    /// `max_sol_in`: slippage protection — reverts if curve requires more SOL.
    pub fn buy(
        ctx: Context<Buy>,
        token_amount_out: u64,
        max_sol_in: u64,
    ) -> Result<()> {
        instructions::buy::handler(ctx, token_amount_out, max_sol_in)
    }

    /// Sell tokens back to the bonding curve for SOL.
    /// `min_sol_out`: slippage protection — reverts if curve returns less SOL.
    pub fn sell(
        ctx: Context<Sell>,
        token_amount_in: u64,
        min_sol_out: u64,
    ) -> Result<()> {
        instructions::sell::handler(ctx, token_amount_in, min_sol_out)
    }

    /// Graduate the token: pay graduation fee, mint the 200M reserve,
    /// forward SOL + tokens to the off-chain graduation authority that
    /// will create the Meteora DLMM pool. Permissionless trigger.
    pub fn graduate(ctx: Context<Graduate>) -> Result<()> {
        instructions::graduate::handler(ctx)
    }

    /// Seal the graduation by recording the Meteora DLMM pool address.
    /// Only callable by `config.graduation_authority` after the off-chain
    /// pool init + seed-liquidity bundle confirms.
    pub fn seal_graduation(
        ctx: Context<SealGraduation>,
        meteora_lb_pair: Pubkey,
    ) -> Result<()> {
        instructions::seal_graduation::handler(ctx, meteora_lb_pair)
    }
}
