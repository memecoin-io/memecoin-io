use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::errors::MemecoinError;
use crate::events::TradeExecuted;
use crate::state::{Config, TokenLaunch};
use crate::utils::{sol_out_for_tokens_in, split_three_way};

#[derive(Accounts)]
pub struct Sell<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

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

    pub mint: Box<Account<'info, Mint>>,

    /// CHECK: PDA SOL escrow.
    #[account(
        mut,
        seeds = [TokenLaunch::VAULT_SEED, mint.key().as_ref()],
        bump = launch.vault_bump,
    )]
    pub curve_vault: UncheckedAccount<'info>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = launch,
    )]
    pub curve_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = seller,
    )]
    pub seller_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: receives creator share.
    #[account(
        mut,
        address = launch.creator @ MemecoinError::InvalidWallet,
    )]
    pub creator_wallet: UncheckedAccount<'info>,

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
}

/// Sell tokens to the bonding curve for SOL.
///
/// Flow:
///   1. Compute gross SOL the curve owes for tokens_in (curve math)
///   2. Apply 1% fee → seller receives (gross - fee), check min_sol_out
///   3. Tokens transferred seller → curve_token_account
///   4. SOL: gross is debited from curve_vault PDA via direct lamports manipulation
///      (system_program::transfer can't sign for a PDA-owned data-less SOL escrow easily)
///      We use the AccountInfo lamports trick: subtract from vault, add to recipients
///   5. Update curve state
pub fn handler(ctx: Context<Sell>, token_amount_in: u64, min_sol_out: u64) -> Result<()> {
    require!(!ctx.accounts.config.paused, MemecoinError::Paused);
    require!(!ctx.accounts.launch.graduated, MemecoinError::AlreadyGraduated);
    require!(token_amount_in > 0, MemecoinError::ZeroSellAmount);

    let config = &ctx.accounts.config;
    let launch = &mut ctx.accounts.launch;

    // === 1. Bonding curve quote ===
    let gross_sol_out = sol_out_for_tokens_in(
        launch.virtual_sol_reserves,
        launch.virtual_token_reserves,
        launch.real_sol_reserves,
        token_amount_in,
    )
    .map_err(|e| error!(e))?;

    require!(
        gross_sol_out <= launch.real_sol_reserves,
        MemecoinError::InsufficientReserves
    );

    // === 2. Fee ===
    let fee = (gross_sol_out as u128)
        .checked_mul(config.trade_fee_bps as u128)
        .ok_or(MemecoinError::MathOverflow)?
        / 10_000;
    let fee_u64: u64 = fee.try_into().map_err(|_| MemecoinError::MathOverflow)?;
    let net_sol_out = gross_sol_out
        .checked_sub(fee_u64)
        .ok_or(MemecoinError::MathUnderflow)?;

    require!(net_sol_out >= min_sol_out, MemecoinError::InsufficientSolOut);

    let (creator_share, dino_share, treasury_share) = split_three_way(
        fee_u64,
        config.trade_split_creator_bps,
        config.trade_split_dino_bps,
        config.trade_split_treasury_bps,
    )
    .map_err(|e| error!(e))?;

    // === 3. Tokens: seller → curve ===
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.seller_token_account.to_account_info(),
                to: ctx.accounts.curve_token_account.to_account_info(),
                authority: ctx.accounts.seller.to_account_info(),
            },
        ),
        token_amount_in,
    )?;

    // === 4. SOL: drain curve_vault → seller + fee recipients ===
    // Vault is a System-owned PDA SOL escrow. Use system_program::transfer
    // signed by the vault PDA seeds.
    let mint_key_for_vault = launch.mint;
    let vault_seeds: &[&[u8]] = &[
        TokenLaunch::VAULT_SEED,
        mint_key_for_vault.as_ref(),
        &[launch.vault_bump],
    ];
    let vault_signer = &[vault_seeds];

    if net_sol_out > 0 {
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.curve_vault.to_account_info(),
                    to: ctx.accounts.seller.to_account_info(),
                },
                vault_signer,
            ),
            net_sol_out,
        )?;
    }
    if creator_share > 0 {
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.curve_vault.to_account_info(),
                    to: ctx.accounts.creator_wallet.to_account_info(),
                },
                vault_signer,
            ),
            creator_share,
        )?;
    }
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

    // === 5. Update curve state ===
    launch.real_sol_reserves = launch
        .real_sol_reserves
        .checked_sub(gross_sol_out)
        .ok_or(MemecoinError::MathUnderflow)?;
    launch.real_token_reserves = launch
        .real_token_reserves
        .checked_add(token_amount_in)
        .ok_or(MemecoinError::MathOverflow)?;
    // Virtual token reserves grow as tokens return to the curve.
    launch.virtual_token_reserves = launch
        .virtual_token_reserves
        .checked_add(token_amount_in)
        .ok_or(MemecoinError::MathOverflow)?;
    // tokens_sold tracks cumulative *net*; we subtract sells so it reflects circulating supply
    launch.tokens_sold = launch.tokens_sold.saturating_sub(token_amount_in);

    emit!(TradeExecuted {
        mint: launch.mint,
        trader: ctx.accounts.seller.key(),
        is_buy: false,
        sol_amount: gross_sol_out,
        token_amount: token_amount_in,
        fee_paid: fee_u64,
        creator_share,
        dino_share,
        treasury_share,
        new_real_sol_reserves: launch.real_sol_reserves,
        new_real_token_reserves: launch.real_token_reserves,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}


