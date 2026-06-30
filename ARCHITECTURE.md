# Memecoin.io — Architecture & Technical Spec (v1)

**Status:** Draft v1.0
**Last updated:** 2026-06-30
**Owner:** Arbel Arif
**Domain:** memecoin.io

---

## 1. What we're building

A Pump.fun-style memecoin launcher on Solana. Anyone can launch a token in one transaction, it trades immediately on a bonding curve, and graduates to Meteora DLMM at ~$42k market cap.

The differentiator: **every trade buys $DINO (DINOLFG on Ethereum)**. One-third of all trade fees flow into a daily cross-chain buyback bot that converts SOL into DINO and holds it in treasury.

### Goals for v1
- Ship a working launcher on Solana mainnet
- Cross-chain DINO buyback bot running daily with full transparency
- Clean, fast PWA — usable on desktop and mobile
- Audited Anchor program before any real money touches it
- ~6–8 weeks from spec lock to mainnet

### Non-goals for v1
- Native iOS/Android apps (PWA covers it)
- Own AMM post-graduation (use Meteora)
- Squads multisig (use plain keypairs, upgrade later)
- DAO governance (admin-controlled, decentralize later)
- Token holder comments / social features (add post-launch)

---

## 2. Fee structure (locked)

| Fee | Amount | Split |
|---|---|---|
| **Trade fee** | 1.0% on every buy & sell | 33.33% creator / 33.33% DINO buyback / 33.33% treasury |
| **Creation fee** | 0.02 SOL per token launch | 50% DINO buyback / 50% treasury |
| **Graduation fee** | 1 SOL on graduation | 50% DINO buyback / 50% treasury |

**On-chain encoding:** all splits in basis points out of 10,000.
- Trade: 3333 / 3333 / 3334 (treasury gets rounding dust)
- Creation & graduation: 5000 / 5000

All fee parameters stored in the program's `Config` account and adjustable by the admin authority via a `update_config` instruction. This is critical — we will tune these in the first 30 days based on real data.

---

## 3. Bonding curve

### Mechanics
Constant-product curve, same family as Pump.fun. Each new token starts with:
- **Total supply:** 1,000,000,000 (1B) tokens
- **Curve allocation:** 800,000,000 (80%) for sale on the curve
- **Graduation reserve:** 200,000,000 (20%) seeded into Meteora DLMM at graduation
- **Virtual SOL reserves:** 30 SOL (so curve starts at a non-zero price)
- **Virtual token reserves:** 1,073,000,000

Price formula: `price = (virtual_sol + real_sol) / (virtual_tokens - tokens_sold)`

### Graduation trigger
- **Threshold:** ~55 SOL of real SOL deposited into the curve
- **Equivalent market cap:** ~$42k at SOL ≈ $180
- **Why $42k:** Hitchhiker's Guide meme number, distinct from Pump.fun's $69k, enough post-grad liquidity for DLMM to function
- **Admin-tunable** via `update_config` — we will adjust based on graduation rate and post-grad UX

### Graduation flow
1. Curve hits 55 SOL deposited
2. Trading on the curve disabled
3. 1 SOL graduation fee deducted (split 50/50 DINO/treasury)
4. Remaining SOL + 200M token reserve seeded into Meteora DLMM
5. LP position locked (sent to a burn address or time-locked vault)
6. Token now trades freely on Meteora

---

## 4. On-chain program (Anchor / Rust)

### Accounts

**`Config`** (singleton PDA)
- `admin: Pubkey` — can update fees, threshold, pause
- `dino_buyback_wallet: Pubkey` — Solana wallet that receives DINO buyback share
- `treasury_wallet: Pubkey` — Solana wallet that receives platform treasury share
- `trade_fee_bps: u16` (default 100 = 1%)
- `trade_split_creator_bps / dino_bps / treasury_bps: u16`
- `creation_fee_lamports: u64`
- `graduation_threshold_lamports: u64`
- `graduation_fee_lamports: u64`
- `paused: bool`

**`TokenLaunch`** (one per launched token, PDA from mint)
- `mint: Pubkey`
- `creator: Pubkey` — receives creator fee share on every trade
- `created_at: i64`
- `real_sol_reserves: u64`
- `real_token_reserves: u64`
- `virtual_sol_reserves: u64`
- `virtual_token_reserves: u64`
- `tokens_sold: u64`
- `graduated: bool`
- `graduation_tx: Option<Pubkey>`

