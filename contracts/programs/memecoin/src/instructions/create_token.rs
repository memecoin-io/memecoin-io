use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::metadata::{
    create_metadata_accounts_v3, mpl_token_metadata::types::DataV2, CreateMetadataAccountsV3,
    Metadata,
};
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
    /// Bump for the mint_authority PDA (Fix 1: moved into params to keep the
    /// entire instruction-arg block on the heap via Box<CreateTokenParams>).
    pub mint_authority_bump: u8,
}

/// # Fix 1 stack-fix — remaining_accounts layout
///
/// Several accounts that were previously named fields on this struct were
/// moved to `ctx.remaining_accounts` to shrink `try_accounts`'s SBF stack
/// frame below Solana's 4 KB limit. Callers MUST pass them in this exact
/// order:
///
///   [0] dino_buyback_wallet        (writable, == config.dino_buyback_wallet)
///   [1] treasury_wallet            (writable, == config.treasury_wallet)
///   [2] metadata                   (writable, Metaplex metadata PDA for mint)
///   [3] token_metadata_program     (Metaplex Token Metadata program)
///   [4] rent sysvar                (SysvarRent111111111111111111111111111111111)
///   [5] mint_authority             (PDA ["mint_authority", mint])
///
/// Note: `curve_vault` (PDA ["vault", mint]) is derived on-chain via
/// `Pubkey::find_program_address` — no account info needed at
/// creation time (nothing is written to it here).
///
/// All are validated inside `handler` before use.
#[derive(Accounts)]
#[instruction(params: Box<CreateTokenParams>)]
pub struct CreateToken<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        seeds = [Config::SEED],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, Config>>,

    /// New token mint — created here. PDA-controlled mint authority
    /// (`mint_authority` is passed via remaining_accounts — see NOTE).
    /// The mint::authority constraint recreates the PDA from the caller-
    /// supplied bump (validated in the handler).
    #[account(
        init,
        payer = creator,
        mint::decimals = TokenLaunch::DECIMALS,
        mint::authority = Pubkey::create_program_address(
            &[TokenLaunch::MINT_AUTHORITY_SEED, mint.key().as_ref(), &[params.mint_authority_bump]],
            &crate::ID
        ).unwrap(),
    )]
    pub mint: Box<Account<'info, Mint>>,

    // mint_authority moved to remaining_accounts (Fix 1 stack).

    #[account(
        init,
        payer = creator,
        space = 8 + TokenLaunch::INIT_SPACE,
        seeds = [TokenLaunch::LAUNCH_SEED, mint.key().as_ref()],
        bump,
    )]
    pub launch: Box<Account<'info, TokenLaunch>>,

    // curve_vault removed from struct — derived in handler (Fix 1 stack).

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
    // See NOTE above the struct: dino_buyback_wallet, treasury_wallet,
    // metadata, token_metadata_program, and rent were moved to
    // ctx.remaining_accounts to shrink the try_accounts stack frame.
}

