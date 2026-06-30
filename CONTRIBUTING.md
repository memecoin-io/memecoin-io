# Contributing

Thanks for looking at the memecoin.io contracts. This is beta software with
public source for community review. Contributions and bug reports welcome.

## Security issues

Do not open public issues for security bugs. See [SECURITY.md](./SECURITY.md).

## Non-security contributions

For bug reports, feature requests, or documentation fixes:

1. Open an issue first to discuss the change
2. Fork the repo
3. Create a feature branch (`git checkout -b fix/short-description`)
4. Make your change
5. Run the local validator test to verify nothing broke:
   ```bash
   cd contracts && anchor build --no-idl -- --tools-version v1.50
   solana-test-validator --reset --quiet &
   cd ../devnet/test-driver && for i in 01 02 03 04 05; do node ${i}-*.mjs; done
   ```
6. Open a pull request referencing the issue

## What we accept

- Documentation improvements
- Test coverage additions
- Devnet rehearsal scripts and fixtures
- Bug fixes (with reproduction)
- Performance improvements with benchmarks

## What we don't accept right now

- Changes to the bonding curve math (locked until v1 audit)
- Changes to the fee split (locked — 33.33/33.33/33.34)
- Removing or weakening the 5 SOL beta cap (locked until audit)
- New instructions (post-audit scope)

## Code style

- Rust: `cargo fmt`, no clippy warnings on `-D warnings`
- JS: Prettier defaults, ESM modules
- Commits: imperative present ("Add cap check" not "Added cap check")

## Toolchain

- Solana CLI 1.18.26
- Anchor 0.30.1
- Rust 1.79
- Node.js 20.x
- platform-tools v1.50 (passed to `anchor build`)

These exact versions matter — newer Anchor / Solana versions break BPF
compilation in this codebase.