**`CurveVault`** (PDA, holds SOL for each launch)
- SOL escrow for each token's bonding curve

### Instructions

1. **`initialize_config`** — admin-only, one-time
2. **`update_config`** — admin-only, change any tunable parameter
3. **`create_token`** — anyone; mints SPL token, creates TokenLaunch + CurveVault PDAs, collects 0.02 SOL creation fee, splits 50/50 to DINO/treasury
4. **`buy`** — anyone; deposits SOL into CurveVault, mints tokens to buyer per curve formula, collects 1% fee split 33/33/33 (creator/DINO/treasury)
5. **`sell`** — anyone; burns tokens, returns SOL per curve formula, collects 1% fee same split
6. **`graduate`** — permissionless trigger when threshold hit; pays graduation fee, calls Meteora DLMM CPI to create + seed pool, locks LP
7. **`pause` / `unpause`** — admin emergency switch

### Security considerations
- Reentrancy: Solana's account model prevents classic reentrancy, but we use `invoke_signed` carefully for Meteora CPI
- Integer overflow: `checked_add` / `checked_mul` everywhere
- Slippage: buy/sell take `min_out` / `max_in` params, transaction reverts if violated
- MEV: front-running is possible — mitigation via Jito bundles for the buyback bot, but launches themselves are inherently exposed (same as Pump.fun)
- Admin key compromise: limits via paused state, and admin can only update params — cannot drain user funds (curve vaults are PDAs not controlled by admin)

### Audit
- Quote from **OtterSec** and **Neodyme** before mainnet
- Estimated cost: $25k–$50k
- Estimated timeline: 2–4 weeks
- Block mainnet launch until clean report

---

## 5. Cross-chain DINO buyback

### The path
Fees in SOL → DINO held on Ethereum. Five steps, executed daily by a bot:

1. **Accumulate** — Solana DINO buyback wallet receives 33.33% of trade fees + 50% of creation/graduation fees
2. **Swap on Solana** — Bot swaps SOL → USDC on Jupiter (best aggregator pricing)
3. **Bridge** — USDC bridged Solana → Ethereum via **Mayan Finance** (primary) with **deBridge DLN** as fallback. ~$1–3 fee, ~5–15 min finality
4. **Swap on Ethereum** — USDC → DINO on Uniswap v3 via 1inch aggregator
5. **Transfer to treasury** — DINO sent to Ethereum treasury wallet, held (no burn)

### Bot architecture (Python)
- Runs daily at a fixed time via cron (proposed: 14:00 UTC)
- Reads SOL balance of buyback wallet; only executes if > 0.5 SOL (gas threshold)
- Uses **Helius RPC** for Solana, **Alchemy** for Ethereum
- Logs every transaction hash + amount to Postgres
- Posts to a public `/buyback` dashboard endpoint
- Failure modes: Mayan down → fallback to deBridge; Uniswap slippage too high → skip and retry next day
- Private key in environment variable, never on disk
- Runs on DigitalOcean droplet (you already have infra)

### Transparency dashboard
Public page on memecoin.io showing:
- Total SOL collected for DINO buybacks (lifetime)
- Total DINO acquired (lifetime + per-day breakdown)
- Average buy price per cycle
- Every buyback transaction with links to Solana Explorer + Etherscan
- Current treasury DINO balance (live)

This is the most important marketing surface on the entire platform.

---

## 6. Web app (Next.js 15 PWA)

### Stack
- **Framework:** Next.js 15 App Router
- **Styling:** Tailwind CSS + shadcn/ui components
- **Wallet:** `@solana/wallet-adapter` — Phantom, Solflare, Backpack
- **Charts:** Lightweight TradingView Charting Library (free tier)
- **State:** React Query for server state, Zustand for client state
- **RPC:** Helius (paid tier for production)

### Pages
- `/` — Hero, trending tokens, recent launches, recent graduations, DINO buyback stats
- `/create` — Token creation form (name, symbol, image upload, description, socials)
- `/coin/[mint]` — Token detail: price chart, buy/sell UI, holders, trades, creator info, curve progress bar
- `/portfolio` — User's holdings + transaction history
- `/buyback` — DINO buyback transparency dashboard
- `/leaderboard` — Top creators by graduations, top tokens by volume
- `/docs` — Fee structure, how it works, FAQ

