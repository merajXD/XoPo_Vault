# Architecture

## Goals

1. One implementation of every business rule, shared by Windows and Android.
2. Replaceable blockchain integrations — adding a network must not touch wallet logic.
3. A security boundary that is explicit in the type system: secrets have their own layer and never appear in domain, storage DTOs or logs.
4. Local-first storage that survives process crashes and schema upgrades.
5. Testability without a network, a device or a UI.

## Layers

```text
┌──────────────────────────────────────────────────────────────┐
│ Presentation        Qt 6 / C++ (Windows)   ·   Compose (Android) │
│                     view models, navigation, design system     │
├──────────────────────────────────────────────────────────────┤
│ Platform bridge     C ABI (FFI) + versioned DTOs               │
├──────────────────────────────────────────────────────────────┤
│ Application         xopo-core: vault aggregate, use cases,     │
│                     later: portfolio and sync orchestration    │
├──────────────────────────────────────────────────────────────┤
│ Domain              xopo-domain: networks, assets, amounts,    │
│                     addresses, derivation paths, wallets       │
├──────────────────────────────────────────────────────────────┤
│ Infrastructure      storage (SQLite) · security (KDF, AEAD) ·  │
│                     backup (.xdvault) · sync (LAN) · adapters  │
└──────────────────────────────────────────────────────────────┘
```

Dependencies point **downward only**. The domain has no dependency on storage, networking, the clock of a platform, or any UI toolkit. Infrastructure crates depend on the domain, never the reverse.

## Crate responsibilities

| Crate | Responsibility | Status |
| --- | --- | --- |
| `xopo-domain` | Value objects and entities with validation: `NetworkId`, `Asset`, `Amount`, `Address`, `DerivationPath`, `Wallet`, `Account`, errors, timestamps. No I/O. | Implemented (Phase 1) |
| `xopo-core` | Application layer: the `Vault` aggregate over wallets/accounts, plus `provision::create_wallet`, which derives addresses from a key source and only then writes the wallet. Later the use cases that coordinate storage, security, backup and sync. | Implemented (Phase 1–3 subset) |
| `xopo-storage` | SQLCipher connection and key verification, versioned schema and migrations, whole-vault/wallet/account/metadata repositories, transactions, integrity checks, key store. | Implemented (Phases 2–4 subset) |
| `xopo-wallet` | BIP-39 mnemonics, BIP-32/SLIP-132 derivation, Bitcoin, Ethereum and Litecoin address encoding, key sources, chain adapters. Encodes and derives only; no signing, no storage. | Implemented (Phase 3, partial: TON and adapter network calls pending) |
| `xopo-chain-adapters` | One independent adapter per network: address checksums, balance retrieval, transaction history, transaction construction. A shared `ChainAdapter` trait lives here. | Phase 3 and 8 |
| `xopo-security` | Argon2id KDF with a parameter floor, ChaCha20-Poly1305 sealing, password-wrapped random data key, per-context sealed secrets, zeroized key buffers. Application lock, platform key protection and security events are still ahead. | Implemented (Phase 4 subset) |
| `xopo-engine` | The only crate that wires core, storage and security together: create a vault, unlock it, add a wallet and seal its phrase, reveal it, change the password. Owns the sidecar key file and the ordering rules. | Implemented (Phase 4 subset) |
| `xopo-cli` | Developer CLI, binary `xopo`. Argument parsing and readable output only; no vault rules. | Implemented (temporary development tool) |
| `xopo-backup` | `.xdvault` writing, parsing, validation and restore planning. | Phase 4 |
| `xopo-sync` | Device identity, discovery, pairing, authenticated session, record exchange, conflict resolution, revocation. | Phase 5 |

Crates are created when they receive real code, not before; `docs/ROADMAP.md` records the order. An empty crate with a placeholder API would misrepresent progress.

## Type and validation strategy

