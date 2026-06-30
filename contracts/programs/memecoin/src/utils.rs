//! Bonding curve math and fee calculation utilities.
//!
//! The curve is constant-product:  x * y = k
//!   x = virtual_sol_reserves + real_sol_reserves
//!   y = virtual_token_reserves
//!
//! `virtual_token_reserves` is mutated on every trade — decremented on buys,
//! incremented on sells. This means buyers see the same SOL/token price
//! regardless of how they split their orders.
//!
//! Price at any moment = x / y (SOL per token).
//!
//! Buy: trader sends Δsol, receives Δtokens such that (x + Δsol)(y - Δtokens) = k
//!   →  Δtokens = y - k / (x + Δsol)
//!
//! Sell: trader returns Δtokens, receives Δsol such that (x - Δsol)(y + Δtokens) = k
//!   →  Δsol = x - k / (y + Δtokens)
//!
//! All math uses u128 internally to prevent overflow; downcast to u64 with checked ops.

use crate::errors::MemecoinError;

/// Compute tokens received for a given SOL input (buy direction).
///
/// Returns `Err(InsufficientReserves)` if the resulting token output would
/// exceed available real_token_reserves (i.e. would overshoot the curve).
pub fn tokens_out_for_sol_in(
    virtual_sol: u64,
    virtual_tokens: u64,
    real_sol: u64,
    real_token_reserves: u64,
    sol_in: u64,
) -> Result<u64, MemecoinError> {
    if sol_in == 0 {
        return Err(MemecoinError::ZeroBuyAmount);
    }

    let x = (virtual_sol as u128)
        .checked_add(real_sol as u128)
        .ok_or(MemecoinError::MathOverflow)?;
    let y = virtual_tokens as u128;
    let k = x.checked_mul(y).ok_or(MemecoinError::MathOverflow)?;

    let new_x = x
        .checked_add(sol_in as u128)
        .ok_or(MemecoinError::MathOverflow)?;
    let new_y = k.checked_div(new_x).ok_or(MemecoinError::MathOverflow)?;

    let tokens_out = y.checked_sub(new_y).ok_or(MemecoinError::MathUnderflow)?;

    // Downcast to u64 safely
    let tokens_out_u64: u64 = tokens_out
        .try_into()
        .map_err(|_| MemecoinError::MathOverflow)?;

    if tokens_out_u64 > real_token_reserves {
        return Err(MemecoinError::InsufficientReserves);
    }

    Ok(tokens_out_u64)
}

/// Compute SOL required for an exact token output (buy with exact-out semantics).
/// Useful for slippage: user says "I want exactly N tokens, pay up to max_sol_in."
pub fn sol_in_for_tokens_out(
    virtual_sol: u64,
    virtual_tokens: u64,
    real_sol: u64,
    real_token_reserves: u64,
    tokens_out: u64,
) -> Result<u64, MemecoinError> {
    if tokens_out == 0 {
        return Err(MemecoinError::ZeroBuyAmount);
    }
    if tokens_out > real_token_reserves {
        return Err(MemecoinError::InsufficientReserves);
    }

    let x = (virtual_sol as u128)
        .checked_add(real_sol as u128)
        .ok_or(MemecoinError::MathOverflow)?;
    let y = virtual_tokens as u128;
    let k = x.checked_mul(y).ok_or(MemecoinError::MathOverflow)?;

    let new_y = y
        .checked_sub(tokens_out as u128)
        .ok_or(MemecoinError::MathUnderflow)?;

    // new_x = ceil(k / new_y) — round up so the curve never gives away dust
    let new_x = k
        .checked_add(new_y.checked_sub(1).ok_or(MemecoinError::MathUnderflow)?)
        .ok_or(MemecoinError::MathOverflow)?
        .checked_div(new_y)
        .ok_or(MemecoinError::MathOverflow)?;

    let sol_in = new_x.checked_sub(x).ok_or(MemecoinError::MathUnderflow)?;

    sol_in.try_into().map_err(|_| MemecoinError::MathOverflow)
}

/// Compute SOL received for a given token input (sell direction).
pub fn sol_out_for_tokens_in(
    virtual_sol: u64,
    virtual_tokens: u64,
    real_sol: u64,
    tokens_in: u64,
) -> Result<u64, MemecoinError> {
    if tokens_in == 0 {
        return Err(MemecoinError::ZeroSellAmount);
    }

    let x = (virtual_sol as u128)
        .checked_add(real_sol as u128)
        .ok_or(MemecoinError::MathOverflow)?;
    let y = virtual_tokens as u128;
    let k = x.checked_mul(y).ok_or(MemecoinError::MathOverflow)?;

    let new_y = y
        .checked_add(tokens_in as u128)
        .ok_or(MemecoinError::MathOverflow)?;
    let new_x = k.checked_div(new_y).ok_or(MemecoinError::MathOverflow)?;

    let sol_out = x.checked_sub(new_x).ok_or(MemecoinError::MathUnderflow)?;

    // Cap at real_sol — we never pay out more than the curve actually holds
    let sol_out_u64: u64 = sol_out
        .try_into()
        .map_err(|_| MemecoinError::MathOverflow)?;

    Ok(sol_out_u64.min(real_sol))
}

