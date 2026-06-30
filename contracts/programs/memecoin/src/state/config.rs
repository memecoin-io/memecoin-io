use anchor_lang::prelude::*;

/// Global program configuration. Singleton PDA at seed = b"config".
///
/// All fee parameters are tunable post-launch via `update_config` (admin-only).
/// This is intentional: we will adjust thresholds and splits based on real data
/// in the first 30 days of operation.
#[account]
#[derive(InitSpace)]
pub struct Config {
    /// Admin authority — can update config, pause program, change wallets.
    /// Should be a hardware-wallet-backed keypair, later upgraded to Squads multisig.
    pub admin: Pubkey,

    /// Solana wallet that receives the DINO buyback share of all fees.
    /// Off-chain bot drains this daily, swaps SOL→USDC→bridge→DINO on Ethereum.
    pub dino_buyback_wallet: Pubkey,

    /// Solana wallet that receives the platform treasury share of all fees.
    /// Cold storage. Funds platform ops, marketing, dev, eventual team payroll.
    pub treasury_wallet: Pubkey,

    /// Off-chain graduation authority — the only signer allowed to claim
    /// graduated liquidity and mark a pool seeded. Runs the Meteora
    /// `initialize_customizable_permissionless_lb_pair2` + seed-liquidity
    /// bundle in a single transaction off-chain, then comes back here to
    /// mark the launch sealed.
    ///
    /// In production this is a hot wallet managed by the same operator that
    /// runs the indexer + buyback bot. Rotatable by admin via update_config.
    pub graduation_authority: Pubkey,

    // === Trade fee (1% default) ===
    /// Total trade fee in basis points (10000 = 100%). Default: 100 = 1%.
    pub trade_fee_bps: u16,
    /// Of the trade fee, share going to the token creator. Default: 3333.
    pub trade_split_creator_bps: u16,
    /// Of the trade fee, share going to DINO buyback. Default: 3333.
    pub trade_split_dino_bps: u16,
    /// Of the trade fee, share going to platform treasury. Default: 3334.
    /// (Gets rounding dust — 3333 + 3333 + 3334 = 10000.)
    pub trade_split_treasury_bps: u16,

    // === Creation fee (0.02 SOL default) ===
    pub creation_fee_lamports: u64,
    /// 50/50 split DINO/treasury. Stored as DINO share; treasury = 10000 - this.
    pub creation_split_dino_bps: u16,

    // === Graduation fee (1 SOL default) ===
    pub graduation_fee_lamports: u64,
    pub graduation_split_dino_bps: u16,

    // === Bonding curve & graduation ===
    /// Real SOL deposited that triggers graduation. Default: 55 SOL.
    pub graduation_threshold_lamports: u64,

    /// Emergency pause — disables create_token, buy, sell, graduate.
    pub paused: bool,

    /// Bump seed for this PDA.
    pub bump: u8,

    /// Reserved space for future fields without migration.
    pub _reserved: [u8; 96],
}

impl Config {
    pub const SEED: &'static [u8] = b"config";

    /// Validate that fee splits are internally consistent.
    pub fn validate(&self) -> std::result::Result<(), crate::errors::MemecoinError> {
        use crate::errors::MemecoinError;

        // Trade fee can't exceed 10%
        if self.trade_fee_bps > 1000 {
            return Err(MemecoinError::FeeBpsTooHigh);
        }

        // Trade splits must sum to exactly 10000
        let trade_sum = self.trade_split_creator_bps as u32
            + self.trade_split_dino_bps as u32
            + self.trade_split_treasury_bps as u32;
        if trade_sum != 10_000 {
            return Err(MemecoinError::InvalidFeeSplit);
        }

        // Creation/graduation splits must be <= 10000
        if self.creation_split_dino_bps > 10_000 || self.graduation_split_dino_bps > 10_000 {
            return Err(MemecoinError::InvalidFeeSplit);
        }

        if self.graduation_threshold_lamports == 0 {
            return Err(MemecoinError::InvalidThreshold);
        }

        // Beta phase: threshold cannot exceed the per-launch SOL cap.
        if self.graduation_threshold_lamports
            > crate::state::TokenLaunch::MAX_REAL_SOL_RESERVES
        {
            return Err(MemecoinError::BetaCapExceeded);
        }

        if self.graduation_authority == Pubkey::default() {
            return Err(MemecoinError::InvalidWallet);
        }

        Ok(())
    }
}
