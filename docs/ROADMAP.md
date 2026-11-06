# Roadmap

Status is updated as work actually lands. A phase is complete only when it compiles, is tested and is documented.

## Summary

| Phase | Scope | Status |
| --- | --- | --- |
| 0 | Repository and architecture foundation | **Complete** |
| 1 | Rust core foundation (domain model, validation, tests) | **Complete** |
| 2 | Storage: SQLite schema, migrations, repositories | **Complete** (asset/balance/transaction repositories follow their data sources) |
| 3 | Wallet architecture: HD derivation, watch-only, adapters trait | **In progress** |
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

## Phase 2 — Storage — complete

- [x] Decide SQLCipher vs field-level encryption → SQLCipher (ADR-008).
- [x] `xopo-storage`: connection setup, key verification, pragmas, error type.
- [x] Migration runner with `user_version`, forward-only steps, pre-migration copy.
- [x] Schema v1 as specified in `docs/DATABASE.md` (one documented deviation: nullable `key_ref`).
- [x] Wallet and account repository: transactional save, load, list, count, delete.
- [x] Whole-vault repository: atomic save of name, creation time and every wallet; rebuild through `Vault::restore`.
- [x] Vault metadata repository.
- [x] Integrity checks (`quick_check` on open, `foreign_key_check` after migrations).
- [x] Corruption, wrong-key, newer-schema, foreign-key, encryption-at-rest and two-connection tests.
- [x] Wire storage into `xopo-core` so the vault aggregate can be loaded and saved as a whole.
- [ ] Repositories for assets, balances and transactions — deferred to Phases 3 and 8, which produce that data; inventing rows with no source would be fake data.
- [ ] Automated test for the pre-migration copy (the copy itself is implemented).

Exit criteria: creating a vault, adding wallets/accounts, reopening the process and reading identical aggregates; a corrupt file produces a clear error, never an empty database. **Met**: four vault-level persistence tests cover it, and damaged files are rejected.

## Phase 3 — Wallet architecture — in progress

Implemented in `xopo-wallet` (29 tests, every address assertion copied from a published vector):

- [x] Key source abstraction: created, imported, watch-only. `WatchOnlyAccount` has no method that returns a seed, key or signing path, so watch-only is enforced by types rather than a flag.
- [x] BIP-39 generation and validation (English, 12–24 words, checksum) through the `bip39` crate; official Trezor vector #1 passes, seed is wiped on drop.
- [x] BIP-32 master, account and child derivation through `rust-bitcoin`'s implementation, with paths from `xopo_domain::DerivationPath`.
- [x] Bitcoin BIP-84: account keys, SLIP-132 `zprv`/`zpub`, first two receiving addresses and first change address match the BIP-84 vectors.
- [x] Bitcoin BIP-44 legacy P2PKH encoding (round-trip tested; no independently published legacy vector in the suite yet).
- [x] Ethereum addresses with EIP-55 checksum, matching the Hardhat derivation vector for `m/44'/60'/0'/0/{0,1,2}`.
- [x] Litecoin BIP-44 P2PKH via Base58Check version `0x30`, matching the `bip_utils` vector for `m/44'/2'/0'/0/0`.
- [x] Bitcoin testnet, signet and regtest address encoding with coin type `1'`.
- [x] `ChainAdapter` trait plus Bitcoin, EVM and Litecoin adapters, each verifying the network's real checksum; feature-gated per chain.
- [x] Test-vector suite checked in with its sources (`crates/xopo-wallet/tests/vectors.rs`).
- [ ] TON: SLIP-0010 ed25519 derivation and the wallet-contract address format (`EQ…`/`UQ…`).
- [ ] Litecoin BIP-84 bech32 (`ltc1…`) and Taproot (BIP-86).
- [ ] `ChainAdapter` balance, history and transaction methods — they need network access, which arrives in Phases 8–9.
- [x] Wire key sources into `xopo-core`: `provision::create_wallet` derives the first address of each requested network, validates it through the domain and writes the wallet only after every step succeeded, so a failure leaves the vault untouched. The key source is handed back to the caller, never stored in the vault.
- [ ] `NetworkId` has no testnet variants yet, so testnet addresses can be derived but not persisted or displayed as a network.

Exit criteria: derived addresses match published vectors for each network; a watch-only wallet can never reach a signing path. **Partially met**: three networks pass published vectors, watch-only has no key path at all, and the core now creates wallets with real derived addresses, but TON is still missing.

## Phase 4 — Security and backup

- [x] `xopo-security`: Argon2id KDF with a parameter floor (19 MiB / two passes minimum, 64 MiB / three passes by default), ChaCha20-Poly1305 helpers, redacted and zeroized key buffers.
- [x] Key hierarchy: password → KEK → random DEK; re-wrap on password change, with the envelope stored in a sidecar beside the database (it is what opens the database, so it cannot live inside it) and secrets sealed per context in `sealed_secrets` (schema v2).
- [x] Developer CLI (`xopo-cli`, binary `xopo`): create a vault, unlock it, add a demo wallet with real derived addresses, seal and reveal its phrase, change the password, and read it all back after a restart. A development tool, not the product interface.
- [x] Vault lifecycle in `xopo-engine`: create, unlock, add wallet, reveal phrase, change password, with the ordering rules (derive, seal, save) and rollback tested end to end.
- [ ] Platform key protection: DPAPI (Windows), Android Keystore (Android).
- [ ] Application lock, auto-lock, failed-attempt throttling, security events.
- [ ] `.xdvault` writer/reader per `docs/BACKUP_FORMAT.md`, with full validation and restore planning.
- [ ] Tamper and truncation test matrix; golden files.

Exit criteria: a backup written on one platform restores on the other; every tampered byte is rejected; no secret is present in plaintext in the file or in logs.

**Partially met**: the key hierarchy, its storage, an end-to-end vault lifecycle and a developer CLI
exist and are tested, but there is no application lock, no platform key protection, no attempt
throttling and no `.xdvault` backup yet.

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
6. **TON wallet contract version:** TON addresses are the hash of a specific wallet contract (v4r2, v5r1, …); which one the vault derives must be fixed before TON support lands, because changing it later orphans addresses.
7. **Testnet in the domain model:** whether `NetworkId` gains `bitcoin-testnet`-style variants, or testnet stays an adapter-level concern that is never persisted.
6. **Licence:** currently MIT; confirm this is the intended distribution model for a commercial release.
