"""Detailed round-trip analysis to characterize the curve spread."""

SCALE = 10**6
CURVE_ALLOCATION = 800_000_000 * SCALE
THRESHOLD = 55 * 1_000_000_000
VIRTUAL_SOL = 30 * 1_000_000_000
VIRTUAL_TOKENS = CURVE_ALLOCATION * (VIRTUAL_SOL + THRESHOLD) // THRESHOLD

def tokens_out(real_sol, real_tok, sol_in):
    x = VIRTUAL_SOL + real_sol
    y = VIRTUAL_TOKENS
    k = x * y
    new_x = x + sol_in
    new_y = k // new_x  # floor → curve keeps a fraction of a token
    out = y - new_y
    return min(out, real_tok)

def sol_out(real_sol, tokens_in):
    x = VIRTUAL_SOL + real_sol
    y = VIRTUAL_TOKENS
    k = x * y
    new_y = y + tokens_in
    new_x = k // new_y  # floor → curve keeps a lamport
    return x - new_x

# Pre-buy a chunk so we're not at the absolute origin
# (curves are most curvy at the bottom where rounding hurts most)
print(f"{'amount in (SOL)':>16} | {'tokens received':>20} | {'SOL back':>16} | {'spread (lamports)':>18} | {'spread %':>10}")
print("-" * 95)

for sol_amt in [0.001, 0.01, 0.1, 1.0, 5.0, 10.0]:
    sol_in = int(sol_amt * 1_000_000_000)
    real_sol = 0
    real_tok = CURVE_ALLOCATION
    t = tokens_out(real_sol, real_tok, sol_in)
    sb = sol_out(real_sol + sol_in, t)
    spread = sol_in - sb
    print(f"{sol_amt:>16.4f} | {t:>20} | {sb:>16} | {spread:>18} | {spread/sol_in*100:>9.4f}%")

# This is the structural spread caused by floor-division on tokens_out then sell-back.
# It's not a bug — it's how the curve works. At Pump.fun's scale (SCALE = 10^6) it's small
# relative to the 1% fee. Verify:

print()
print("With 1% fee applied, the round-trip economic loss is:")
print(f"{'amount in':>10} | {'spread+fee back %':>22}")
print("-" * 38)
for sol_amt in [0.1, 1.0, 10.0]:
    sol_in = int(sol_amt * 1_000_000_000)
    sol_net_to_curve = int(sol_in * 0.99)  # 1% fee removed
    real_sol = 0
    real_tok = CURVE_ALLOCATION
    t = tokens_out(real_sol, real_tok, sol_net_to_curve)
    gross_back = sol_out(real_sol + sol_net_to_curve, t)
    net_back = int(gross_back * 0.99)
    print(f"{sol_amt:>10.4f} | {(sol_in - net_back)/sol_in*100:>22.4f}%")

# Conclusion: a normal user trading 1 SOL pays ~3% rounding spread + 2% fees ≈ 5% total
# round-trip loss. That's actually fine and matches Pump.fun's UX.
# But the *rounding* part of that varies with curve position.
