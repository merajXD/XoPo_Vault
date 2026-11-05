-- Schema v1.
--
-- The shape follows `docs/DATABASE.md`. Identifiers are UUID text, timestamps
-- are UTC milliseconds and amounts are stored as atomic decimal text because
-- `u128` exceeds SQLite's signed 64-bit integer.
--
-- Deliberate deviation from the document: `wallets.key_ref` has no
-- `NOT NULL`-style constraint for standard wallets yet. The key store does not
-- exist before Phase 4, and writing a placeholder reference would claim that a
-- key is available when it is not. The column stays nullable and documented;
-- the constraint returns in a later migration once real key references exist.

CREATE TABLE vault_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE network_settings (
    network_id TEXT PRIMARY KEY,
    enabled    INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE assets (
    id             TEXT PRIMARY KEY,
    network_id     TEXT NOT NULL,
    kind           TEXT NOT NULL CHECK (kind IN ('native', 'token')),
    token_standard TEXT CHECK (token_standard IN ('erc-20', 'ton-jetton')),
    contract       TEXT,
    symbol         TEXT NOT NULL,
    name           TEXT NOT NULL,
    decimals       INTEGER NOT NULL CHECK (decimals BETWEEN 0 AND 18),
    CHECK (
        (kind = 'native' AND token_standard IS NULL AND contract IS NULL)
        OR (kind = 'token' AND token_standard IS NOT NULL AND contract IS NOT NULL)
    )
);

CREATE TABLE wallets (
    id         TEXT PRIMARY KEY,
    label      TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('hierarchical', 'watch-only')),
    key_ref    TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE accounts (
    id          TEXT PRIMARY KEY,
    wallet_id   TEXT NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    network_id  TEXT NOT NULL,
    label       TEXT NOT NULL,
    address     TEXT NOT NULL,
    derivation  TEXT,
    created_at  INTEGER NOT NULL,
    UNIQUE (wallet_id, network_id, address)
);

CREATE INDEX accounts_by_network ON accounts (network_id);

CREATE TABLE balances (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    asset_id   TEXT NOT NULL REFERENCES assets(id),
    atomic     TEXT NOT NULL,
    decimals   INTEGER NOT NULL CHECK (decimals BETWEEN 0 AND 18),
    updated_at INTEGER NOT NULL,
    source     TEXT NOT NULL,
    PRIMARY KEY (account_id, asset_id)
);

CREATE TABLE transactions (
    id           TEXT PRIMARY KEY,
    account_id   TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    asset_id     TEXT NOT NULL REFERENCES assets(id),
    direction    TEXT NOT NULL CHECK (direction IN ('incoming', 'outgoing', 'self')),
    status       TEXT NOT NULL CHECK (status IN ('pending', 'confirmed', 'failed')),
    atomic       TEXT NOT NULL,
    decimals     INTEGER NOT NULL,
    fee_atomic   TEXT,
    counterparty TEXT,
    block_height INTEGER,
    occurred_at  INTEGER NOT NULL,
    observed_at  INTEGER NOT NULL,
    UNIQUE (account_id, id)
);

CREATE TABLE devices (
    id           TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    platform     TEXT NOT NULL CHECK (platform IN ('windows', 'android')),
    public_key   TEXT NOT NULL,
    paired_at    INTEGER NOT NULL,
    last_seen_at INTEGER,
    revoked_at   INTEGER
);

CREATE TABLE sync_state (
    entity    TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    revision  INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    origin    TEXT NOT NULL,
    PRIMARY KEY (entity, entity_id)
);

CREATE TABLE security_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT NOT NULL,
    severity    TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    occurred_at INTEGER NOT NULL,
    device_id   TEXT,
    detail      TEXT
);

CREATE TABLE backups (
    id             TEXT PRIMARY KEY,
    created_at     INTEGER NOT NULL,
    format_version INTEGER NOT NULL,
    location_hint  TEXT,
    verified_at    INTEGER,
    record_count   INTEGER
);
