# memecoin.io — Solana memecoin launcher (BETA)

> **⚠️ BETA — UNAUDITED — HARD-CAPPED AT 5 SOL PER LAUNCH**
>
> These contracts have NOT been audited. They enforce a hardcoded 5 SOL cap per
> bonding curve to limit blast radius until the v1 audit is complete. Do not deposit
> more than the cap — the program will reject any buy that would push real SOL above
> 5 SOL. Source is published for community review and bug bounty submissions.

## What this is

memecoin.io is a Solana program for launching memecoins via a Uniswap V2-style
bonding curve (`x * y = k`), graduating into a Meteora DLMM pool once the curve
reaches its SOL threshold. Fees are split three ways:

- **33.33% to the token creator**
- **33.33% to a DINO buyback bot** that bridges to Ethereum and burns the DINO ERC-20
- **33.34% to the protocol treasury**

The curve is mathematically equivalent to pump.fun's bonding curve with different
parameters and fee distribution. See `BONDING_CURVE_DIFF.md` for the line-by-line
comparison.

## Beta phase

- **Hard cap:** 5 SOL per launch (enforced on-chain, line 207 of `buy.rs`).
- **Graduation threshold:** 5 SOL during beta (configurable post-audit; v1 target is 55 SOL ≈ $42k market cap).
- **Status:** Local validator E2E tested. Not deployed to devnet or mainnet.

The 5 SOL cap exists so that even a worst-case undiscovered bug cannot drain more
than 5 SOL from any single launch's vault PDA. Once the contracts pass an external
audit, this cap is removed and the threshold is raised to 55 SOL.

## Architecture

```
┌─────────────┐    ┌──────────────────┐    ┌─────────────────┐
│ User wallet │───►│ memecoin.so      │◄───│ Meteora DLMM    │
└─────────────┘    │ (bonding curve)  │    │ (post-grad pool)│
                   └──────────────────┘    └─────────────────┘
                          │
                          ├──► creator wallet (33.33% of trade fees)
                          ├──► DINO buyback bot ──► Ethereum ──► DINO burn
                          └──► treasury (33.34% of trade fees)
```

Components in this repo:

- `contracts/programs/memecoin/` — Anchor 0.30.1 program (the actual on-chain code)
- `devnet/test-driver/` — Node.js scripts that drive a local validator end-to-end test
- `services/indexer/` — Postgres indexer that consumes program events (separate tarball)
- `services/meteora_crank/` — Off-chain crank that seeds the Meteora DLMM pool post-graduation
- `services/buyback_bot/` — Bridges DINO buyback SOL → Ethereum → burns DINO ERC-20

## Instructions

| Ix | Signer | What it does |
|---|---|---|
| `initialize_config` | admin | Sets fee bps, threshold, treasury/DINO wallets |
| `update_config` | admin | Updates non-immutable config fields |
| `create_token` | creator | Mints 1B tokens to curve, 0 to creator; pays 0.02 SOL creation fee |
| `buy` | buyer | Exact-out: requests N tokens, pays up to max_sol_in; enforces 5 SOL cap |
| `sell` | seller | Exact-in: sells N tokens, receives floor SOL out |
| `graduate` | anyone (crank) | When real_sol >= threshold: drains vault, pays 1 SOL grad fee (split 50/50 DINO/treasury), mints 200M reserve to graduation_authority |
| `seal_graduation` | graduation_authority | After Meteora pool is seeded off-chain, stores `meteora_lb_pair` on the launch account |

## Known issues / historical bugs

All three were caught during local validator testing. None reached devnet or mainnet.

1. **Stack overflow in `create_token`** — original try_accounts blew the 4 KB BPF stack. Fixed by `Box<Account>` + `UncheckedAccount` across all instruction files. Symptom was a spurious `Paused` error.
2. **Bonding curve never decremented `virtual_token_reserves`** — 800M tokens sold for only 28 SOL instead of ~55. Fixed by mutating `launch.virtual_token_reserves` in both buy.rs and sell.rs.
3. **Vault SOL ownership mismatch** — `graduate.rs` and `sell.rs` mutated vault lamports directly, but the vault PDA is System-owned (funded via `system_program::transfer`). Fixed by switching to `system_program::transfer` signed by vault PDA seeds.

See `MAINNET_RUNBOOK.md` for full incident write-ups.

## Audit roadmap

Funded from cumulative trade fees post-beta:

| Phase | Audit firm | Cost target | Trigger |
|---|---|---|---|
| Beta 1 | Sec3 Pro (automated + light manual) | ~$2k | After 50+ devnet launches with no critical issues |
| Beta 2 | Ackee Blockchain | $15–25k | After ~$50k cumulative trade volume on mainnet beta |
| v1 GA | OtterSec or Trail of Bits | $40–80k | Before raising the cap above 55 SOL |

## Bug bounty (beta)

Open to anyone who finds a real exploit. No KYC, no NDA.

| Severity | Definition | Bounty |
|---|---|---|
| Critical | Drains vault or mint authority beyond intended fee flow; bypasses cap | **5–10 SOL** |
| High | Cap bypass, fee math error >0.5%, graduation race condition | **2–5 SOL** |
| Medium | DoS on a launch, sell math off >0.1%, event spoofing | **0.5–2 SOL** |
| Low | Account validation gap with no money impact, log injection | **0.1–0.5 SOL** |

Report to: security@memecoin.io (PGP key at `keys/security.asc` — TODO add)

**Out of scope:** Front-end issues, Solana cluster halts, RPC provider outages, anything in the Meteora DLMM or DINO ERC-20 contracts (report those to their respective teams).

## Building

Requires Solana 1.18.x, Anchor 0.30.1, Rust 1.79, platform-tools v1.50.

```bash
cd contracts
anchor build --no-idl -- --tools-version v1.50
```

The `--no-idl` flag is intentional — IDL generation pulls newer toolchain versions that break BPF compilation in this codebase.

## Testing

Local validator E2E test (no devnet, no real SOL):

```bash
cd contracts
solana-test-validator --reset --quiet &
cd ../devnet/test-driver
npm install
node 01-init-config.mjs   # admin sets config
node 02-create-token.mjs  # creator launches a token
node 03-buy.mjs           # 5 buyers drive curve to 5 SOL cap
node 04-graduate.mjs      # crank graduates
node 05-seal-graduation.mjs # graduation_authority seals with mock LB pair
```

Last full E2E test: all 5 stages passed with real_sol landing at exactly 5_000_000_000 lamports (5.000000000 SOL) before graduation.

## License

MIT — fork freely. If you ship a fork to mainnet, please at minimum keep the 5 SOL beta cap or remove it only after your own audit.

## Status legend

- ✅ Curve math + buy/sell/graduate/seal — implemented, tested
- ✅ Beta cap (5 SOL) — implemented, tested
- ✅ Three-way fee split — implemented, tested
- ⏳ Devnet deployment — pending
- ⏳ Sec3 audit pass — pending
- ⏳ Mainnet launch — pending audit
- ⏳ DINO buyback bot mainnet keys — pending
- ⏳ Meteora crank devnet rehearsal — pending
