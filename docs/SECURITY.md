# Security

> XoPo Vault is pre-release and has not been independently reviewed.
> Do not use it with real funds, and never enter a real seed phrase or private key into any build.

## What must be protected

| Asset | Sensitivity | Where it must live |
| --- | --- | --- |
| Seed phrases / mnemonic words | Critical — full spend capability | Security layer only, sealed with a device-bound key |
| Private keys | Critical | Security layer only, sealed at rest |
| Wallet password | Critical — derives the key-encryption key | Never stored; kept in memory only while unlocked |
| Database encryption key | Critical | Wrapped by a password-derived key and, where available, an OS-protected key |
| Addresses and balances | Private | Local database; never sent to market-data providers |
| Labels, preferences | Low | Local database; eligible for Local Sync |
| Backup files `.xdvault` | Critical (encrypted) | User-chosen location, never uploaded automatically |

## Threat model

In scope:

* Theft or loss of a device, including a device that is powered on and unlocked.
* An attacker with filesystem access to the vault database or a backup file.
* A malicious or corrupted database, backup or sync payload.
* An attacker on the same local network attempting to impersonate a device or join a sync session.
* Accidentally leaking secrets through logs, crash reports, clipboard, screenshots or Git history.
* Supply-chain risk from dependencies.

Out of scope for now (documented, not silently assumed away):

* A compromised operating system or a debugger attached to an unlocked process.
* Physical attacks that extract keys from hardware.
* Malicious firmware of the device's secure element.
* Coercion of the user.

## Cryptography

Established primitives and libraries only. No project-specific algorithms.

| Purpose | Choice | Notes |
| --- | --- | --- |
| Password-based key derivation | Argon2id | Memory and iteration parameters recorded in the file header, never below a documented floor, calibrated per platform |
| Authenticated encryption | ChaCha20-Poly1305 (preferred) or AES-256-GCM | Single-use random nonces; associated data binds the header |
| Randomness | OS CSPRNG via `getrandom` / `OsRng` | Never a userspace PRNG seeded from time |
| Integrity of stored files | AEAD tag plus explicit structural validation | A file is not "valid" until authentication succeeds |
| Device identity (Phase 5) | Ed25519 signing key, X25519 key agreement | Private keys generated on-device, never exported |
| Password verification | AEAD decryption of a known verifier, or an Argon2id-derived verifier | Never a fast hash such as SHA-256 of the password |

Forbidden: MD5, SHA-1, DES/3DES, ECB mode, static IVs, password "encryption" with a hard-coded key, custom KDFs, custom hashing, steganography, "obfuscation" presented as encryption.

## Key hierarchy (planned, Phase 4)

```text
user password
   │  Argon2id(salt, params from header)
   ▼
KEK  (key-encryption key; never leaves memory)
   │  AEAD unwrap
   ▼
DEK  (database/file data key; random 256-bit, generated once)
   │  AEAD
   ▼
vault records / .xdvault payload
```

* The DEK is random and independent of the password, so a password change only re-wraps the DEK instead of rewriting the whole database.
* Optional second factor: the KEK wrap is additionally protected by DPAPI (Windows) or a Keystore-backed key (Android). Losing the OS key must never silently destroy data — the restore path and backup remain the recovery route, and the UI must say so.
* A successful unlock is proven by authenticating a verifier, not by comparing a stored hash with `==` in application code.
* Keys live in memory only while unlocked; the buffers are zeroized on lock. Rust cannot guarantee the OS kept no copies, so this is documented as defence in depth, not a guarantee.

## Application lock

* Manual lock, automatic lock after a configurable idle period, lock on OS session lock, and lock on suspend where the platform reports it.
* Biometric unlock (Android) or OS authentication (Windows Hello/DPAPI) is an *unlock of the local key*, never a replacement for the password-derived key: the password remains required after a backup is restored onto a new device.
* Failed-attempt throttling with a growing delay; the attempt counter is stored so a restart does not reset it.

## Platform facilities

* Windows: DPAPI (`CryptProtectData`) for machine/user-bound wrapping of the DEK, optional Windows Hello via `Windows.Security.Credentials` when the app is packaged.
* Android: Android Keystore with a non-exportable AES key for wrapping the DEK; `BiometricPrompt` with `CryptoObject` binding, strong biometrics only.
* Platform-specific code lives in the platform projects and in a dedicated Rust module behind a trait, so the domain and core never branch on the OS.

## Backups

* `.xdvault` is versioned, self-describing, authenticated and integrity-checked before a single record is applied. See `docs/BACKUP_FORMAT.md`.
* Restore is a plan-then-apply flow: the user sees what will be restored (wallets, accounts, settings, conflicts) and must confirm. Existing data is never overwritten silently.
* A password is required for both directions. There is no "no password" backup.

## Local synchronization

Absolute rules, enforced in the code that builds sync payloads:

1. Seed phrases, private keys, wallet passwords and any spend-capable material are **never** included in Local Sync — including between two paired devices.
2. Pairing two devices authorizes metadata exchange only.
3. Every session is mutually authenticated with device keys established during pairing, and every payload is AEAD-sealed with a session key.
4. A revoked device is rejected before any payload is parsed.
5. The protocol sends no wallet balances by default; any balance or transaction sync is a separate, explicit opt-in and is documented in `docs/SYNC_PROTOCOL.md` before it is implemented.

## Logging and diagnostics

* No secret, mnemonic word, private key, password, session key, or full address is written to logs.
* Errors carry identifiers and reasons, not payload contents.
* Crash reports, if ever added, are opt-in, local and reviewed for sensitive fields.
* Clipboard use for addresses is explicit; secrets are never placed on the clipboard by default.

## Dependencies

* Small, well-maintained crates with a clear purpose; every dependency is version-pinned in `Cargo.lock`.
* Cryptographic functionality comes from well-reviewed crates (`argon2`, `chacha20poly1305`/`aes-gcm`, `ed25519-dalek`, `x25519-dalek`, `getrandom`), never hand-written primitives.
* Adding a dependency requires a note in the pull request explaining why the existing set is insufficient.

## Reporting a vulnerability

Do not open a public issue for a security defect. Contact the maintainers privately with a description, affected version/commit, reproduction steps and impact. Never include real user secrets in a report.

## Implementation status

| Capability | Status |
| --- | --- |
| Domain validation (labels, symbols, addresses, amounts, paths) | Implemented and tested |
| Encrypted storage (SQLCipher, key supplied by the caller) | Implemented and tested; key derivation and key store are Phase 4 |
| Password KDF / AEAD / key hierarchy | Not implemented |
| Application lock, auto-lock, biometrics | Not implemented |
| `.xdvault` backup and restore | Not implemented |
| Device pairing and Local Sync | Not implemented |
| Security Center UI reporting real checks | Not implemented |

The Security Center must not display a "secure" state until the corresponding check above actually runs. This table is the source of truth for that claim.
