# Roadmap

Status is updated as work actually lands. A phase is complete only when it compiles, is tested and is documented.

## Summary

| Phase | Scope | Status |
| --- | --- | --- |
| 0 | Repository and architecture foundation | **Complete** |
| 1 | Rust core foundation (domain model, validation, tests) | **Complete** |
| 2 | Storage: SQLite schema, migrations, repositories | Next |
| 3 | Wallet architecture: HD derivation, watch-only, adapters trait | Planned |
| 4 | Security and encrypted `.xdvault` backup | Planned |
| 5 | Local pairing and synchronization | Planned |
| 6 | Windows application (Qt/C++) | Planned |
| 7 | Android application (Kotlin) | Planned |
| 8 | Market data (prices, USD/IRR, caching) | Planned |
| 9 | Integration, QA, packaging | Planned |

---

## Phase 0 — Repository and architecture foundation — complete

- [x] Rust workspace, shared package metadata, lint policy (`unsafe_code = "forbid"`, clippy `all`).
- [x] `.gitignore` covering Rust, Qt, Gradle, secrets and user vault artifacts.
- [x] README with an honest status statement.
- [x] Architecture document with layers, crate responsibilities and ADRs.
- [x] Development guide with commands, conventions and platform prerequisites.
- [x] Security model, database specification, backup format specification, sync protocol specification.
- [x] UI design system specification.
- [x] CI workflow running format, lint and tests.

## Phase 1 — Rust core foundation — complete

Implemented in `xopo-domain` and `xopo-core`:

- [x] `NetworkId` for 10 networks with factual metadata (display name, native symbol, native decimals, SLIP-0044 coin type, chain family, delivery tier).
- [x] `Asset` / `AssetId` / `AssetKind` / `TokenStandard`, with native asset construction from network metadata and token standards restricted to compatible networks.
- [x] `Amount`: exact atomic `u128` + decimals, decimal parsing without rounding, canonical formatting, checked add/subtract.
- [x] `Address`: network-scoped structural validation (charset, prefix, length) for all ten networks, validated again on deserialization.
- [x] `DerivationPath` / `PathSegment`: BIP-32 path parsing, hardened markers (`'` and `h`), canonical output, depth and index bounds, account/address path helpers.
- [x] `Label` and `Symbol` validation (character counting, no silent trimming or case changes).
- [x] `Timestamp`: UTC milliseconds, rejects negative values and a pre-epoch clock.
- [x] `Wallet` aggregate: kinds (standard/watch-only), per-network duplicate address rejection, account lookup by id/address/network, restore-time invariant checks.
- [x] `Account`: network/address consistency, optional derivation path, immutable identity.
- [x] Typed `WalletId` / `AccountId` (UUIDv4 newtypes).
- [x] Explicit `DomainError` for every failure; nothing is swallowed.
- [x] `Vault` aggregate in `xopo-core`: wallet creation, account creation with cross-wallet checks, lookup, watch-only reporting.
- [x] Unit tests per module plus integration tests for the vault.
- [x] JSON round-trip tests proving deserialization re-validates.

### Deliberately not implemented in Phase 1

No key generation, no mnemonic handling, no signing, no balances, no transactions, no persistence, no encryption, no networking, no UI. Nothing here can hold or move value.

## Phase 2 — Storage — next

- [ ] Decide SQLCipher vs field-level encryption (`docs/DATABASE.md`), record the decision.
- [ ] `xopo-storage`: connection setup, pragmas, error type.
- [ ] Migration runner with `user_version`, forward-only steps, pre-migration backup.
- [ ] Schema v1 exactly as specified in `docs/DATABASE.md`.
- [ ] Repositories: wallets, accounts, assets, settings.
- [ ] Transactional write paths and read-after-crash tests.
- [ ] Integrity checks (`quick_check`, `foreign_key_check`) on open.
- [ ] Corruption and migration-failure tests.

Exit criteria: creating a vault, adding wallets/accounts, reopening the process and reading identical aggregates; corrupt file produces a clear error, never an empty database.

## Phase 3 — Wallet architecture

