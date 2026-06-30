use anchor_lang::prelude::*;

/// State for a single launched token. One PDA per token mint.
/// Seeds: [b"launch", mint.key().as_ref()]
#[account]
#[derive(InitSpace)]
pub struct TokenLaunch {
    /// SPL token mint this launch governs.
    pub mint: Pubkey,

    /// Wallet that launched this token. Receives creator share of trade fees.
    pub creator: Pubkey,

    /// Unix timestamp of creation.
    pub created_at: i64,

    // === Bonding curve state ===
    /// Real SOL currently held in the curve vault.
    pub real_sol_reserves: u64,

    /// Tokens currently available on the curve (decreases as people buy).
    pub real_token_reserves: u64,

    /// Virtual SOL — bootstraps the price so curve doesn't start at zero.
    /// Default: 30 SOL (in lamports).
    pub virtual_sol_reserves: u64,

    /// Virtual tokens — paired with virtual SOL to set starting price.
    /// Default: 1,073,000,000 tokens (in base units).
    pub virtual_token_reserves: u64,

    /// Total tokens sold from the curve (lifetime).
    pub tokens_sold: u64,

    /// True once `graduate` has been successfully called.
    /// While false: trading on the curve via buy/sell.
    /// Once true: trading on Meteora DLMM, curve is frozen and the
    /// graduation authority holds the SOL + 200M reserve tokens.
    pub graduated: bool,

    /// True once `mark_pool_seeded` has been called by the graduation
    /// authority. Confirms that the off-chain Meteora DLMM pool is live
    /// and seeded with our reserve liquidity.
    pub pool_seeded: bool,

    /// Meteora DLMM pool address once `mark_pool_seeded` confirms it.
    /// Zero until then.
    pub meteora_lb_pair: Pubkey,

    /// SOL forwarded to the graduation authority at graduate-time, for the
    /// off-chain bundle to deposit into the DLMM pool. Snapshot for audit.
    pub graduation_sol_lamports: u64,

    /// Tokens forwarded to the graduation authority at graduate-time.
    /// Always GRADUATION_RESERVE * 10^DECIMALS. Snapshot for audit.
    pub graduation_token_amount: u64,

    /// Bump for the TokenLaunch PDA.
    pub bump: u8,

    /// Bump for the CurveVault PDA (SOL escrow).
    pub vault_bump: u8,

    /// Bump for the mint authority PDA.
    pub mint_authority_bump: u8,

    /// Reserved space for future fields.
    pub _reserved: [u8; 64],
}

impl TokenLaunch {
    pub const LAUNCH_SEED: &'static [u8] = b"launch";
    pub const VAULT_SEED: &'static [u8] = b"vault";
    pub const MINT_AUTHORITY_SEED: &'static [u8] = b"mint_authority";

    /// Initial supply of every launched token: 1,000,000,000 (1B).
    pub const TOTAL_SUPPLY: u64 = 1_000_000_000;

    /// Tokens available on the bonding curve: 800,000,000 (80%).
    pub const CURVE_ALLOCATION: u64 = 800_000_000;

    /// Tokens held in reserve for Meteora DLMM at graduation: 200,000,000 (20%).
    pub const GRADUATION_RESERVE: u64 = 200_000_000;

    /// Default virtual SOL reserves (30 SOL in lamports).
    pub const DEFAULT_VIRTUAL_SOL: u64 = 30 * 1_000_000_000;

    /// Default virtual token reserves — sized so the curve sells 100% of
    /// CURVE_ALLOCATION (800M) by the time real_sol = 55 SOL (graduation).
    ///
    /// Derivation:
    ///   At graduation:  (vsol + threshold) * (vtok - CURVE_ALLOCATION) = vsol * vtok
    ///   →  vtok = CURVE_ALLOCATION * (vsol + threshold) / threshold
    ///   →  vtok = 800M * (30 + 55) / 55 = 1,236,363,636 whole tokens
    ///
    /// Stored in base units (×10^DECIMALS).
    pub const DEFAULT_VIRTUAL_TOKENS: u64 = 1_236_363_636;

    /// Token decimals (matches SPL conventions for fungible).
    pub const DECIMALS: u8 = 6;

    /// **Beta phase hard cap on per-launch SOL exposure.**
    ///
    /// Bounds the blast radius of any unknown bug to 5 SOL per launch. Once the
    /// program is audited and battle-tested, raise this in a program upgrade
    /// (no Config field on purpose — it must require a redeploy).
    ///
    /// graduation_threshold_lamports in Config MUST be ≤ MAX_REAL_SOL_RESERVES.
    pub const MAX_REAL_SOL_RESERVES: u64 = 5 * 1_000_000_000; // 5 SOL
}