### PWA features
- Installable on iOS and Android home screens
- Push notifications (token graduations, price alerts, buyback events)
- Offline shell with cached recent data
- Service worker for background updates

### Mobile UX priorities
- Buy/sell in 2 taps from token detail page
- Mobile Wallet Adapter for Phantom on Android, deep-links on iOS
- Bottom nav: Home / Create / Portfolio / Buyback

---

## 7. Indexer (Helius → Postgres)

### Why we need it
Reading every token's trade history from RPC is slow and rate-limited. We index events into Postgres for fast queries.

### Architecture
1. **Helius Webhooks** subscribed to Memecoin.io program ID
2. **Webhook receiver** (Next.js API route or separate Node service)
3. Parses program events (TokenCreated, Trade, Graduation)
4. Writes to **Postgres** tables: `tokens`, `trades`, `holders`, `graduations`, `buybacks`
5. Materialized views for trending, leaderboards, 24h volume
6. **Redis cache** for hot data (current prices, top tokens)

### Why Postgres
You already use it for DeniCards. Same operational tooling, backups, monitoring.

---

## 8. Wallet setup (v1)

Plain keypairs, no multisig — easiest, smallest mistake surface.

| Wallet | Chain | Purpose | Custody |
|---|---|---|---|
| Admin keypair | Solana | Calls admin instructions (update_config, pause) | Hardware wallet (Ledger) |
| DINO buyback wallet | Solana | Receives DINO buyback share | Bot has private key, hot |
| Treasury wallet | Solana | Receives platform treasury share | Hardware wallet, cold |
| Ethereum treasury | Ethereum | Receives bought DINO | Hardware wallet, cold |
| Bot ops wallet | Ethereum | Pays gas for swap+transfer | Hot, small balance only |

**Upgrade path:** Once monthly fees exceed ~$50k, migrate treasury wallets to Squads (Solana) and Safe (Ethereum) 2-of-3 multisigs. No contract changes required — just update the `treasury_wallet` field via `update_config`.

---

## 9. Build sequence & timeline

| Week | Milestone |
|---|---|
| 1 | Architecture doc finalized · Anchor scaffold · devnet deploy of skeleton |
| 2 | Bonding curve math · buy/sell instructions · unit tests |
| 3 | Graduation flow · Meteora CPI integration · full devnet flow working |
| 4 | Next.js scaffold · wallet connect · token creation UI · trading UI |
| 5 | Helius indexer · Postgres schema · trending/feeds wired up |
| 6 | Buyback bot · Mayan integration · transparency dashboard |
| 7 | Audit kickoff · bug fixes · testnet stress test with community |
| 8 | Audit report addressed · mainnet deploy · soft launch |

**Total: ~8 weeks from today to mainnet, assuming audit lands cleanly.**

---

## 10. Open questions & risks

### Open
- Image/metadata storage: Arweave (permanent, ~$0.01/image) vs. S3 + CDN (cheap, mutable)? Default: Arweave via Bundlr for on-chain permanence
- Initial liquidity for the platform itself — do we pre-seed any tokens? Default: no, organic launches only
- Referral program — should creators get a share for bringing other creators? Default: out of scope for v1
- Anti-bot measures on token creation — rate limits per wallet? Default: 0.02 SOL fee is the main throttle

### Risks
- **Audit findings delay launch** — buffer 2 extra weeks
- **Meteora DLMM CPI complexity** — fallback to Raydium if integration takes too long
- **Mayan bridge downtime** — deBridge fallback in bot
- **SOL price crash mid-launch** — graduation threshold is SOL-denominated so it's resilient, but $42k figure will drift; admin can retune
- **Pump.fun retaliation / clone wars** — our DINO flywheel is the moat; double down on transparency dashboard as marketing

---

## 11. What's in the workspace

```
/home/user/workspace/memecoin-io/
├── ARCHITECTURE.md          ← this file
├── contracts/               ← Anchor program (next)
├── web/                     ← Next.js PWA (week 4)
├── bot/                     ← Python buyback bot (week 6)
├── indexer/                 ← Helius webhook handler (week 5)
└── docs/                    ← Additional design docs as we go
```

---

## 12. Sign-off

Once you've read this and you're happy with the spec, I move to the Anchor program. Any changes — fees, threshold, graduation venue, stack choices — are easier to make now than after code lands.
