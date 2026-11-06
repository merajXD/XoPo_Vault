# Database

> Status: **implemented through schema v2 (Phases 2–4).**
> The engine choice, encryption, migrations and integrity checks exist and are tested for wallets,
> accounts, metadata, the whole-vault aggregate and the security layer's key store.
> Repositories for assets, balances and transactions arrive with the phases that produce that data
> (chain adapters and market data); their tables exist, their access code does not.

## Engine and rules

* SQLite 3, bundled through `rusqlite` with the `bundled` feature so Windows and Android ship the same engine version.
* One database file per vault, in the platform's application data directory. The path is chosen by the platform layer and passed to the core, never hard-coded.
* `PRAGMA journal_mode = WAL`, `PRAGMA foreign_keys = ON`, `PRAGMA synchronous = FULL` for the main vault (durability of wallet metadata outweighs write throughput here).
* Every write happens inside an explicit transaction. A repository method either completes or leaves the database untouched.
* The schema version lives in `PRAGMA user_version`; migrations are forward-only, ordered, idempotent steps applied in one transaction each.

## Schema v1 (implemented)

Identifiers are UUIDv4 stored as `TEXT`. Timestamps are UTC milliseconds (`INTEGER`). Amounts are atomic units (`TEXT`, because `u128` exceeds SQLite's signed 64-bit range) plus the asset's decimals.

```sql
-- Vault-level metadata: schema markers, vault name, created time.
CREATE TABLE vault_meta (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL
);

-- Networks are static in the domain; this table stores per-vault preferences
-- such as enabled/disabled state and the user's display order.
CREATE TABLE network_settings (
    network_id   TEXT PRIMARY KEY,
    enabled      INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    sort_order   INTEGER NOT NULL DEFAULT 0
);

-- Assets the user actually holds or watches. Native assets are rows too, so a
-- balance always points at an asset row.
CREATE TABLE assets (
    id              TEXT PRIMARY KEY,             -- canonical "network:native" or "network:erc20:0x…"
    network_id      TEXT NOT NULL,
    kind            TEXT NOT NULL CHECK (kind IN ('native', 'token')),
    token_standard  TEXT CHECK (token_standard IN ('erc-20', 'ton-jetton')),
    contract        TEXT,
    symbol          TEXT NOT NULL,
    name            TEXT NOT NULL,
    decimals        INTEGER NOT NULL CHECK (decimals BETWEEN 0 AND 18),
    CHECK ((kind = 'native' AND token_standard IS NULL AND contract IS NULL)
        OR (kind = 'token' AND token_standard IS NOT NULL AND contract IS NOT NULL))
);

CREATE TABLE wallets (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('hierarchical', 'watch-only')),
    -- Reference into the security layer's key store; never a key itself.
    key_ref     TEXT,
    created_at  INTEGER NOT NULL,
    CHECK ((kind = 'watch-only') OR key_ref IS NOT NULL)
);

CREATE TABLE accounts (
    id           TEXT PRIMARY KEY,
    wallet_id    TEXT NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    network_id   TEXT NOT NULL,
    label        TEXT NOT NULL,
    address      TEXT NOT NULL,
    derivation   TEXT,
    created_at   INTEGER NOT NULL,
    UNIQUE (wallet_id, network_id, address)
);

CREATE INDEX accounts_by_network ON accounts (network_id);

-- Last observed balances. A row's presence means "observed at updated_at";
-- absence means "unknown", which the UI must show as unknown, not as zero.
CREATE TABLE balances (
    account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    asset_id    TEXT NOT NULL REFERENCES assets(id),
    atomic      TEXT NOT NULL,
    decimals    INTEGER NOT NULL CHECK (decimals BETWEEN 0 AND 18),
    updated_at  INTEGER NOT NULL,
    source      TEXT NOT NULL,
    PRIMARY KEY (account_id, asset_id)
);

CREATE TABLE transactions (
    id             TEXT PRIMARY KEY,          -- canonical chain txid when known
    account_id     TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    asset_id       TEXT NOT NULL REFERENCES assets(id),
    direction      TEXT NOT NULL CHECK (direction IN ('incoming', 'outgoing', 'self')),
    status         TEXT NOT NULL CHECK (status IN ('pending', 'confirmed', 'failed')),
    atomic         TEXT NOT NULL,
    decimals       INTEGER NOT NULL,
    fee_atomic     TEXT,
    counterparty   TEXT,
    block_height   INTEGER,
    occurred_at    INTEGER NOT NULL,
    observed_at    INTEGER NOT NULL,
    UNIQUE (account_id, id)
);

CREATE TABLE devices (
    id             TEXT PRIMARY KEY,          -- device public identity key fingerprint
    display_name   TEXT NOT NULL,
    platform       TEXT NOT NULL CHECK (platform IN ('windows', 'android')),
    public_key     TEXT NOT NULL,
    paired_at      INTEGER NOT NULL,
    last_seen_at   INTEGER,
    revoked_at     INTEGER
);

CREATE TABLE sync_state (
    entity      TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    revision    INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    origin      TEXT NOT NULL,
    PRIMARY KEY (entity, entity_id)
);

CREATE TABLE security_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT NOT NULL,
    severity    TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    occurred_at INTEGER NOT NULL,
    device_id   TEXT,
    detail      TEXT                             -- no secrets, no addresses
);

CREATE TABLE backups (
    id             TEXT PRIMARY KEY,
    created_at     INTEGER NOT NULL,
    format_version INTEGER NOT NULL,
    location_hint  TEXT,
    verified_at    INTEGER,
    record_count   INTEGER
);
```

## Schema v2 — sealed secrets (implemented)

The security layer needs somewhere to keep the payloads it seals with the vault's data key. They are
opaque, already-authenticated blobs; the storage layer cannot decrypt them.

```sql
CREATE TABLE sealed_secrets (
    context    TEXT PRIMARY KEY,             -- 'wallet:<uuid>'
    payload    BLOB NOT NULL,                -- nonce || ciphertext
    updated_at INTEGER NOT NULL
);
```

* Every blob is validated by `xopo-security` when it is read back; a row edited directly in the file
  returns an explicit `InvalidRecord` error instead of being trusted.
* The rows sit inside the SQLCipher file, so they are encrypted twice: by the database key and by the
  data key. Losing either one leaves the secrets unreadable.
* `wallets.key_ref` is still nullable: nothing writes a reference yet, and a placeholder would claim
  a key exists when it does not. The constraint returns once the creation flow sets it.

### The wrapped data key lives outside the database

An earlier draft of this schema put the 90-byte key envelope in a `key_store` table. That cannot
work: the envelope holds the database key, so it could only be read after the database was already
open — a cycle. The envelope therefore lives in `<vault>.key` beside the database, and schema v2
keeps only `sealed_secrets`. No vault created outside the test suite ever used the earlier shape.

## Migrations

* `xopo-storage/src/migrations/` holds ordered SQL scripts `m0001_initial`, `m0002_sealed_secrets`.
* Each migration is a pure SQL script plus a Rust guard that asserts the expected starting `user_version`.
* Applying migrations runs inside a single transaction; on failure the database is left at its previous version and the error is returned, never swallowed.
* Downgrades are not supported. An older application refuses to open a database written by a newer schema and says so explicitly.
* Before any migration, a copy of the database file is written next to the original (`*.pre-migration.bak`), because a migration failure on a user's vault is not recoverable by asking them to retry.

## Integrity and recovery

* On open: `PRAGMA quick_check`; a corrupt database is reported, not silently recreated.
* `PRAGMA foreign_key_check` after migrations.
* A database that fails to open must never be replaced by a fresh empty one: losing data silently is worse than refusing to start.
* Restoring from `.xdvault` is the documented recovery path.

## Encryption — decided: SQLCipher

**Decision (ADR-008): page-level SQLCipher with a raw 256-bit key supplied at open time.**

| Option | Pros | Cons |
| --- | --- | --- |
| **SQLCipher** (chosen) | Whole file encrypted, including indexes and the WAL; queries unchanged; no column can be forgotten | Native build for Windows and Android; the key must be supplied at open time |
| Application-level field encryption (rejected) | No native dependency; can encrypt only sensitive columns | Metadata (labels, counts) leaks; every query must handle ciphertext; one forgotten column silently leaks |

The reason is the failure mode: with field encryption, a single forgotten column leaks permanently and
silently. With SQLCipher a mistake is loud (the file does not open). The cost is a native build, which is
accepted and documented in `README.md`.

The key itself is **not** produced here. `DatabaseKey` only wraps 32 bytes given by the security layer;
Phase 4 adds the Argon2id-derived KEK and the random DEK, and passes the DEK to `Database::open`. Until
then no password, key generation or key store exists — the encryption is real, but the key management
around it is not.

## Implementation status (Phase 2)

| Piece | Status |
| --- | --- |
| SQLCipher file, key verification, WAL, foreign keys, `synchronous = FULL` | Implemented and tested |
| `quick_check` on open, `foreign_key_check` after migrations | Implemented and tested |
| Schema v1 (`m0001_initial.sql`), `user_version` guard, forward-only | Implemented and tested |
| Pre-migration copy (`*.pre-migration.bak`) | Implemented, not covered by an automated test |
| Wallet repository: transactional save, load, list, count, delete | Implemented and tested |
| Whole-vault repository: atomic save of name, time and every wallet; rebuild on load | Implemented and tested |
| Vault metadata repository | Implemented and tested |
| Sealed secrets (schema v2) and the sidecar key envelope | Implemented and tested |
| Vault recovery: `.xdvault` backup | Specification only (Phase 4) |
| Two connections writing the same file | Implemented and tested (WAL, `busy_timeout`) |
| Asset, balance, transaction, device, sync, security-event, backup repositories | Specification only |
| Automated test for the pre-migration copy | Not written (the copy itself is implemented) |

Deviation from the original schema text: `wallets.key_ref` is nullable for every wallet kind. The key
store does not exist before Phase 4, and a placeholder reference would claim a key is available when it
is not. The constraint returns in a later migration.

## Test strategy

* Repository tests run against a temporary file database, each test with its own file.
* Migration tests apply every migration to an empty database and assert the resulting schema and `user_version`.
* Corruption tests truncate or flip bytes in a copy and assert a clear error rather than a panic.
* Concurrency tests open two connections and assert that transactions serialize without losing writes.
