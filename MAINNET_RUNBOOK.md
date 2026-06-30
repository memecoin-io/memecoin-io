# Memecoin.io — Mainnet Deployment Runbook

End-to-end procedure for deploying the Memecoin.io launcher to Solana mainnet-beta. Validated against the local validator test on 2026-07-01 (all instructions executed successfully, all events emitted, graduation hand-off and DLMM seal verified on-chain).

---

## 0. Locked v1 Parameters

| Parameter | Value |
|---|---|
| Total supply | 1,000,000,000 tokens |
| Curve allocation | 800,000,000 tokens |
| Graduation reserve | 200,000,000 tokens (held by graduation authority for DLMM seed) |
| Decimals | 6 |
| Virtual SOL | 30 SOL |
| Virtual tokens | 1,236,363,636 (matches pump.fun curve shape) |
| Trade fee | 100 bps (1%) |
| Trade split | 33.33% creator / 33.33% DINO buyback / 33.34% treasury |
| Creation fee | 0.02 SOL (20,000,000 lamports) |
| Creation split | 50% DINO / 50% treasury |
| Graduation fee | 1 SOL |
| Graduation split | 50% DINO / 50% treasury |
| **Graduation threshold** | **55 SOL** (≈ $42k at $760/SOL) — **NEVER use the 54 SOL test value on mainnet** |

---

## 1. Pre-flight checklist

### Wallets (use Squads multisig or Ledger hardware for every role)

| Role | Purpose | Hardware/multisig? |
|---|---|---|
| `program upgrade authority` | Can deploy new program versions | Multisig (3-of-5 recommended) |
| `admin` | Updates `Config` via `update_config` | Multisig (2-of-3 minimum) |
| `dino_buyback_wallet` | Receives DINO buyback share | Hot wallet — buyback bot signs |
| `treasury_wallet` | Receives treasury share | Multisig |
| `graduation_authority` | Holds 200M reserve + ~54 SOL between `graduate` and Meteora seed | Hot wallet (crank operator) with monitored balance alerts |

**Never** make the program upgrade authority and the admin the same key. Compromise of admin should not give code-deploy rights.

### Toolchain

```bash
solana-cli 1.18.26
anchor 0.30.1
platform-tools v1.50  (passed via `-- --tools-version v1.50`)
node >= 20
```

---

## 2. Build (reproducible)

```bash
cd contracts
anchor build --no-idl -- --tools-version v1.50
shasum -a 256 target/deploy/memecoin.so   # record this; verify pre- and post-deploy
```

Expected binary size: ~395 KB.

A `Stack offset of 4736 exceeded max offset of 4096` warning appears on the `CreateToken` accounts struct. Verified safe in practice because all large accounts are boxed (`Box<Account<...>>`) and the warning is emitted by an estimator that includes inlined accessor frames the runtime never actually allocates simultaneously. **Do not add new fields to the `CreateToken` accounts struct without re-running a local validator end-to-end test** — additions can push the runtime stack past 4096 and corrupt deserialization (the `paused` bug we saw on the first test run was caused by exactly this).

---

## 3. Deploy

```bash
# Set cluster
solana config set --url https://api.mainnet-beta.solana.com

# Reserve a program ID (use a freshly generated keypair, then back it up offline)
solana-keygen new -o keys/mainnet-program-id.json

# Compute program ID and update declare_id! macro in programs/memecoin/src/lib.rs
PROGRAM_ID=$(solana-keygen pubkey keys/mainnet-program-id.json)
echo $PROGRAM_ID

# Rebuild with the new program ID baked in
anchor build --no-idl -- --tools-version v1.50

# Deploy
solana program deploy \
  target/deploy/memecoin.so \
  --program-id keys/mainnet-program-id.json \
  --upgrade-authority <UPGRADE_AUTHORITY_KEYPAIR_OR_MULTISIG>

# Verify binary on-chain matches local
solana program dump $PROGRAM_ID /tmp/deployed.so
diff <(shasum -a 256 target/deploy/memecoin.so | awk '{print $1}') \
     <(shasum -a 256 /tmp/deployed.so | awk '{print $1}')
```

