# Database

> Status: **specification only.** The storage layer is Phase 2. Nothing in this document is implemented yet, and no database file is created by the current code.

## Engine and rules

* SQLite 3, bundled through `rusqlite` with the `bundled` feature so Windows and Android ship the same engine version.
* One database file per vault, in the platform's application data directory. The path is chosen by the platform layer and passed to the core, never hard-coded.
* `PRAGMA journal_mode = WAL`, `PRAGMA foreign_keys = ON`, `PRAGMA synchronous = FULL` for the main vault (durability of wallet metadata outweighs write throughput here).
* Every write happens inside an explicit transaction. A repository method either completes or leaves the database untouched.
* The schema version lives in `PRAGMA user_version`; migrations are forward-only, ordered, idempotent steps applied in one transaction each.

## Planned schema (v1)

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

## Migrations

* `xopo-storage/src/migrations/` holds ordered modules `m0001_initial`, `m0002_…`.
* Each migration is a pure SQL script plus a Rust guard that asserts the expected starting `user_version`.
* Applying migrations runs inside a single transaction; on failure the database is left at its previous version and the error is returned, never swallowed.
* Downgrades are not supported. An older application refuses to open a database written by a newer schema and says so explicitly.
* Before any migration, a copy of the database file is written next to the original (`*.pre-migration.bak`), because a migration failure on a user's vault is not recoverable by asking them to retry.

## Integrity and recovery

* On open: `PRAGMA quick_check`; a corrupt database is reported, not silently recreated.
* `PRAGMA foreign_key_check` after migrations.
* A database that fails to open must never be replaced by a fresh empty one: losing data silently is worse than refusing to start.
* Restoring from `.xdvault` is the documented recovery path.

## Encryption — open decision

Two options, to be decided before Phase 2 code is written:

| Option | Pros | Cons |
| --- | --- | --- |
| **SQLCipher** (page-level) | Whole file encrypted, including indexes and WAL; queries unchanged | Extra native build for Windows and Android; key must be supplied at open time |
| **Application-level field encryption** | No native dependency; can encrypt only sensitive columns | Metadata (labels, counts) leaks; every query must handle ciphertext; easier to leak a column by mistake |

The current preference is SQLCipher with a random DEK from the security layer, because it protects labels and structure as well as keys. The decision and its rationale will be recorded here and in `docs/ARCHITECTURE.md` before implementation.

## Test strategy

* Repository tests run against a temporary file database, each test with its own file.
* Migration tests apply every migration to an empty database and assert the resulting schema and `user_version`.
* Corruption tests truncate or flip bytes in a copy and assert a clear error rather than a panic.
* Concurrency tests open two connections and assert that transactions serialize without losing writes.
