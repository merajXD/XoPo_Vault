-- Schema v2: sealed secrets.
--
-- `payload` is a nonce plus ciphertext produced by `xopo-security`, bound to its
-- `context` string. It is validated by that crate when read back, so a row
-- edited directly in the file is reported, not trusted.
--
-- The password-wrapped data key is deliberately *not* kept in this file. It
-- lives in a sidecar beside the database: the key that opens this file is inside
-- that envelope, so an envelope stored in here could never be read.
--
-- The rows are encrypted twice by design: the SQLCipher page key protects the
-- whole database, and the data key protects the payload from anyone who has the
-- database key but not the password.

CREATE TABLE sealed_secrets (
    context    TEXT PRIMARY KEY,             -- 'wallet:<uuid>'
    payload    BLOB NOT NULL,                -- nonce || ciphertext
    updated_at INTEGER NOT NULL
);
