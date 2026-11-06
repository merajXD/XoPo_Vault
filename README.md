# XoPo Vault

A local-first, privacy-focused digital asset vault for **Windows** and **Android**, built on a shared **Rust** core.

XoPo Vault is not a dashboard with a login screen. It is meant to be a personal digital vault: multiple networks, multiple wallets, real balances, encrypted local storage, encrypted `.xdvault` backups, and direct device-to-device synchronization over the local network — with no mandatory cloud.

> **Development status: Phase 4 in progress (security).**
> The domain model, the vault aggregate and SQLCipher-encrypted persistence are implemented and tested.
> `xopo-wallet` turns a BIP-39 phrase into BIP-32 keys and encodes Bitcoin (BIP-84 and BIP-44),
> Ethereum (EIP-55) and Litecoin (BIP-44) addresses, checked against published test vectors, and
> `xopo-core` creates a wallet, stores its real derived addresses and hands the key source back to the
> caller.
> `xopo-security` wraps a random data key under an Argon2id-derived key, seals each wallet's phrase
> with ChaCha20-Poly1305, and `xopo-engine` + the `xopo` developer CLI now drive that end to end: create
> a vault, unlock it, add a wallet with real derived addresses, reveal its phrase, change the password,
> and read it all back after a restart.
> There is still **no application lock, no `.xdvault` backup, no signing, no balance retrieval and no
> product interface**; TON has not started, and platform key protection (DPAPI, Android Keystore) is
> not implemented.
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
│   ├── xopo-storage/           Encrypted SQLite: schema, migrations, repositories
│   ├── xopo-wallet/            BIP-39/32 keys, address encoding, chain adapters
│   ├── xopo-engine/            Vault lifecycle: create, unlock, add wallet, reveal, passwd
│   └── xopo-cli/               Developer CLI (binary `xopo`), temporary tooling
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

## Developer CLI (temporary)

`xopo` exercises the real engine against a real encrypted vault: a real SQLCipher file, a real
Argon2id envelope, real BIP-39/32 derivation and the sealed phrase. It is **not** the product
interface and it must not be used with real funds.

```bash
cargo build -p xopo-cli

# Create a vault (password is prompted twice, without echo)
./target/debug/xopo init demo.xopo --name "Demo Vault"

# Add a wallet from a phrase, on two networks (addresses are derived, not invented)
./target/debug/xopo wallet add demo.xopo --label "Demo Wallet" \
    --network bitcoin,ethereum --phrase-file phrase.txt

# Read it back in a separate process: this proves persistence, not a cached form
./target/debug/xopo unlock demo.xopo

# Show one wallet, and optionally the sealed phrase
./target/debug/xopo wallet show demo.xopo --id <wallet-id> --phrase

# Change the password; only the 90-byte envelope is rewritten
./target/debug/xopo passwd demo.xopo
```

`wallet add --watch <address>` records a watch-only wallet instead, which has no phrase to reveal.
The password prompt reads without echo. For scripted demos only, `XOPO_PASSWORD` and
`XOPO_NEW_PASSWORD` can supply it; that is unsafe because a process environment is readable by the
same user. Exit code `2` means wrong password or damaged key file.

A real run against the public BIP-39 test vector:

```text
$ xopo init demo.xopo --name "Demo Vault"
Vault created.
  name      : Demo Vault
  database  : /tmp/xopo-demo/demo.xopo
  key file  : /tmp/xopo-demo/demo.xopo.key
  schema    : v2

$ xopo wallet add demo.xopo --label "Demo Wallet" --network bitcoin,ethereum --phrase-file phrase.txt
Wallet added.
  id       : 7b240fe2-add8-45ec-9eaa-7f3a3148c4a4
  accounts : 2
  phrase   : sealed with the vault's data key
Unlocked "Demo Vault"
  wallets  : 1
  accounts : 2
  schema   : v2

  7b240fe2  "Demo Wallet"  (Standard)
    bitcoin   bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu     m/84'/0'/0'
    ethereum  0x9858EfFD232B4033E47d90003D41EC34EcaEda94     m/44'/60'/0'

$ xopo unlock demo.xopo
Unlocked "Demo Vault"
  wallets  : 1
  accounts : 2
...

$ XOPO_PASSWORD=wrong xopo unlock demo.xopo; echo $?
error: wrong password, or the vault key file was damaged
2
```

Both addresses match their published vectors, and the phrase does not appear in the database file.

## Documentation

Start with [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the code is organised, [docs/SECURITY.md](docs/SECURITY.md) for the security model, and [docs/ROADMAP.md](docs/ROADMAP.md) for what is real today and what is not.

## Security

This project is pre-release and unreviewed. Never enter a real seed phrase or private key into any build of it, and never commit secrets to this repository. See [docs/SECURITY.md](docs/SECURITY.md).

## License

MIT — see [LICENSE](LICENSE).
