# Security Policy

## Reporting a vulnerability

If you find a security vulnerability in the memecoin.io contracts, please
report it privately. Do **NOT** open a public GitHub issue.

**Email:** security@memecoin.io
**Response time:** We aim to acknowledge within 48 hours.

If you don't get a response within 48 hours, open a public issue titled
"Security contact unreachable" (no exploit details) so the maintainers see it.

## Scope

In scope:

- `contracts/programs/memecoin/` — the on-chain Solana program
- `devnet/test-driver/` — only insofar as it exposes contract-level issues

Out of scope:

- Front-end issues (memecoin.io website, DeniCards.com)
- Solana cluster halts, RPC provider outages
- The Meteora DLMM program
- The DINO ERC-20 token on Ethereum / its bridge
- Issues requiring social engineering or physical access
- Issues that require >50% of stake / validator collusion

## Bounty schedule (beta)

| Severity | Definition | Bounty |
|---|---|---|
| **Critical** | Drains vault SOL beyond intended fee flow, bypasses the 5 SOL cap, mints unauthorized tokens, takes control of the program | **5–10 SOL** |
| **High** | Cap bypass under specific conditions, fee math error > 0.5% of trade size, graduation race condition causing fund loss | **2–5 SOL** |
| **Medium** | Denial-of-service on a specific launch, sell math drift > 0.1%, event spoofing that misleads indexers | **0.5–2 SOL** |
| **Low** | Account validation gap with no money impact, log injection, off-by-one without exploitability | **0.1–0.5 SOL** |

Bounty is paid in SOL to a wallet you specify after the issue is confirmed and fixed.

## Hall of fame

Reporters who agree to be named will be listed in a `HALL_OF_FAME.md` file
once the contracts go to mainnet.

## What we ask of reporters

1. Give us a reasonable window (typically 30 days for critical / high, 14 days for medium / low) before public disclosure.
2. Don't exploit the issue on mainnet (devnet PoCs are fine).
3. Don't access data that isn't yours (other users' tokens, etc.).
4. Provide a clear reproduction — Anchor test, IDL ix sequence, or transaction signatures.

## Out-of-bounds testing

Local validator testing is encouraged. The `devnet/test-driver/` directory
includes scripts that run the full lifecycle on a `solana-test-validator`
instance. Use those scripts as a starting point for PoCs.

```bash
cd contracts && anchor build --no-idl -- --tools-version v1.50
solana-test-validator --reset --quiet &
cd ../devnet/test-driver && node 01-init-config.mjs
```

See `README.md` for the full local test sequence.
