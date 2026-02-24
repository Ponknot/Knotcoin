# Contributing to Knotcoin

The original creator may or may not be available. This project is designed to survive without them.
If you're reading this after the creator is gone: **you are the maintainer now.**

## The Prime Directive

**Do not break consensus.**

Any change to these files requires extreme caution — a bug here splits the network:
- `src/consensus/` — Chain state, reward logic, genesis
- `src/crypto/` — SPHINCS+, PONC, hashing
- `src/primitives/` — Block and transaction structure
- `src/miner/` — Mining algorithm

Changes to consensus-critical code require:
1. A written proposal explaining the change and why it is necessary
2. At least 2 independent reviewers who understand the cryptography
3. An extended test period on a testnet fork before touching mainnet
4. A version bump and clearly documented migration path

## Safe Changes (one reviewer is fine)

- Bug fixes in the explorer UI
- Performance improvements in non-consensus code
- Adding new bootstrap peer addresses
- Improving the README
- Build system improvements

## How to Submit a Change

1. Fork the repository
2. Create a branch: `git checkout -b fix/description-of-fix`
3. Make your change
4. Run all tests: `cargo test`
5. Run clippy: `cargo clippy -- -D warnings`
6. Run fmt: `cargo fmt`
7. Submit a pull request with a clear description of what changed and why

## Security Vulnerabilities

**Do not open a public issue for security vulnerabilities.**
See [SECURITY.md](./SECURITY.md).

## Adding Bootstrap Peers

If you run a stable node 24/7, you can add it via `addnode` in the CLI.
This is one of the most valuable contributions you can make to the network.

## Philosophy

Knotcoin has no foundation, no company, and no employees. It is software released under the MIT license.
Improvements happen because individuals care enough to make them.
The governance system built into the protocol handles consensus-level decisions.
Everything else is decided by who shows up and does the work.
