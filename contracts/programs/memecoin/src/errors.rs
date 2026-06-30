use anchor_lang::prelude::*;

#[error_code]
pub enum MemecoinError {
    #[msg("Program is paused")]
    Paused,

    #[msg("Unauthorized — admin only")]
    Unauthorized,

    #[msg("Invalid fee configuration — splits must sum to 10000 bps")]
    InvalidFeeSplit,

    #[msg("Fee basis points exceed maximum (1000 = 10%)")]
    FeeBpsTooHigh,

    #[msg("Token has already graduated — trading on curve disabled")]
    AlreadyGraduated,

    #[msg("Token has not reached graduation threshold")]
    NotReadyToGraduate,

    #[msg("Slippage exceeded — SOL required is more than max_sol_in")]
    SlippageExceeded,

    #[msg("Slippage exceeded — SOL returned is less than min_sol_out")]
    InsufficientSolOut,

    #[msg("Insufficient token reserves on curve")]
    InsufficientReserves,

    #[msg("Math overflow")]
    MathOverflow,

    #[msg("Math underflow")]
    MathUnderflow,

    #[msg("Buy amount must be greater than zero")]
    ZeroBuyAmount,

    #[msg("Sell amount must be greater than zero")]
    ZeroSellAmount,

    #[msg("Token name too long (max 32 bytes)")]
    NameTooLong,

    #[msg("Token symbol too long (max 10 bytes)")]
    SymbolTooLong,

    #[msg("Metadata URI too long (max 200 bytes)")]
    UriTooLong,

    #[msg("Invalid wallet — cannot be system program or zero")]
    InvalidWallet,

    #[msg("Graduation threshold must be greater than zero")]
    InvalidThreshold,

    #[msg("Token has not graduated yet")]
    NotGraduated,

    #[msg("Meteora pool already seeded for this token")]
    PoolAlreadySeeded,

    #[msg("Invalid Meteora pool address")]
    InvalidPoolAddress,

    #[msg("Beta cap exceeded - real_sol_reserves would exceed MAX_REAL_SOL_RESERVES")]
    BetaCapExceeded,
}
