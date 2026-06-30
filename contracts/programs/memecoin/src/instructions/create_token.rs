use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};

use crate::errors::MemecoinError;
use crate::events::TokenCreated;
use crate::state::{Config, TokenLaunch};
use crate::utils::split_two_way;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateTokenParams {
    pub name: String,
    pub symbol: String,
    /// Off-chain metadata URI (Arweave / IPFS / S3) — image, description, socials.
    pub uri: String,
}

#[derive(Accounts)]
#[instruction(params: CreateTokenParams)]
pub struct CreateToken<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        seeds = [Config::SEED],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, Config>>,

    /// CHECK: validated via `address = config.dino_buyback_wallet`
    #[account(
        mut,
        address = config.dino_buyback_wallet @ MemecoinError::InvalidWallet,
    )]
    pub dino_buyback_wallet: UncheckedAccount<'info>,

    /// CHECK: validated via `address = config.treasury_wallet`
    #[account(
        mut,
        address = config.treasury_wallet @ MemecoinError::InvalidWallet,
    )]
    pub treasury_wallet: UncheckedAccount<'info>,

    /// New token mint — created here. PDA-controlled mint authority.
    #[account(
        init,
        payer = creator,
        mint::decimals = TokenLaunch::DECIMALS,
        mint::authority = mint_authority,
    )]
    pub mint: Box<Account<'info, Mint>>,

    /// CHECK: PDA, signs mint instructions for this launch.
    #[account(
        seeds = [TokenLaunch::MINT_AUTHORITY_SEED, mint.key().as_ref()],
        bump,
    )]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = creator,
        space = 8 + TokenLaunch::INIT_SPACE,
        seeds = [TokenLaunch::LAUNCH_SEED, mint.key().as_ref()],
        bump,
    )]
    pub launch: Box<Account<'info, TokenLaunch>>,

    /// CHECK: PDA SOL escrow for this launch's bonding curve.
    #[account(
        mut,
        seeds = [TokenLaunch::VAULT_SEED, mint.key().as_ref()],
        bump,
    )]
    pub curve_vault: UncheckedAccount<'info>,

    /// Token account owned by the launch PDA, holds the curve's token supply.
    #[account(
        init,
        payer = creator,
        associated_token::mint = mint,
        associated_token::authority = launch,
    )]
    pub curve_token_account: Box<Account<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<CreateToken>, params: CreateTokenParams) -> Result<()> {
    require!(!ctx.accounts.config.paused, MemecoinError::Paused);
    require!(params.name.len() <= 32, MemecoinError::NameTooLong);
    require!(params.symbol.len() <= 10, MemecoinError::SymbolTooLong);
    require!(params.uri.len() <= 200, MemecoinError::UriTooLong);

    let config = &ctx.accounts.config;
    let creator = &ctx.accounts.creator;

    // === Pay creation fee, split DINO/treasury ===
    let (dino_share, treasury_share) =
        split_two_way(config.creation_fee_lamports, config.creation_split_dino_bps)
            .map_err(|e| error!(e))?;

    if dino_share > 0 {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: creator.to_account_info(),
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
                    from: creator.to_account_info(),
                    to: ctx.accounts.treasury_wallet.to_account_info(),
                },
            ),
            treasury_share,
        )?;
    }

    // === Mint full curve allocation (800M) to the launch PDA's token account ===
    // Note: token amounts use 6 decimals → 800M tokens = 800_000_000 * 10^6 base units
    let mint_amount = TokenLaunch::CURVE_ALLOCATION
        .checked_mul(10u64.pow(TokenLaunch::DECIMALS as u32))
        .ok_or(MemecoinError::MathOverflow)?;

    let mint_key = ctx.accounts.mint.key();
    let mint_authority_seeds: &[&[u8]] = &[
        TokenLaunch::MINT_AUTHORITY_SEED,
        mint_key.as_ref(),
        &[ctx.bumps.mint_authority],
    ];
    let signer_seeds = &[mint_authority_seeds];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.curve_token_account.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
            signer_seeds,
        ),
        mint_amount,
    )?;

    // === Initialize TokenLaunch state ===
    let launch = &mut ctx.accounts.launch;
    launch.mint = ctx.accounts.mint.key();
    launch.creator = creator.key();
    launch.created_at = Clock::get()?.unix_timestamp;
    launch.real_sol_reserves = 0;
    launch.real_token_reserves = mint_amount;
    launch.virtual_sol_reserves = TokenLaunch::DEFAULT_VIRTUAL_SOL;
    launch.virtual_token_reserves = TokenLaunch::DEFAULT_VIRTUAL_TOKENS
        .checked_mul(10u64.pow(TokenLaunch::DECIMALS as u32))
        .ok_or(MemecoinError::MathOverflow)?;
    launch.tokens_sold = 0;
    launch.graduated = false;
    launch.pool_seeded = false;
    launch.meteora_lb_pair = Pubkey::default();
    launch.graduation_sol_lamports = 0;
    launch.graduation_token_amount = 0;
    launch.bump = ctx.bumps.launch;
    launch.vault_bump = ctx.bumps.curve_vault;
    launch.mint_authority_bump = ctx.bumps.mint_authority;
    launch._reserved = [0u8; 64];

    emit!(TokenCreated {
        mint: launch.mint,
        creator: launch.creator,
        name: params.name,
        symbol: params.symbol,
        uri: params.uri,
        creation_fee_paid: config.creation_fee_lamports,
        dino_share,
        treasury_share,
        timestamp: launch.created_at,
    });

    Ok(())
}
