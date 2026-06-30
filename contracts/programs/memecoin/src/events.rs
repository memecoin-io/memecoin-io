use anchor_lang::prelude::*;

/// Emitted when admin initializes the program config.
#[event]
pub struct ConfigInitialized {
    pub admin: Pubkey,
    pub dino_buyback_wallet: Pubkey,
    pub treasury_wallet: Pubkey,
}

/// Emitted when admin updates any config field.
#[event]
pub struct ConfigUpdated {
    pub admin: Pubkey,
}

/// Emitted when a new token is launched.
#[event]
pub struct TokenCreated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub creation_fee_paid: u64,
    pub dino_share: u64,
    pub treasury_share: u64,
    pub timestamp: i64,
}

/// Emitted on every buy from the bonding curve.
#[event]
pub struct TradeExecuted {
    pub mint: Pubkey,
    pub trader: Pubkey,
    pub is_buy: bool,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub fee_paid: u64,
    pub creator_share: u64,
    pub dino_share: u64,
    pub treasury_share: u64,
    pub new_real_sol_reserves: u64,
    pub new_real_token_reserves: u64,
    pub timestamp: i64,
}

/// Emitted when a token graduates: curve frozen, fee paid, SOL + reserve
/// tokens parked with the graduation authority for off-chain pool seeding.
#[event]
pub struct TokenGraduated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub final_sol_reserves: u64,
    pub final_token_reserves: u64,
    pub graduation_fee_paid: u64,
    pub dino_share: u64,
    pub treasury_share: u64,
    /// Where the off-chain crank will receive the SOL + tokens.
    pub graduation_authority: Pubkey,
    pub timestamp: i64,
}

/// Emitted when the off-chain graduation authority confirms the Meteora
/// DLMM pool is live + seeded. From this point the launch is fully
/// migrated and trades happen on the DLMM pool, not our program.
#[event]
pub struct PoolSeeded {
    pub mint: Pubkey,
    pub meteora_lb_pair: Pubkey,
    pub timestamp: i64,
}