/// Split a gross amount into (creator, dino, treasury) shares using bps.
/// Treasury receives the rounding remainder.
pub fn split_three_way(
    amount: u64,
    creator_bps: u16,
    dino_bps: u16,
    _treasury_bps: u16,
) -> Result<(u64, u64, u64), MemecoinError> {
    let amount_u128 = amount as u128;

    let creator = amount_u128
        .checked_mul(creator_bps as u128)
        .ok_or(MemecoinError::MathOverflow)?
        / 10_000;

    let dino = amount_u128
        .checked_mul(dino_bps as u128)
        .ok_or(MemecoinError::MathOverflow)?
        / 10_000;

    let creator_u64: u64 = creator.try_into().map_err(|_| MemecoinError::MathOverflow)?;
    let dino_u64: u64 = dino.try_into().map_err(|_| MemecoinError::MathOverflow)?;

    // Treasury gets the remainder so the three shares sum to exactly `amount`
    let treasury_u64 = amount
        .checked_sub(creator_u64)
        .ok_or(MemecoinError::MathUnderflow)?
        .checked_sub(dino_u64)
        .ok_or(MemecoinError::MathUnderflow)?;

    Ok((creator_u64, dino_u64, treasury_u64))
}

/// Split a gross amount 50/50 (or per `dino_bps`) between DINO and treasury.
pub fn split_two_way(
    amount: u64,
    dino_bps: u16,
) -> Result<(u64, u64), MemecoinError> {
    let dino = (amount as u128)
        .checked_mul(dino_bps as u128)
        .ok_or(MemecoinError::MathOverflow)?
        / 10_000;
    let dino_u64: u64 = dino.try_into().map_err(|_| MemecoinError::MathOverflow)?;
    let treasury = amount
        .checked_sub(dino_u64)
        .ok_or(MemecoinError::MathUnderflow)?;
    Ok((dino_u64, treasury))
}

/// Apply trade fee to a gross SOL amount.
/// Returns (fee, net_after_fee).
pub fn apply_fee(amount: u64, fee_bps: u16) -> Result<(u64, u64), MemecoinError> {
    let fee = (amount as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(MemecoinError::MathOverflow)?
        / 10_000;
    let fee_u64: u64 = fee.try_into().map_err(|_| MemecoinError::MathOverflow)?;
    let net = amount
        .checked_sub(fee_u64)
        .ok_or(MemecoinError::MathUnderflow)?;
    Ok((fee_u64, net))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buy_then_sell_roundtrip_is_lossless_ignoring_fees() {
        let vsol = 30_000_000_000u64;
        let vtok = 1_073_000_000u64;
        let real_sol = 0u64;
        let real_tok = 800_000_000u64;

        let sol_in = 1_000_000_000u64; // 1 SOL
        let tokens_out =
            tokens_out_for_sol_in(vsol, vtok, real_sol, real_tok, sol_in).unwrap();

        // After buying, sell the same amount back — should get ≈ sol_in
        let sol_back = sol_out_for_tokens_in(vsol, vtok, real_sol + sol_in, tokens_out).unwrap();

        // Allow 1 lamport rounding
        let diff = sol_in.abs_diff(sol_back);
        assert!(diff <= 1, "round-trip diff = {} lamports", diff);
    }

    #[test]
    fn fee_split_sums_to_amount() {
        let (c, d, t) = split_three_way(10_000, 3333, 3333, 3334).unwrap();
        assert_eq!(c + d + t, 10_000);
        // Treasury gets the dust
        assert!(t >= c && t >= d);
    }

    #[test]
    fn split_two_way_sums_to_amount() {
        let (d, t) = split_two_way(1_000_000, 5000).unwrap();
        assert_eq!(d + t, 1_000_000);
        assert_eq!(d, t);
    }

    #[test]
    fn apply_fee_one_percent() {
        let (fee, net) = apply_fee(1_000_000, 100).unwrap();
        assert_eq!(fee, 10_000);
        assert_eq!(net, 990_000);
    }

    #[test]
    fn price_increases_with_buys() {
        let vsol = 30_000_000_000u64;
        let vtok = 1_073_000_000u64;
        let real_tok = 800_000_000u64;

        let t1 = tokens_out_for_sol_in(vsol, vtok, 0, real_tok, 1_000_000_000).unwrap();
        let t2 = tokens_out_for_sol_in(vsol, vtok, 10_000_000_000, real_tok, 1_000_000_000)
            .unwrap();

        // Same SOL input later in curve should yield fewer tokens
        assert!(t2 < t1, "price should rise: t1={}, t2={}", t1, t2);
    }
}