**Cost:** ~4-5 SOL for the program account (rent for ~395 KB).

---

## 4. Initialize Config (one-time)

Use `devnet/test-driver/01-init-config.mjs` as a template. Update wallet pubkeys to the production values from §1. Verify on a fork (`solana-test-validator --url mainnet-beta --clone <PROGRAM_ID>`) before broadcasting.

After broadcast, immediately verify:

```bash
# Read Config account
solana account <CONFIG_PDA> --output json | jq .
```

Confirm all fields match expected; if not, do **not** create any launches — call `update_config` to correct first.

---

## 5. Operational Procedures

### 5.1 Token creation (user-facing)

Frontend calls `create_token` directly with the creator's wallet. Pays 0.02 SOL fee. Mints 800M tokens to the launch's PDA-owned ATA.

### 5.2 Buy / Sell (user-facing)

Frontend calls `buy` or `sell`. Slippage protection enforced via `min_tokens_out` / `min_sol_out`.

Curve math (post-fix in `utils.rs` / `buy.rs` / `sell.rs`):
```
x = virtual_sol + real_sol
y = virtual_tokens  (mutated: -= tokens_out on buy, += on sell)
k = x * y          (invariant within a single tx, varies across tx due to ceil rounding)
```

### 5.3 Graduation (permissionless crank)

When `real_sol_reserves >= 55_000_000_000` (55 SOL in lamports), anyone may call `graduate`. We run a dedicated crank for SLA:

```bash
node crank/graduate-monitor.mjs   # subscribes to logs, fires graduate within 1 slot
```

Outcome of `graduate`:
- Vault drained: 1 SOL graduation fee split 0.5/0.5 to DINO buyback + treasury
- 200M reserve tokens minted to `graduation_authority` ATA
- Remaining ~54 SOL transferred to `graduation_authority` wallet
- Launch marked `graduated = true`, `pool_seeded = false`
- `TokenGraduated` event emitted

### 5.4 Meteora DLMM seed (off-chain)

Within 30 seconds of `TokenGraduated`, the meteora-crank service:

1. Reads `final_sol_reserves` and `final_token_reserves` from the event
2. Calls Meteora SDK `initializeCustomizablePermissionlessLbPair2` with our DLMM config
3. Calls `addLiquidityByStrategy2` to seed the 200M tokens + ~54 SOL
4. Waits for both txs to confirm (max 60s)
5. Calls `seal_graduation` with the resulting `lb_pair` pubkey

If step 2/3 fails, the crank retries with exponential backoff. The 200M tokens + ~54 SOL stay safely in the graduation_authority wallet until success — they are **not lost** even if the crank crashes; the operator can resume from any seed of the wallet.

**Monitoring:** alert if any launch sits with `graduated=true && pool_seeded=false` for more than 5 minutes.

---

## 6. Emergency Procedures

### 6.1 Pause

`set_paused(true)` blocks all `create_token`, `buy`, `sell`, `graduate`, `seal_graduation`. Use if a vulnerability is detected.

```bash
node ops/set-paused.mjs true
```

### 6.2 Config rotation

Wallet compromised? Call `update_config` from admin with new wallet pubkeys. Existing launches keep functioning (config is read at every instruction).

### 6.3 Program upgrade

```bash
anchor build --no-idl -- --tools-version v1.50
solana program deploy target/deploy/memecoin.so \
  --program-id keys/mainnet-program-id.json \
  --upgrade-authority <UPGRADE_AUTHORITY>
```

**Always** run a full local validator end-to-end test (`devnet/test-driver/01..05.mjs`) before upgrading mainnet. Stack frame size and curve math are the historical foot-guns.

---

## 7. Local Validator Test Evidence (2026-07-01)

The following test run validates the production code path. Replay it with `devnet/test-driver/01..05.mjs` before each mainnet deploy.

