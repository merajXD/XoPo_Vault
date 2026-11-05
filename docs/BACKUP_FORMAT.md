# `.xdvault` backup format

> Status: **draft specification, not implemented.** The backup layer is Phase 4. No code in this repository reads or writes `.xdvault` files yet.

## Goals

* A single portable file containing everything needed to restore a vault: wallets, accounts, assets, preferences and references to key material.
* Password-protected with a memory-hard KDF, authenticated end to end, and versioned for forward evolution.
* Detectible tampering: a modified byte anywhere must fail authentication, not produce a subtly different vault.
* Restorable on the other platform (Windows ↔ Android) without a server.

## Container layout

```text
offset  size        field
0       8           magic: "XOPOVAUL" (ASCII)
8       2           format version (u16, big-endian), currently 1
10      2           header length in bytes (u16, big-endian)
12      H           header: canonical UTF-8 JSON, H bytes
12+H    12          nonce (96-bit, random per file)
…       N           ciphertext of the payload
…       16          AEAD tag (ChaCha20-Poly1305) or 16-byte GCM tag
```

The trailing tag is part of the AEAD output; it is listed separately because it is what a validator checks first.

### Header (JSON, canonical form)

```json
{
  "format": "xdvault",
  "version": 1,
  "kdf": {
    "algorithm": "argon2id",
    "version": 19,
    "memoryKib": 65536,
    "iterations": 3,
    "parallelism": 1,
    "salt": "<base64, 16 bytes>"
  },
  "cipher": {
    "algorithm": "chacha20poly1305",
    "nonce": "<base64, 12 bytes>"
  },
  "createdAt": 1767225600000,
  "createdBy": { "platform": "windows", "appVersion": "0.1.0" },
  "deviceId": "<public identity fingerprint, not a secret>",
  "payload": { "encoding": "json", "compression": "none", "recordCount": 42 },
  "payloadDigest": "<base64 SHA-256 of the plaintext payload, for diagnostics>",
  "notes": "optional user description"
}
```

Rules:

* The header is written with sorted keys, no insignificant whitespace, UTF-8, and no duplicate keys. It is serialized exactly as parsed for authentication.
* The **entire header** (magic, version, length and the JSON bytes) is passed as AEAD associated data. Changing any header field invalidates the tag.
* Unknown header fields are preserved when reading and are still covered by the tag. A reader refuses a `version` it does not know instead of guessing.
* KDF parameters below the documented floor are rejected; a file demanding absurd resources is rejected before allocation is attempted (a memory cap derived from the header's declared `memoryKib`).
* `payloadDigest` is diagnostic only. It never replaces AEAD verification.

## Payload

Version 1 payload is UTF-8 JSON with a top-level object:

```json
{
  "schema": 1,
  "vault": { "name": "Personal", "createdAt": 1767225600000 },
  "wallets": [ { "id": "…", "label": "Main", "kind": "hierarchical", "createdAt": 0, "accounts": [ … ] } ],
  "assets": [ … ],
  "preferences": { … },
  "keyRefs": [ { "walletId": "…", "keyRef": "…", "wrappedKey": "<base64>", "wrapScheme": "argon2id+aead" } ]
}
```

* The payload never contains plaintext seed phrases or private keys. Key material is included only as ciphertext produced by the security layer, wrapped with the backup password, and it is optional per backup ("metadata only" is a supported choice the user makes explicitly).
* Addresses are included, because a restore without addresses cannot rebuild a watch-only wallet.
* Values are canonical: identifiers, timestamps as UTC milliseconds, amounts as atomic units with decimals.

## Writing a backup

1. Derive the key: `Argon2id(password, salt, params)` → 32-byte key.
2. Build the payload, serialize it, and compute its length.
3. Build the header with the salt, nonce, parameters and payload digest.
4. Seal the payload with ChaCha20-Poly1305, using the full header as associated data.
5. Write to a temporary file in the destination directory, `fsync`, then rename over the final name. A backup is never left half-written under its final name.
6. Re-open and fully verify the written file before reporting success.

## Restoring a backup

1. Read magic, version and length. Reject unknown magic or version.
2. Parse the header strictly; reject malformed base64, missing fields, duplicate keys and KDF parameters outside the allowed range.
3. Ask for the password, derive the key, verify the AEAD tag. **Nothing is written before this succeeds.**
4. Decrypt and parse the payload. Validate every entity through the domain constructors — a backup is untrusted input, even if the tag is valid.
5. Build a restore plan: what exists now, what the backup contains, what would be created, replaced or skipped. Show it to the user.
6. Apply the plan inside one transaction, or cancel. Existing data is never overwritten without explicit confirmation.
7. Record the backup in the backup history and refresh integrity status.

## Failure handling

| Situation | Behaviour |
| --- | --- |
| Wrong password / modified file | "Authentication failed" — no partial restore, no hint about which is wrong |
| Unknown format version | Clear "newer than this version" message |
| Valid header, invalid payload entities | Restore aborts; the diagnostics list the failing entities, not their contents |
| Insufficient disk space | Detected before applying; nothing is changed |
| Power loss during restore | Single transaction; database unchanged |

## Test plan (Phase 4)

* Round trip: write → verify → restore on a fresh vault, deep-equal to the source aggregates.
* Tamper matrix: flip a byte in magic, version, header JSON, salt, nonce, ciphertext and tag; every case must fail.
* Truncation at every structural boundary.
* Wrong password, empty password, and a password with non-ASCII characters (NFD/NFC input).
* Cross-platform: a file written on Windows restores on Android and vice versa.
* Resource-exhaustion headers (huge `memoryKib`, `iterations` at the cap).
* Golden files checked in under `tests/fixtures/`, with their passwords clearly marked as test-only.
