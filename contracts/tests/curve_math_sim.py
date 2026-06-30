"""
Off-chain simulator that mirrors the on-chain bonding curve math exactly.

Used to:
  1. Sanity-check the curve formulas before deploying
  2. Generate the expected SOL→tokens table at key milestones
  3. Verify the graduation threshold produces the expected market cap
"""

DECIMALS = 6
SCALE = 10**DECIMALS

CURVE_ALLOCATION = 800_000_000 * SCALE        # 800M tokens in base units
VIRTUAL_SOL = 30 * 1_000_000_000              # 30 SOL in lamports
VIRTUAL_TOKENS = 1_073_000_000 * SCALE        # 1.073B tokens in base units
GRADUATION_THRESHOLD = 55 * 1_000_000_000     # 55 SOL in lamports


def tokens_out_for_sol_in(real_sol: int, real_token_reserves: int, sol_in: int) -> int:
    x = VIRTUAL_SOL + real_sol
    y = VIRTUAL_TOKENS
    k = x * y
    new_x = x + sol_in
    new_y = k // new_x
    tokens_out = y - new_y
    assert tokens_out <= real_token_reserves, "would overshoot curve"
    return tokens_out


def sol_in_for_tokens_out(real_sol: int, real_token_reserves: int, tokens_out: int) -> int:
    x = VIRTUAL_SOL + real_sol
    y = VIRTUAL_TOKENS
    k = x * y
    new_y = y - tokens_out
    # ceil(k / new_y)
    new_x = (k + new_y - 1) // new_y
    return new_x - x


def sol_out_for_tokens_in(real_sol: int, tokens_in: int) -> int:
    x = VIRTUAL_SOL + real_sol
    y = VIRTUAL_TOKENS
    k = x * y
    new_y = y + tokens_in
    new_x = k // new_y
    return min(x - new_x, real_sol)


def price_in_sol_per_token(real_sol: int) -> float:
    """Marginal price at the current curve state — SOL per 1 whole token."""
    x = VIRTUAL_SOL + real_sol
    y = VIRTUAL_TOKENS
    # price = x / y, in lamports per base unit. Convert to SOL per whole token:
    return (x / y) * (SCALE / 1_000_000_000)


def market_cap_usd(real_sol: int, sol_price_usd: float = 180.0) -> float:
    """Market cap = current marginal price × total supply (1B whole tokens)."""
    price_sol = price_in_sol_per_token(real_sol)
    return price_sol * 1_000_000_000 * sol_price_usd


print("=" * 70)
print("Memecoin.io bonding curve simulator")
print("=" * 70)
print(f"\nStarting state:")
print(f"  Virtual SOL:    {VIRTUAL_SOL / 1e9:>10.2f} SOL")
print(f"  Virtual tokens: {VIRTUAL_TOKENS / SCALE:>10,.0f} tokens")
print(f"  Curve supply:   {CURVE_ALLOCATION / SCALE:>10,.0f} tokens")
print(f"  Starting price: {price_in_sol_per_token(0):>10.10f} SOL/token")
print(f"  Starting mcap:  ${market_cap_usd(0):>14,.0f} USD (SOL=$180)")

print(f"\n{'SOL on curve':>14} | {'Tokens sold':>16} | {'% sold':>7} | {'Price (SOL)':>14} | {'Mcap USD':>14}")
print("-" * 80)

real_sol = 0
real_tok = CURVE_ALLOCATION
tokens_sold = 0

for target_sol in [1, 5, 10, 20, 30, 40, 50, 55, 60, 70, 80, 85]:
    sol_in = target_sol * 1_000_000_000 - real_sol
    if sol_in <= 0:
        continue
    try:
        tokens = tokens_out_for_sol_in(real_sol, real_tok, sol_in)
    except AssertionError:
        print(f"  ⚠ stopped at {real_sol/1e9:.2f} SOL — would exceed curve supply")
        break
    real_sol += sol_in
    real_tok -= tokens
    tokens_sold += tokens
    pct = tokens_sold / CURVE_ALLOCATION * 100
    marker = "  ← GRADUATION" if target_sol == 55 else ""
    print(f"{target_sol:>14} | {tokens_sold/SCALE:>16,.0f} | {pct:>6.2f}% | "
          f"{price_in_sol_per_token(real_sol):>14.10f} | ${market_cap_usd(real_sol):>13,.0f}{marker}")

print()
print("=" * 70)
print("Round-trip test: buy 1 SOL worth, then sell it all back")
print("=" * 70)
real_sol_test = 0
tokens = tokens_out_for_sol_in(real_sol_test, CURVE_ALLOCATION, 1_000_000_000)
sol_back = sol_out_for_tokens_in(real_sol_test + 1_000_000_000, tokens)
print(f"  SOL in:  {1_000_000_000:>15} lamports")
print(f"  Tokens:  {tokens:>15} base units ({tokens/SCALE:,.4f} whole)")
print(f"  SOL out: {sol_back:>15} lamports")
print(f"  Diff:    {1_000_000_000 - sol_back:>15} lamports (rounding loss to curve)")

print()
print("=" * 70)
print("Fee splits — sanity check")
print("=" * 70)


def split_three_way(amount: int, creator_bps: int, dino_bps: int) -> tuple[int, int, int]:
    creator = (amount * creator_bps) // 10_000
    dino = (amount * dino_bps) // 10_000
    treasury = amount - creator - dino
    return creator, dino, treasury


for amount in [1_000_000, 999_999, 100, 7]:
    c, d, t = split_three_way(amount, 3333, 3333)
    print(f"  amount={amount:>10}  →  creator={c:>7}  dino={d:>7}  treasury={t:>7}  "
          f"sum={c+d+t}  ok={c+d+t == amount}")
