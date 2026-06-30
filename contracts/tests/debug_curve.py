"""Find the right virtual reserve sizing so the curve graduates at 55 SOL."""

SCALE = 10**6
CURVE_ALLOCATION = 800_000_000 * SCALE

# What we want: when real_sol = 55 SOL, the curve has sold ~ALL 800M tokens.
# (Or, more precisely, "most" — Pump.fun graduates at ~85% sold.)
#
# Curve: (vsol + real_sol)(vtok) = (vsol)(vtok + initial_tokens_for_sale)? No — let me redo.
#
# Standard constant-product:  x * y = k
#   x = vsol + real_sol      (SOL side)
#   y = vtok_initial - tokens_sold  (token side, decreases as bought)
# At start: tokens_sold = 0, real_sol = 0
#   k = vsol * vtok_initial
# After buying tokens_sold = T:
#   (vsol + real_sol)(vtok_initial - T) = k
#
# At graduation: T_grad = CURVE_ALLOCATION (or close), real_sol = THRESHOLD
#   (vsol + threshold)(vtok_initial - CURVE_ALLOCATION) = vsol * vtok_initial
#
# So:  vtok_initial - CURVE_ALLOCATION = vsol * vtok_initial / (vsol + threshold)
#      vtok_initial * (1 - vsol/(vsol+threshold)) = CURVE_ALLOCATION
#      vtok_initial * (threshold / (vsol + threshold)) = CURVE_ALLOCATION
#      vtok_initial = CURVE_ALLOCATION * (vsol + threshold) / threshold

THRESHOLD = 55 * 1_000_000_000  # lamports

# Try various virtual SOL values
print(f"Goal: curve sells ~{CURVE_ALLOCATION/SCALE:,.0f} tokens by the time real_sol = 55 SOL\n")
print(f"{'vSOL (SOL)':>12} | {'vTokens needed':>20} | {'start price (SOL/tok)':>22}")
print("-" * 60)

for vsol_sol in [10, 20, 30, 40, 50, 70, 100]:
    vsol = vsol_sol * 1_000_000_000
    # vtok_initial = CURVE_ALLOCATION * (vsol + threshold) / threshold
    vtok_initial = CURVE_ALLOCATION * (vsol + THRESHOLD) // THRESHOLD
    start_price_sol_per_token = (vsol / vtok_initial) * (SCALE / 1e9)
    print(f"{vsol_sol:>12} | {vtok_initial/SCALE:>20,.0f} | {start_price_sol_per_token:>22.12f}")

# Pump.fun-style: vSOL ≈ 30, vTokens sized to graduate at threshold
print("\nLet's lock in: vSOL = 30, then vTokens computed to graduate at exactly 55 SOL.\n")

VIRTUAL_SOL = 30 * 1_000_000_000
VIRTUAL_TOKENS = CURVE_ALLOCATION * (VIRTUAL_SOL + THRESHOLD) // THRESHOLD
print(f"  VIRTUAL_SOL    = {VIRTUAL_SOL:>20} lamports ({VIRTUAL_SOL/1e9} SOL)")
print(f"  VIRTUAL_TOKENS = {VIRTUAL_TOKENS:>20} base units ({VIRTUAL_TOKENS/SCALE:,.0f} whole)")

# Now simulate with corrected virtuals
def tokens_out(real_sol, real_tok, sol_in):
    x = VIRTUAL_SOL + real_sol
    y = VIRTUAL_TOKENS
    k = x * y
    new_x = x + sol_in
    new_y = k // new_x
    out = y - new_y
    return min(out, real_tok)

def sol_out(real_sol, tokens_in):
    x = VIRTUAL_SOL + real_sol
    y = VIRTUAL_TOKENS
    k = x * y
    new_y = y + tokens_in
    new_x = k // new_y
    return x - new_x

print()
print(f"{'SOL in curve':>12} | {'Tokens sold':>16} | {'% of curve':>10} | {'Price SOL/tok':>16}")
print("-" * 65)
real_sol = 0
real_tok = CURVE_ALLOCATION
total_sold = 0
for target in [1, 5, 10, 20, 30, 40, 50, 55]:
    sol_in = target * 1_000_000_000 - real_sol
    t = tokens_out(real_sol, real_tok, sol_in)
    real_sol += sol_in
    real_tok -= t
    total_sold += t
    pct = total_sold / CURVE_ALLOCATION * 100
    price = ((VIRTUAL_SOL + real_sol) / VIRTUAL_TOKENS) * (SCALE / 1e9)
    print(f"{target:>12} | {total_sold/SCALE:>16,.0f} | {pct:>9.2f}% | {price:>16.12f}")

print(f"\nAt graduation (55 SOL): {total_sold/SCALE:,.0f} tokens sold = {total_sold/CURVE_ALLOCATION*100:.2f}% of curve")

# Round-trip test with corrected curve
print()
print("Round-trip test (1 SOL buy, then sell):")
real_sol = 0
t = tokens_out(real_sol, CURVE_ALLOCATION, 1_000_000_000)
sb = sol_out(real_sol + 1_000_000_000, t)
print(f"  1 SOL → {t} tokens → {sb} lamports back  (diff: {1_000_000_000 - sb} lamports = {(1_000_000_000-sb)/1e9*100:.4f}%)")

real_sol = 30 * 1_000_000_000
t = tokens_out(real_sol, CURVE_ALLOCATION, 1_000_000_000)
sb = sol_out(real_sol + 1_000_000_000, t)
print(f"  At 30 SOL: 1 SOL → {t} tokens → {sb} lamports back  (diff: {1_000_000_000 - sb} lamports)")
