# Bonding Curve Diff — memecoin.io vs pump.fun

Comparison of our v0.5 bonding curve against the publicly documented pump.fun program
(`6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`).

## Same math, different parameters

Both use the Uniswap V2 constant-product invariant `x * y = k` with synthetic ("virtual") reserves added to real reserves for price calculation.

| Aspect | pump.fun | memecoin.io v0.5 |
|---|---|---|
| Formula | `x * y = k` | `x * y = k` (identical) |
| `x` | virtual_sol_reserves + real_sol_reserves | virtual_sol_reserves + real_sol_reserves |
| `y` | virtual_token_reserves | virtual_token_reserves |
| Token decimals | 6 | 6 |
| Total supply | 1,000,000,000 tokens | 1,000,000,000 tokens |

## Numeric parameters

| Parameter | pump.fun | memecoin.io | Notes |
|---|---|---|---|
| initial_virtual_sol_reserves | 30 SOL (30e9 lamports) | **30 SOL** | Same |
| initial_virtual_token_reserves | 1,073,000,000 whole | **1,236,363,636 whole** | Ours: targets graduation at ~55 SOL; pump targets ~85 SOL |
| initial_real_token_reserves (curve allocation) | ~793M | **800M** | Approximate parity |
| graduation reserve (post-curve mint) | ~207M (migrated to PumpSwap LP) | **200M** (held in treasury for Meteora DLMM seeding) | Different post-graduation strategy |
| graduation threshold (real SOL) | ~85 SOL (≈ $69k mcap) | **55 SOL** post-beta (≈ $42k mcap) / **5 SOL** during beta | Lower threshold → faster graduations |
| creation fee | 0.02 SOL | **0.02 SOL** | Same |
| trade fee | 1% | **1%** | Same |
| graduation fee | — | **1 SOL flat** | New: pays for Meteora pool setup |

## Rounding direction

| Operation | pump.fun (from public IDL/SDK) | memecoin.io v0.5 |
|---|---|---|
| Buy: tokens out for given SOL in | `tokens_out = floor((sol_in * vTok) / (vSol + sol_in))` — floor in user's favor | We expose buy as **exact-out**: user requests N tokens, contract computes `new_x = ceil(k / new_y)`, then `sol_in = new_x - x`. Ceiling rounds in the protocol's favor (never dust loss). |
| Sell: SOL out for given tokens in | floor (in protocol's favor) | floor (in protocol's favor) — same direction |

Why the buy difference matters: if pump's floor-rounding tokens_out is off by a few base units, the curve might give away a tiny amount of dust. Our exact-out + ceiling on the SOL side guarantees `k` is never decremented by rounding. For human-scale buys (anything > ~1k base units of tokens), the divergence is invisible.

## Liquidity migration

| Aspect | pump.fun | memecoin.io |
|---|---|---|
| Destination | PumpSwap AMM (their own AMM) | Meteora DLMM (third-party concentrated-liquidity DEX) |
| LP tokens | Burned (permanent lock) | **Held in treasury** (not burned) — recoverable on token unwind |
| Cross-chain | None | DINO buyback bot bridges 33.33% of fees to Ethereum and burns DINO ERC-20 |

## Fee distribution

| Recipient | pump.fun | memecoin.io |
|---|---|---|
| Protocol/treasury | ~1% all to protocol multisig | 33.34% to treasury |
| Creator | 0% (some recent creator-fee instruments via add-on programs) | **33.33% to creator** |
| Token buyback | none | **33.33% to DINO buyback bot** (cross-chain) |

## Beta cap (memecoin.io only)

We add a hardcoded `MAX_REAL_SOL_RESERVES = 5 SOL` constant enforced on every buy. pump.fun has no equivalent cap — they rely on the natural graduation threshold (~85 SOL) as the ceiling. Our cap exists for the beta phase only and will be removed (raised to 55 SOL) once the contracts are audited.

## What we deliberately did NOT clone

- **PumpSwap migration code path** — we use Meteora DLMM directly; the seal_graduation instruction stores the LB pair address but the crank/seeding happens off-chain in our meteora_crank service.
- **No anti-bot rate limiting** — pump.fun has no on-chain rate limit either; both rely on Solana TPS economics.
- **No whitelist gating** during beta — we use the SOL cap instead.

## Conclusion

The mathematical core (x*y=k, ceiling on buy_sol_in, floor on sell_sol_out, dual virtual+real reserves) matches pump.fun's documented behavior exactly. Differences are parameter choices (graduation threshold, virtual reserves) and the fee distribution model. No novel cryptography or unaudited primitives are introduced — the curve math is a well-understood Uniswap V2 derivative that has been audited many times in other contexts (Raydium, Orca, Meteora itself).

Audit-relevant deltas vs known-good code:

1. `MAX_REAL_SOL_RESERVES` cap (lines 207–210 in `buy.rs`) — added this segment, exercised in full E2E test.
2. Vault SOL is stored under the System program owner (not the program); transfers use `system_program::transfer` signed by vault PDA seeds (`graduate.rs`, `sell.rs`) — this was Bug #3 we fixed.
3. Three-way fee split (creator/DINO/treasury) — additional CPI transfers vs pump's single-recipient fee transfer.
4. Graduation pays a flat 1 SOL fee before forwarding the rest to graduation_authority — pump.fun does this differently (LP-token burn).