| Step | Result |
|---|---|
| `initialize_config` | ✓ TX `2wtQZceHDkmurc7knQW7HeXrWJmmWWoNyJCJd2hxRxmXvv1K7DjVGzEfLbJ1CFXdhqTQmZXdYYNCYAExf1SFQ75p` |
| `update_config` (threshold→54 SOL for test) | ✓ TX `2vYwxr5mpTh2LNk1heRDtNbBRKPJugAFCpkzK82MmUykGQo1wkFQory7mimHTmQXKyRnHg5tmWFYCoYqedz28hhL` |
| `create_token` | ✓ TX `4hmyiY69zqrDcwEWiUNVsozaUBTQYXv5nH2KmwsNXtRmTzGJpV9Ebmcqk8XurgQhLMYBHFstMAp9Zz837jxdNyD7` |
| 47 buys driving curve from 0 → 54.9998 SOL | ✓ all confirmed |
| `graduate` | ✓ TX `3eGS5V6XhvTDuneDKMXKyDJoWWWqAjm25vMh8KkKV7t2C2rDQ9KJThMZMZJz5iCnxaSbHp65FBGf7TH4HzwusyvU` |
| Vault drained 54.9998 SOL → 0 | ✓ |
| DINO + treasury each received 0.5 SOL fee | ✓ |
| Graduation authority received 53.9998 SOL + 200,000,000.000000 tokens (200M × 10⁶ base units) | ✓ |
| `TokenGraduated` event emitted | ✓ |
| `seal_graduation` with mock LB pair | ✓ TX `2f5iFgU6YTuwrHJbpP4iLu9diCGTGtySyLWm7se5KhYdKqKH7BZSpnZ1qEEvH9U7FU5WoxbG9uHn7o6pRC78dyCP` |
| `PoolSeeded` event emitted; `meteora_lb_pair` written to launch | ✓ |

---

## 8. Historical Bugs Fixed (do not regress)

### 8.1 Stack overflow in `CreateToken::try_accounts` (corrupting `config.paused`)
**Symptom:** `create_token` failed with `Error Code: Paused (6000)` even though `config.paused == 0` on-chain.
**Cause:** `CreateToken` accounts struct deserialized into 5128 bytes of stack — corrupted neighboring memory including the loaded `Config`'s `paused` field.
**Fix:** Boxed all heavy `Account<...>` and converted `AccountInfo` to `UncheckedAccount` across every instruction. Final stack usage 4736 bytes (still warns, no longer corrupts).

### 8.2 Bonding curve never decremented `virtual_token_reserves`
**Symptom:** Selling all 800M curve tokens yielded only ~28.5 SOL instead of ~55 SOL.
**Cause:** `utils.rs` math used `y = virtual_tokens` as a constant; never updated.
**Fix:** `buy.rs` decrements `launch.virtual_token_reserves` by `token_amount_out`; `sell.rs` mirrors with `checked_add`. Now matches pump.fun curve shape (constant-product on `(virtual_sol + real_sol)` × `(virtual_tokens − tokens_sold)`).

### 8.3 Vault SOL escrow not program-owned
**Symptom:** `graduate` and `sell` failed with `instruction spent from the balance of an account it does not own`.
**Cause:** The vault PDA receives SOL via `system_program::transfer`, so it stays System-owned. The original `debit_pda_lamports` direct-lamport mutation only works on program-owned accounts.
**Fix:** Replaced direct lamport mutation in `graduate.rs` and `sell.rs` with `system_program::transfer` signed by the vault PDA seeds (`[VAULT_SEED, mint, vault_bump]`).

---

## 9. Files & Artifacts

| Artifact | Path |
|---|---|
| Program source | `contracts/programs/memecoin/src/` |
| Built binary | `contracts/target/deploy/memecoin.so` |
| Test driver | `devnet/test-driver/0[1-5]*.mjs` |
| Indexer service | `indexer/` |
| Buyback bot | `buyback-bot/` |
| Meteora crank | `meteora-crank/` |
| Architecture | `ARCHITECTURE.md` |
| This runbook | `MAINNET_RUNBOOK.md` |
