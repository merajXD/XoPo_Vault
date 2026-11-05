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
| `xopo-core` | Application layer: the `Vault` aggregate over wallets/accounts; later the use cases that coordinate storage, security, backup and sync. | Started (in-memory aggregate) |
| `xopo-storage` | SQLCipher connection and key verification, versioned schema and migrations, wallet/account/metadata repositories, transactions, integrity checks. | Implemented (Phase 2) |
| `xopo-wallet` | HD derivation and address generation, wallet creation/import, watch-only support, key-source abstraction. | Phase 3 |
| `xopo-chain-adapters` | One independent adapter per network: address checksums, balance retrieval, transaction history, transaction construction. A shared `ChainAdapter` trait lives here. | Phase 3 and 8 |
| `xopo-security` | Argon2id KDF, AEAD sealing/opening, key hierarchy, application lock, platform key protection, security events. | Phase 4 |
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

The storage layout will keep only wrapped key material (AEAD ciphertexts) and metadata. Logs, error messages and DTOs are designed so that a secret has no path to them.

## FFI boundary (Phases 6–7)

* A dedicated `xopo-ffi` crate (created with the Windows application) exposes a **C ABI**: opaque handles, explicit ownership rules, no Rust types in the header, no panics across the boundary (every entry point catches and converts to an error code + message).
* Structurally rich data (wallet lists, settings) travels as versioned UTF-8 JSON DTOs with `serde`, because both Qt and Kotlin can parse it without generated bindings. DTOs are versioned and validated on entry, never trusted.
* Rust panics are converted to a stable error code; the platform layer maps codes to localized text.
* Android uses the same C ABI through JNI in a thin Kotlin bridge; the core is built with `cargo-ndk` per ABI.

## Storage direction (Phase 2)

SQLite with a versioned schema and forward-only migrations, one repository per aggregate, all writes inside explicit transactions. Details and the planned schema are in `docs/DATABASE.md`. The decision between SQLCipher (page-level encryption) and application-level field encryption is **open** and recorded in `docs/DATABASE.md`.

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
