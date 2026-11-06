-- Schema v2: the security layer's key store.
--
-- Nothing here is plaintext. `key_store.envelope` is the 90-byte password-
-- wrapped data key, and `sealed_secrets.payload` is a nonce plus ciphertext
-- bound to its `context`. Both are validated by `xopo-security` when they are
-- read back, so a row edited directly in the file is reported, not trusted.
--
-- The rows live inside the SQLCipher file, so they are encrypted twice: the
-- page encryption protects the whole database, and the envelope protects the
-- data key from someone who has the database key but not the password.

CREATE TABLE key_store (
    id         TEXT PRIMARY KEY,             -- 'vault' for the single vault key
    envelope   BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE sealed_secrets (
    context    TEXT PRIMARY KEY,             -- 'wallet:<uuid>'
    payload    BLOB NOT NULL,                -- nonce || ciphertext
    updated_at INTEGER NOT NULL
);
