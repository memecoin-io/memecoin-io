//! Graduate a token from the bonding curve to the Meteora DLMM hand-off stage.
//!
//! ## Why this is split in two
//!
//! Meteora's DLMM pool init + seed liquidity flow requires ~80 accounts and
//! a Rust crate version-pinned to a specific Solana toolchain that
//! conflicts with our 0.30.1 Anchor stack. Cramming all of it into one
//! atomic CPI also exceeds Solana's 1232-byte tx size limit.
//!
//! Production launchers (pump.fun, raydium launchlab, letsbonk) solve this
//! the same way: the on-chain program finalises the curve and **parks
//! the graduation liquidity with an off-chain authority**; that authority
//! runs the Meteora bundle in a single transaction off-chain (using
//! Meteora's official TS SDK with all the right account derivations) and
//! then comes back on-chain to seal the launch via `mark_pool_seeded`.
//!
//! This file handles step one only.
//!
//! ## What this instruction does
//!
//! 1. Validates the curve hit the graduation threshold
//! 2. Pays the 1 SOL graduation fee, split 50/50 to DINO buyback + treasury
//! 3. Mints the 200M `GRADUATION_RESERVE` tokens to the graduation
//!    authority's ATA
//! 4. Drains the remaining SOL from the curve vault to the graduation
//!    authority wallet
//! 5. Marks `launch.graduated = true`, snapshots the amounts forwarded
//! 6. Emits `TokenGraduated` (consumed by the off-chain crank)
//!
//! After this runs, `mark_pool_seeded` (in `seal_graduation.rs`) is the
//! follow-up the crank calls once Meteora confirms.

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};

use crate::errors::MemecoinError;
use crate::events::TokenGraduated;
use crate::state::{Config, TokenLaunch};
use crate::utils::split_two_way;

#[derive(Accounts)]
pub struct Graduate<'info> {
    /// Anyone can trigger graduation once the threshold is met (permissionless
    /// crank). The caller pays for any ATA initialization but receives nothing.
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        seeds = [Config::SEED],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [TokenLaunch::LAUNCH_SEED, mint.key().as_ref()],
        bump = launch.bump,
        has_one = mint,
    )]
    pub launch: Box<Account<'info, TokenLaunch>>,

    #[account(mut)]
    pub mint: Box<Account<'info, Mint>>,

    /// CHECK: PDA, validated by seeds. Drained at graduation.
    #[account(
        mut,
        seeds = [TokenLaunch::VAULT_SEED, mint.key().as_ref()],
        bump = launch.vault_bump,
    )]
    pub curve_vault: UncheckedAccount<'info>,

    /// CHECK: PDA mint authority — signs the 200M graduation reserve mint.
    #[account(
        seeds = [TokenLaunch::MINT_AUTHORITY_SEED, mint.key().as_ref()],
        bump = launch.mint_authority_bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    /// CHECK: validated against `config.graduation_authority`. Receives the
    /// remaining curve SOL minus the graduation fee.
    #[account(
        mut,
        address = config.graduation_authority @ MemecoinError::InvalidWallet,
    )]
    pub graduation_authority: UncheckedAccount<'info>,

    /// ATA owned by the graduation authority that will hold the 200M
    /// graduation reserve. Created on demand by the caller.
    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = mint,
        associated_token::authority = graduation_authority,
    )]
    pub graduation_authority_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: validated via config.
    #[account(
        mut,
        address = config.dino_buyback_wallet @ MemecoinError::InvalidWallet,
    )]
    pub dino_buyback_wallet: UncheckedAccount<'info>,

    /// CHECK: validated via config.
    #[account(
        mut,
        address = config.treasury_wallet @ MemecoinError::InvalidWallet,
    )]
    pub treasury_wallet: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<Graduate>) -> Result<()> {
    require!(!ctx.accounts.config.paused, MemecoinError::Paused);
    require!(!ctx.accounts.launch.graduated, MemecoinError::AlreadyGraduated);

    let config = &ctx.accounts.config;
    let launch = &mut ctx.accounts.launch;

    require!(
        launch.real_sol_reserves >= config.graduation_threshold_lamports,
        MemecoinError::NotReadyToGraduate
    );

    // === 1. Pay graduation fee, split DINO/treasury ===
    let fee = config.graduation_fee_lamports;
    require!(
        launch.real_sol_reserves >= fee,
        MemecoinError::InsufficientReserves
    );

    let (dino_share, treasury_share) =
        split_two_way(fee, config.graduation_split_dino_bps).map_err(|e| error!(e))?;

    // Vault is a System-owned PDA SOL escrow. Transfer out via
    // system_program::transfer signed by the vault PDA seeds.
    let mint_key_for_vault = launch.mint;
    let vault_seeds: &[&[u8]] = &[
        TokenLaunch::VAULT_SEED,
        mint_key_for_vault.as_ref(),
        &[launch.vault_bump],
    ];
    let vault_signer = &[vault_seeds];

    if dino_share > 0 {
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.curve_vault.to_account_info(),
                    to: ctx.accounts.dino_buyback_wallet.to_account_info(),
                },
                vault_signer,
            ),
            dino_share,
        )?;
    }
    if treasury_share > 0 {
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.curve_vault.to_account_info(),
                    to: ctx.accounts.treasury_wallet.to_account_info(),
                },
                vault_signer,
            ),
            treasury_share,
        )?;
    }

    // === 2. Mint the 200M graduation reserve to the graduation authority ===
    let reserve_amount = TokenLaunch::GRADUATION_RESERVE
        .checked_mul(10u64.pow(TokenLaunch::DECIMALS as u32))
        .ok_or(MemecoinError::MathOverflow)?;

    let mint_key = launch.mint;
    let mint_auth_seeds: &[&[u8]] = &[
        TokenLaunch::MINT_AUTHORITY_SEED,
        mint_key.as_ref(),
        &[launch.mint_authority_bump],
    ];
    let signer = &[mint_auth_seeds];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.graduation_authority_token_account.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
            signer,
        ),
        reserve_amount,
    )?;

    // === 3. Forward remaining curve SOL to the graduation authority ===
    //
    // This is the SOL the off-chain crank will deposit as the quote-side of
    // the Meteora DLMM pool. The crank holds it in trust for ~tens of
    // seconds until the seed-liquidity tx confirms.
    let remaining_sol = launch.real_sol_reserves.saturating_sub(fee);
    if remaining_sol > 0 {
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.curve_vault.to_account_info(),
                    to: ctx.accounts.graduation_authority.to_account_info(),
                },
                vault_signer,
            ),
            remaining_sol,
        )?;
    }

    // === 4. Mark as graduated, snapshot the hand-off ===
    launch.graduated = true;
    launch.pool_seeded = false;
    launch.meteora_lb_pair = Pubkey::default();
    launch.graduation_sol_lamports = remaining_sol;
    launch.graduation_token_amount = reserve_amount;
    launch.real_sol_reserves = 0;
    launch.real_token_reserves = 0;

    emit!(TokenGraduated {
        mint: launch.mint,
        creator: launch.creator,
        final_sol_reserves: remaining_sol,
        final_token_reserves: reserve_amount,
        graduation_fee_paid: fee,
        dino_share,
        treasury_share,
        graduation_authority: config.graduation_authority,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}