* Untrusted input (user text, imported metadata, wire payloads, database rows) crosses a **fallible constructor**: `Label::parse`, `Symbol::parse`, `Address::parse`, `Amount::from_decimal_str`, `DerivationPath::parse`, `Timestamp::from_millis`.
* Validated types implement `Deserialize` by re-running those constructors, so a tampered payload cannot produce an invalid value — even when it arrives from storage.
* Compile-time guarantees where possible: `WalletId` and `AccountId` are distinct newtypes, so an account identifier cannot be passed to a wallet lookup.
* Money is never a float. `Amount` stores atomic units in `u128` plus decimals, and arithmetic checks precision and bounds.
* No silent repair. Whitespace is not trimmed off labels, lowercase symbols are not uppercased, over-precise amounts are not rounded: the caller gets an explicit error.

## Secrets

Secret material (seed phrases, private keys, wallet passwords) is **not** part of the domain model. It belongs to `xopo-security`, which exposes operations, not values:

```text
unlock(password) -> session handle        // keys stay inside the security layer
sign(wallet_id, payload) -> signature     // no key crosses the boundary
```

The storage layout keeps only wrapped key material (AEAD ciphertexts) and metadata: `sealed_secrets`
holds one AEAD blob per context inside the encrypted file, and the 90-byte envelope that wraps the
data key sits in a sidecar `<vault>.key` — inside the file it opens, it could never be read.
`xopo-storage` never sees a password and cannot decrypt anything. Logs, error messages and DTOs are
designed so that a secret has no path to them.

The dependency direction is `xopo-security` → `xopo-domain`, `xopo-storage` → `xopo-security` only to
hand it the sealed types, and `xopo-engine` → (core, storage, security). The domain and the core
never branch on cryptography, and the CLI depends on the engine alone.

## FFI boundary (Phases 6–7)

* A dedicated `xopo-ffi` crate (created with the Windows application) exposes a **C ABI**: opaque handles, explicit ownership rules, no Rust types in the header, no panics across the boundary (every entry point catches and converts to an error code + message).
* Structurally rich data (wallet lists, settings) travels as versioned UTF-8 JSON DTOs with `serde`, because both Qt and Kotlin can parse it without generated bindings. DTOs are versioned and validated on entry, never trusted.
* Rust panics are converted to a stable error code; the platform layer maps codes to localized text.
* Android uses the same C ABI through JNI in a thin Kotlin bridge; the core is built with `cargo-ndk` per ABI.

## Storage direction (Phase 2)

SQLCipher with a versioned schema and forward-only migrations, all writes inside explicit transactions. Details are in `docs/DATABASE.md`.

Dependency direction: `xopo-storage` depends on `xopo-core` (an adapter knows the application aggregate it persists), and `xopo-core` never depends on storage. `xopo-wallet` depends only on `xopo-domain`, so key handling can be swapped or isolated without touching persistence, and `xopo-core` calls the key layer instead of deriving anything itself. The vault never stores a mnemonic: provisioning hands the key source back to the caller, which will be the security layer. That keeps the core free of SQLite and of I/O for the FFI build while still letting storage save and load a whole [`Vault`].

## Decisions

| ID | Decision | Rationale |
| --- | --- | --- |
| ADR-001 | Shared business logic in Rust; UI in Qt and Compose | One set of rules, two native experiences, no duplicated validation |
| ADR-002 | Local-first SQLite, no server dependency | Privacy, offline use, no account required |
| ADR-003 | Secrets never travel through ordinary Local Sync | Pairing is not authorization to move spend-capable material |
| ADR-004 | Validation in constructors and in `Deserialize` | A boundary that only validates "on the way in from the UI" is bypassed by storage and sync |
| ADR-005 | Domain values carry no formatting | Currency symbols, grouping, locale and timezone are presentation concerns |
| ADR-006 | Adapters are independent crates/modules with a shared trait | A failing or changed network must not affect others |
| ADR-007 | No empty scaffolding crates | The repository must show what is real, not what is planned |
| ADR-008 | Page-level SQLCipher, not field encryption | One forgotten column leaks silently with field encryption; a wrong file fails loudly with SQLCipher. Cost: a native build for Windows and Android |

## Non-goals for now

* No cloud account, no telemetry, no analytics.
* No transaction signing until derivation is verified against published test vectors on test networks.
* No "security score" computed from heuristics; the Security Center will only report checks that were actually performed.
