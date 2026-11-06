# XoPo Vault

A local-first, privacy-focused digital asset vault for **Windows** and **Android**, built on a shared **Rust** core.

XoPo Vault is not a dashboard with a login screen. It is meant to be a personal digital vault: multiple networks, multiple wallets, real balances, encrypted local storage, encrypted `.xdvault` backups, and direct device-to-device synchronization over the local network — with no mandatory cloud.

> **Development status: Phase 4 in progress (security).**
> The domain model, the vault aggregate and SQLCipher-encrypted persistence are implemented and tested.
> `xopo-wallet` turns a BIP-39 phrase into BIP-32 keys and encodes Bitcoin (BIP-84 and BIP-44),
> Ethereum (EIP-55) and Litecoin (BIP-44) addresses, checked against published test vectors, and
> `xopo-core` creates a wallet, stores its real derived addresses and hands the key source back to the
> caller.
> `xopo-security` wraps a random data key under an Argon2id-derived key, seals individual secrets with
> ChaCha20-Poly1305, and `xopo-storage` stores the wrapped key and the sealed secrets in schema v2.
> There is still **no application lock, no `.xdvault` backup, no signing, no balance retrieval and no
> interface**; a mnemonic is not yet stored by any flow, TON has not started, and platform key
> protection (DPAPI, Android Keystore) is not implemented.
> Do not use this project with real funds. Nothing in the current code can hold or move value, the
> code has not been independently reviewed, and the password is the only protection around the data
> key once the database key has been supplied.

## Principles

- **Local-first.** User data lives on the user's own devices. Synchronization between two devices on the same Wi-Fi network does not require any third-party server.
- **Privacy by design.** Market-data providers receive public asset identifiers only — never addresses, balances, keys or personal information.
- **No invented data.** Balances, prices, transactions and security states are either real, or explicitly presented as unavailable. No placeholder number is ever presented as a fact.
- **Established cryptography only.** Argon2id for password-based key derivation, AES-256-GCM or ChaCha20-Poly1305 for authenticated encryption, OS facilities (DPAPI, Android Keystore) for key protection. No custom algorithms.
- **One core, two platforms.** Business logic lives in Rust. Qt/C++ and Kotlin own presentation and platform integration, not duplicated domain rules.

## Repository layout

```text
.
├── Cargo.toml                  Rust workspace
├── crates/
│   ├── xopo-domain/            Networks, assets, amounts, addresses, wallets, accounts
│   ├── xopo-core/              Application layer: vault aggregate, wallet provisioning
│   ├── xopo-security/          Argon2id KDF, AEAD sealing, key hierarchy
│   ├── xopo-storage/           Encrypted SQLite: schema, migrations, repositories, key store
│   └── xopo-wallet/            BIP-39/32 keys, address encoding, chain adapters
├── platforms/                  Qt/C++ (Windows) and Kotlin (Android) — arrives with Phases 6–7
├── docs/
│   ├── ARCHITECTURE.md         Layers, crate responsibilities, FFI boundary, decisions
│   ├── DEVELOPMENT.md          Prerequisites, commands, workflow, conventions
│   ├── SECURITY.md             Threat model, cryptography, key hierarchy, status
│   ├── DATABASE.md             Planned SQLite schema, migrations, integrity
│   ├── BACKUP_FORMAT.md        `.xdvault` container specification (draft, Phase 4)
│   ├── SYNC_PROTOCOL.md        Local pairing and synchronization protocol (draft, Phase 5)
│   ├── UI_DESIGN_SYSTEM.md     Visual identity, tokens, components, layout
│   └── ROADMAP.md              Phases, current status, open decisions
├── scripts/                    Developer tooling and release packaging
└── .github/workflows/ci.yml    Format, lint and test on every push
```

## Build and test

Prerequisites: Rust stable (see `rust-toolchain.toml`), `rustfmt`, `clippy`, and a C toolchain
(`cc`, `make`, `perl`, `pkg-config`) because SQLCipher and its TLS dependency are built from source.

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

No network service or platform SDK is required for the current phase. The first build compiles SQLCipher
and OpenSSL from source, which takes a few minutes; later builds reuse the artifacts.

## Documentation

Start with [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the code is organised, [docs/SECURITY.md](docs/SECURITY.md) for the security model, and [docs/ROADMAP.md](docs/ROADMAP.md) for what is real today and what is not.

## Security

This project is pre-release and unreviewed. Never enter a real seed phrase or private key into any build of it, and never commit secrets to this repository. See [docs/SECURITY.md](docs/SECURITY.md).

## License

MIT — see [LICENSE](LICENSE).
