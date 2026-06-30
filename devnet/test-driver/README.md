# Test driver

End-to-end driver scripts that run the memecoin program through its full
lifecycle on a Solana cluster (local validator by default, devnet with env
overrides).

## Setup

```bash
npm install
```

Generate fresh keypairs (one-time):

```bash
mkdir -p ../keys
for n in deployer graduation_authority dino_buyback treasury creator buyer1 buyer2 buyer3 buyer4 buyer5; do
  solana-keygen new --no-bip39-passphrase --silent --outfile ../keys/${n}.json
done
```

## Run against local validator

```bash
# In one terminal:
solana-test-validator --reset --quiet

# In another, fund the wallets:
for n in deployer graduation_authority creator buyer1 buyer2 buyer3 buyer4 buyer5; do
  PK=$(solana-keygen pubkey ../keys/${n}.json)
  solana airdrop 100 $PK --url localhost
done

# Deploy the program (from contracts/ directory):
cd ../../contracts
anchor build --no-idl -- --tools-version v1.50
solana program deploy target/deploy/memecoin.so \
  --program-id target/deploy/memecoin-keypair.json \
  --url localhost \
  --keypair ../devnet/keys/deployer.json

# Run the lifecycle:
cd ../devnet/test-driver
node 01-init-config.mjs
node 02-create-token.mjs
node 03-buy.mjs
node 04-graduate.mjs
node 05-seal-graduation.mjs
```

## Run against devnet

```bash
export MEMECOIN_RPC=https://api.devnet.solana.com
# Fund the deployer wallet via https://faucet.solana.com or wallet airdrop
solana airdrop 5 $(solana-keygen pubkey ../keys/deployer.json) --url devnet
# Then deploy and run the same scripts.
```

## Environment variables

| Var | Default | Purpose |
|---|---|---|
| `MEMECOIN_RPC` | `http://127.0.0.1:8899` | RPC endpoint |
| `MEMECOIN_KEYS_DIR` | `../keys` (relative to repo root) | Where to find `*.json` keypair files |
| `MEMECOIN_STATE_FILE` | `./state.json` | Where the driver persists run state |

## Notes

- `state.json` is the bridge between scripts — it holds the mint, launch PDA, vault PDA, and TX signatures from each step. Delete it before re-running from scratch.
- During beta the threshold is hardcoded to 5 SOL in the program. Scripts 01 and 03 are pre-configured for this.
- `99-lower-threshold.mjs` and `99-set-paused.mjs` are admin utilities.
