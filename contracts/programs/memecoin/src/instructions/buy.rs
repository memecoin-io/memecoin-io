use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::errors::MemecoinError;
use crate::events::TradeExecuted;
use crate::state::{Config, TokenLaunch};
use crate::utils::{apply_fee, sol_in_for_tokens_out, split_three_way};

#[derive(Accounts)]
pub struct Buy<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

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

    /// CHECK: PDA, validated by seeds. Holds SOL deposited into the curve.
    #[account(
        mut,
        seeds = [TokenLaunch::VAULT_SEED, mint.key().as_ref()],
        bump = launch.vault_bump,
    )]
    pub curve_vault: UncheckedAccount<'info>,

    /// The curve's token reserves — owned by the launch PDA.
    #[account(
        mut,
        token::mint = mint,
        token::authority = launch,
    )]
    pub curve_token_account: Box<Account<'info, TokenAccount>>,

    /// Buyer's token account — created beforehand off-chain via ATA.
    #[account(
        mut,
        token::mint = mint,
        token::authority = buyer,
    )]
    pub buyer_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: receives creator share. Validated to match launch.creator.
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

/// Buy tokens with exact-out semantics: buyer specifies tokens_out, caps SOL via max_sol_in.
///
/// Flow:
///   1. Compute SOL required for tokens_out using bonding curve
///   2. Compute 1% fee on that SOL amount
///   3. Buyer pays (curve_sol + fee) — reverts if > max_sol_in
///   4. curve_sol → curve_vault (PDA), increases real_sol_reserves
///   5. fee split 33.33% creator / 33.33% DINO / 33.33% treasury (treasury gets dust)
///   6. Tokens transferred from curve_token_account → buyer
pub fn handler(ctx: Context<Buy>, token_amount_out: u64, max_sol_in: u64) -> Result<()> {
    require!(!ctx.accounts.config.paused, MemecoinError::Paused);
    require!(!ctx.accounts.launch.graduated, MemecoinError::AlreadyGraduated);
    require!(token_amount_out > 0, MemecoinError::ZeroBuyAmount);

    let config = &ctx.accounts.config;
    let launch = &mut ctx.accounts.launch;

    // === 1. Bonding curve quote ===
    let sol_to_curve = sol_in_for_tokens_out(
        launch.virtual_sol_reserves,
        launch.virtual_token_reserves,
        launch.real_sol_reserves,
        launch.real_token_reserves,
        token_amount_out,
    )
    .map_err(|e| error!(e))?;

    // === 2. Fee on curve SOL amount ===
    // Fee is applied on top: total cost = curve_sol + fee.
    let fee = (sol_to_curve as u128)
        .checked_mul(config.trade_fee_bps as u128)
        .ok_or(MemecoinError::MathOverflow)?
        / 10_000;
    let fee_u64: u64 = fee.try_into().map_err(|_| MemecoinError::MathOverflow)?;
    let total_cost = sol_to_curve
        .checked_add(fee_u64)
        .ok_or(MemecoinError::MathOverflow)?;

    require!(total_cost <= max_sol_in, MemecoinError::SlippageExceeded);

    // === 3. Transfer curve SOL → curve_vault ===
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.curve_vault.to_account_info(),
            },
        ),
        sol_to_curve,
    )?;

    // === 4. Split fee three ways ===
    let (creator_share, dino_share, treasury_share) = split_three_way(
        fee_u64,
        config.trade_split_creator_bps,
        config.trade_split_dino_bps,
        config.trade_split_treasury_bps,
    )
    .map_err(|e| error!(e))?;

    if creator_share > 0 {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.creator_wallet.to_account_info(),
                },
            ),
            creator_share,
        )?;
    }
    if dino_share > 0 {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.dino_buyback_wallet.to_account_info(),
                },
            ),
            dino_share,
        )?;
    }
    if treasury_share > 0 {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.treasury_wallet.to_account_info(),
                },
            ),
            treasury_share,
        )?;
    }

    // === 5. Transfer tokens from curve → buyer ===
    let mint_key = launch.mint;
    let launch_seeds: &[&[u8]] = &[
        TokenLaunch::LAUNCH_SEED,
        mint_key.as_ref(),
        &[launch.bump],
    ];
    let signer = &[launch_seeds];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.curve_token_account.to_account_info(),
                to: ctx.accounts.buyer_token_account.to_account_info(),
                authority: launch.to_account_info(),
            },
            signer,
        ),
        token_amount_out,
    )?;

    // === 6. Update curve state ===
    launch.real_sol_reserves = launch
        .real_sol_reserves
        .checked_add(sol_to_curve)
        .ok_or(MemecoinError::MathOverflow)?;

    // Beta phase hard cap on per-launch SOL exposure.
    require!(
        launch.real_sol_reserves <= TokenLaunch::MAX_REAL_SOL_RESERVES,
        MemecoinError::BetaCapExceeded
    );
    launch.real_token_reserves = launch
        .real_token_reserves
        .checked_sub(token_amount_out)
        .ok_or(MemecoinError::MathUnderflow)?;
    // Virtual token reserves track curve-side liquidity and decrease as tokens leave.
    launch.virtual_token_reserves = launch
        .virtual_token_reserves
        .checked_sub(token_amount_out)
        .ok_or(MemecoinError::MathUnderflow)?;
    launch.tokens_sold = launch
        .tokens_sold
        .checked_add(token_amount_out)
        .ok_or(MemecoinError::MathOverflow)?;

    emit!(TradeExecuted {
        mint: launch.mint,
        trader: ctx.accounts.buyer.key(),
        is_buy: true,
        sol_amount: sol_to_curve,
        token_amount: token_amount_out,
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