- [ ] Key source abstraction (`xopo-wallet`): created, imported, watch-only.
- [ ] BIP-39 mnemonic validation and checksum (using an audited crate).
- [ ] BIP-32 derivation and BIP-44/84 account/address paths for Bitcoin, Ethereum, TON, Litecoin.
- [ ] Address encoding with checksums per network, verified against published test vectors.
- [ ] `ChainAdapter` trait: network info, address validation, balance query, history query, transaction construction (later).
- [ ] Per-network adapters, independently feature-gated.
- [ ] Test-vector suite checked in with sources.

Exit criteria: derived addresses match published vectors for each network; a watch-only wallet can never reach a signing path (enforced by types, not by a UI flag).

## Phase 4 — Security and backup

- [ ] `xopo-security`: Argon2id KDF with parameter policy, AEAD helpers, zeroized buffers.
- [ ] Key hierarchy: password → KEK → random DEK; re-wrap on password change.
- [ ] Platform key protection: DPAPI (Windows), Android Keystore (Android).
- [ ] Application lock, auto-lock, failed-attempt throttling, security events.
- [ ] `.xdvault` writer/reader per `docs/BACKUP_FORMAT.md`, with full validation and restore planning.
- [ ] Tamper and truncation test matrix; golden files.

Exit criteria: a backup written on one platform restores on the other; every tampered byte is rejected; no secret is present in plaintext in the file or in logs.

## Phase 5 — Local synchronization

- [ ] Device identity (Ed25519 + X25519) in the platform keystore.
- [ ] mDNS discovery with an explicit on/off switch.
- [ ] Pairing with SAS confirmation on both devices.
- [ ] Authenticated sessions with ephemeral keys and replayed-frame rejection.
- [ ] Metadata manifest exchange, records, revisions, tombstones.
- [ ] Conflict detection/resolution with review items; revocation propagation.
- [ ] Protocol tests including MITM, replay and revoked-device cases.

Exit criteria: label/preference changes propagate both ways over Wi-Fi; a revoked device is refused; a payload cannot carry key material (schema test).

## Phase 6 — Windows application

- [ ] `xopo-ffi` C ABI crate with opaque handles, panic containment and versioned JSON DTOs.
- [ ] Qt 6 application shell: sidebar, header, routing, design tokens.
- [ ] Components from the design system with all states.
- [ ] Overview, Wallets, Portfolio, Transactions, Market, Devices & Sync, Security Center, Backups, Settings.
- [ ] DPAPI-backed unlock and auto-lock wiring.

## Phase 7 — Android application

- [ ] Kotlin app with Compose design system matching the desktop tokens.
- [ ] Bottom navigation, Overview, Wallets, Portfolio, Activity, Settings.
- [ ] JNI bridge to `xopo-core`, per-ABI builds via `cargo-ndk`.
- [ ] Biometric unlock bound to a Keystore key; pairing and sync screens.

## Phase 8 — Market data

- [ ] Provider abstraction with a documented privacy contract (symbols only).
- [ ] Price retrieval, caching with explicit staleness, failure states.
- [ ] USD and IRR with explicit rial/toman handling, source and timestamp display.
- [ ] No provider call may include an address, balance or identifier derived from wallet data.

## Phase 9 — Integration and QA

- [ ] Cross-platform integration tests, database integrity, restore and sync correctness.
- [ ] Performance budgets (cold start, large wallet lists, 10k transactions).
- [ ] Error-handling audit; packaging and signing for both platforms.
- [ ] Independent security review before any real-fund usage is permitted.

---

## Open decisions

1. **Database encryption:** SQLCipher vs application-level field encryption (must be settled before Phase 2 code).
2. **Token discovery:** are ERC-20/jetton assets added manually, or discovered through an indexer? Affects Phase 3/8 scope and privacy.
3. **Balance retrieval providers:** which RPC/indexer endpoints, and how the app avoids linking addresses to a single provider.
4. **Sync of balances/history:** whether it is ever offered as an opt-in, and how the risk is communicated.
5. **Iranian market data source:** which provider supplies USD/IRR, its licensing, and how rial/toman is presented by default.
6. **Licence:** currently MIT; confirm this is the intended distribution model for a commercial release.
