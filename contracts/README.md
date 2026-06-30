# Memecoin.io Anchor Program

Solana on-chain program for the Memecoin.io launcher. Anchor 0.30.1 / Solana 1.18.

## Status

**v1 complete.** Compiles, all instructions defined, math verified off-chain,
graduation hand-off implemented via the off-chain graduation-authority pattern.

Meteora pool creation runs in the off-chain crank (see `meteora-crank/` in the
repo root) — not as an on-chain CPI. This is the same pattern pump.fun and
letsbonk use because:

- Meteora DLMM init + seed-liquidity requires ~80 accounts; embedding the
  full CPI exceeds Solana's 1232-byte transaction size limit
- The Meteora Rust SDK is version-pinned to Solana 2.x / Anchor 0.31 which
  conflicts with this program's 0.30.1 stack
- The off-chain crank can use Meteora's official TypeScript SDK with all
  the correct account derivations and route the entire bundle through
  Jito for atomicity

## What's in here

```
programs/memecoin/src/
├── lib.rs                     entry points & program declaration
├── errors.rs                  all MemecoinError variants
├── events.rs                  emitted events (indexed by Helius)
├── utils.rs                   bonding-curve math + fee splits (with unit tests)
├── state/
│   ├── config.rs              global Config PDA
│   └── token_launch.rs        per-token TokenLaunch + curve constants
└── instructions/
    ├── initialize_config.rs   admin: one-time setup
    ├── update_config.rs       admin: tune any parameter
    ├── set_paused.rs          admin: emergency switch
    ├── create_token.rs        anyone: launch a new memecoin
    ├── buy.rs                 anyone: buy from bonding curve
    ├── sell.rs                anyone: sell back to curve
    ├── graduate.rs            anyone: trigger graduation, hand liquidity
    │                          to graduation_authority for off-chain seed
    └── seal_graduation.rs     graduation_authority: record Meteora pool
```

## Locked v1 parameters

| Parameter | Value |
|---|---|
| Trade fee | 1.0% (100 bps) |
| Trade split | 33.33% creator / 33.33% DINO / 33.34% treasury |
| Creation fee | 0.02 SOL |
| Graduation fee | 1 SOL |
| Graduation threshold | 55 SOL deposited (~$42k mcap) |
| Curve total supply | 1B tokens |
| Curve allocation | 800M (sold via curve) |
| Graduation reserve | 200M (seeded into Meteora) |
| Virtual SOL reserves | 30 SOL |
| Virtual tokens | 1,236,363,636 |
| Token decimals | 6 |

All fee/threshold parameters are tunable post-launch via `update_config`.

## Bonding curve math

Constant product: `(vsol + real_sol) × (vtok - tokens_sold) = k`

Virtual reserves sized so the curve sells **all 800M tokens** by the time real_sol = 55 SOL:
```
vtok = CURVE_ALLOCATION × (vsol + threshold) / threshold
     = 800M × (30 + 55) / 55 = 1,236,363,636 tokens
```

See `tests/curve_math_sim.py` and `tests/roundtrip_test.py` for verification.

## Build & deploy

```bash
# install Solana + Anchor toolchain if not already
sh -c "$(curl -sSfL https://release.solana.com/v1.18.18/install)"
cargo install --git https://github.com/coral-xyz/anchor avm --locked
avm install 0.30.1 && avm use 0.30.1

# build
cd contracts
anchor build

# set up devnet wallet
solana config set --url devnet
solana airdrop 5

# deploy to devnet
anchor deploy --provider.cluster devnet

# run integration tests
yarn install
anchor test --skip-local-validator
```

## Security notes

- All math uses `checked_*` ops; `u128` intermediates where needed
- Admin can update parameters but **cannot drain user funds** — curve vaults are PDAs not controlled by admin
- `paused` flag halts create/buy/sell/graduate but doesn't affect already-deposited SOL
- PDA seeds prevent address spoofing on all accounts
- Slippage protection via `max_sol_in` (buy) / `min_sol_out` (sell)
- **Must be audited before mainnet** — OtterSec or Neodyme

## Graduation flow

```
┌──────────────────┐    1. graduate()                  ┌───────────────────────┐
│  Curve hits 55   │ ────────────────────────────────► │  Anchor program       │
│  SOL threshold   │                                   │  - pay 1 SOL fee      │
└──────────────────┘                                   │  - mint 200M reserve  │
                                                       │  - send SOL+tokens to │
                                                       │    graduation_auth    │
                                                       │  - emit TokenGraduated│
                                                       └──────────┬────────────┘
                                                                  │
                                                                  ▼
                                              ┌───────────────────────────────┐
                                              │  Off-chain crank (Node.js)    │
                                              │  - initializeCustomizable...  │
                                              │  - addLiquidityByStrategy2    │
                                              │  - bundle via Jito            │
                                              └──────────┬────────────────────┘
                                                         │
                                                         ▼
                                              ┌───────────────────────────────┐
                                              │  Anchor program               │
                                              │  seal_graduation(lb_pair)     │
                                              │  - launch.pool_seeded = true  │
                                              │  - emit PoolSeeded            │
                                              └───────────────────────────────┘
```

The LP position NFT stays in the graduation authority's wallet and is then
transferred to `config.treasury_wallet` in the same Meteora seed-liquidity
transaction (set `position_owner = treasury_wallet` and `fee_owner = treasury_wallet`
when calling Meteora). Per user instruction "dont burn just hold in treasury."

## Known TODOs (week 3+)

- [ ] Token metadata via Metaplex (name, symbol, URI on-chain)
- [ ] Integration tests for buy / sell / graduate / seal_graduation
- [ ] Client TypeScript SDK in `app/sdk/`
- [ ] Off-chain Meteora crank (`meteora-crank/` directory)