pub fn handler<'info>(
    ctx: Context<'_, '_, '_, 'info, CreateToken<'info>>,
    params: Box<CreateTokenParams>,
) -> Result<()> {
    require!(!ctx.accounts.config.paused, MemecoinError::Paused);
    require!(params.name.len() <= 32, MemecoinError::NameTooLong);
    require!(params.symbol.len() <= 10, MemecoinError::SymbolTooLong);
    require!(params.uri.len() <= 200, MemecoinError::UriTooLong);

    // === Pull remaining_accounts (see struct-level NOTE for layout) ===
    require!(
        ctx.remaining_accounts.len() >= 6,
        MemecoinError::MissingMetadataAccounts
    );
    let dino_buyback_wallet_ai = &ctx.remaining_accounts[0];
    let treasury_wallet_ai = &ctx.remaining_accounts[1];
    let metadata_ai = &ctx.remaining_accounts[2];
    let token_metadata_program_ai = &ctx.remaining_accounts[3];
    let rent_ai = &ctx.remaining_accounts[4];
    let mint_authority_ai = &ctx.remaining_accounts[5];

    // --- Validate mint_authority PDA and derive its bump ---
    let (expected_mint_authority_key, mint_authority_bump) = Pubkey::find_program_address(
        &[TokenLaunch::MINT_AUTHORITY_SEED, ctx.accounts.mint.key().as_ref()],
        ctx.program_id,
    );
    require_keys_eq!(
        mint_authority_ai.key(),
        expected_mint_authority_key,
        MemecoinError::InvalidWallet
    );

    // --- validate wallets against config (mirrors the previous
    //     `#[account(address = config.*_wallet)]` constraints) ---
    require_keys_eq!(
        dino_buyback_wallet_ai.key(),
        ctx.accounts.config.dino_buyback_wallet,
        MemecoinError::InvalidWallet
    );
    require!(
        dino_buyback_wallet_ai.is_writable,
        MemecoinError::InvalidWallet
    );
    require_keys_eq!(
        treasury_wallet_ai.key(),
        ctx.accounts.config.treasury_wallet,
        MemecoinError::InvalidWallet
    );
    require!(treasury_wallet_ai.is_writable, MemecoinError::InvalidWallet);

    // --- validate token_metadata_program is the real Metaplex program ---
    require_keys_eq!(
        token_metadata_program_ai.key(),
        Metadata::id(),
        MemecoinError::InvalidMetadataProgram
    );
    require!(
        token_metadata_program_ai.executable,
        MemecoinError::InvalidMetadataProgram
    );

    // --- validate metadata PDA derivation: ["metadata", MPL_ID, mint] ---
    let mint_key = ctx.accounts.mint.key();
    let mpl_id = Metadata::id();
    let (expected_metadata_pda, _bump) = Pubkey::find_program_address(
        &[b"metadata", mpl_id.as_ref(), mint_key.as_ref()],
        &mpl_id,
    );
    require_keys_eq!(
        metadata_ai.key(),
        expected_metadata_pda,
        MemecoinError::InvalidMetadataAccount
    );
    require!(
        metadata_ai.is_writable,
        MemecoinError::InvalidMetadataAccount
    );

    // --- validate rent sysvar ---
    require_keys_eq!(
        rent_ai.key(),
        anchor_lang::solana_program::sysvar::rent::ID,
        MemecoinError::InvalidRentSysvar
    );

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
                    to: dino_buyback_wallet_ai.to_account_info(),
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
                    to: treasury_wallet_ai.to_account_info(),
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

    let mint_authority_seeds: &[&[u8]] = &[
        TokenLaunch::MINT_AUTHORITY_SEED,
        mint_key.as_ref(),
        &[mint_authority_bump],
    ];
    let signer_seeds = &[mint_authority_seeds];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.curve_token_account.to_account_info(),
                authority: mint_authority_ai.to_account_info(),
            },
            signer_seeds,
        ),
        mint_amount,
    )?;

    // === Create Metaplex on-chain metadata (name / symbol / uri) ===
    // The mint_authority PDA is both the mint authority and the update
    // authority, so the same seeds sign the CreateMetadataAccountV3 CPI.
    // is_mutable = true keeps the uri updatable later; update_authority is a
    // signer so the graduation-authority pattern is unaffected.
    create_metadata_accounts_v3(
        CpiContext::new_with_signer(
            token_metadata_program_ai.to_account_info(),
            CreateMetadataAccountsV3 {
                metadata: metadata_ai.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                mint_authority: mint_authority_ai.to_account_info(),
                payer: ctx.accounts.creator.to_account_info(),
                update_authority: mint_authority_ai.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                rent: rent_ai.to_account_info(),
            },
            signer_seeds,
        ),
        DataV2 {
            name: params.name.clone(),
            symbol: params.symbol.clone(),
            uri: params.uri.clone(),
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        },
        true,
        true,
        None,
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
    // Derive curve_vault PDA to record its bump — the account itself is not
    // needed at token creation (SOL escrow is only written to on buy/sell).
    let (_curve_vault_key, curve_vault_bump) = Pubkey::find_program_address(
        &[TokenLaunch::VAULT_SEED, mint_key.as_ref()],
        ctx.program_id,
    );
    launch.vault_bump = curve_vault_bump;
    launch.mint_authority_bump = mint_authority_bump;
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
