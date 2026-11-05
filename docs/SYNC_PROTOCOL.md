# Local Sync protocol

> Status: **draft specification, not implemented.** Local Sync is Phase 5. Nothing in the current code opens a socket, advertises a service or pairs a device.

## Scope

Two-way synchronization between a Windows instance and an Android instance on the **same local network**, without any third-party service. Ordinary sync moves **metadata only**:

| Data | Synced | Notes |
| --- | --- | --- |
| Wallet labels, account labels | Yes | Last-writer-wins per field with revisions |
| Network enable/disable, display order | Yes | |
| Application preferences (currency, theme, hidden-value preference) | Yes | User-visible toggles only |
| Watch-only addresses and their metadata | Yes | Public data, but still opt-in |
| Device trust and revocation state | Yes | Revocation must propagate |
| Backup status, integrity status | Yes | Status flags, not file contents |
| Seed phrases, private keys, wallet passwords | **Never** | Not in any message, any version |
| Balances and transaction history | **No** by default | Would require a separate, documented opt-in |
| Addresses of spendable accounts | **No** by default | Leaks financial links; opt-in only |

A paired device is authorized to exchange metadata. Pairing never authorizes transferring spend-capable secrets. If encrypted backup transfer is ever added, it is a separate, explicitly authorized workflow with its own confirmation on the receiving device, and it is not part of this protocol.

## Device identity

On first launch each installation generates:

* An Ed25519 signing key pair — proves who is talking.
* An X25519 key-agreement key pair — establishes session keys.
* A random 128-bit `device_id`.

Private keys are held by the platform keystore (DPAPI / Android Keystore) and never leave the device. The public keys and fingerprints are the device's identity; `device_id` is derived from the fingerprint of the identity keys, so it cannot be spoofed by editing a database row.

## Discovery

* mDNS/DNS-SD service type `_xopo-vault._tcp.local`, with TXT records: `v=1`, `id=<device_id>`, `pk=<base64 Ed25519 public key>`, `name=<display name>`.
* Discovery only announces presence. It grants nothing: every connection is authenticated and every session requires prior pairing or an interactive pairing flow.
* Discovery is off when the user disables sync in Settings, and the service stops immediately.

## Pairing

Both devices must show and confirm the same short code. The flow is a symmetric commitment scheme; neither device can force a code on the other.

```text
A → B   hello            { version, device_id, ed25519_pub, x25519_pub, nonce_a }
B → A   hello            { version, device_id, ed25519_pub, x25519_pub, nonce_b }
A → B   commitment       { SHA-256("xopo-pair" || nonce_a || nonce_b || ed25519_pub_a || ed25519_pub_b) }
B → A   reveal           { nonce_b }
A → B   reveal           { nonce_a }
both    verify commitment; compute SAS = first 6 digits of
        SHA-256("xopo-sas" || nonce_a || nonce_b || pk_a || pk_b)
        show SAS on both screens; each side requires local confirmation
both    derive pairing_key = HKDF-SHA256(X25519(dh_priv, peer_dh_pub), salt = nonce_a||nonce_b,
        info = "xopo-pair-v1")
        exchange and verify an Ed25519 signature over the transcript with pairing_key protection
        store peer identity as trusted (public keys + device_id + timestamp)
```

* The SAS is short enough to read aloud and long enough that a man-in-the-middle must guess it: 6 digits, displayed identically on both devices, valid for a single attempt, expiring after 120 seconds.
* The user must confirm on **both** devices. One-sided confirmation stores nothing.
* Pairing records: `device_id`, both public keys, display name, platform, pairing timestamp, and the SAS-confirmed transcript hash.

## Session

```text
initiator → responder   session_hello { device_id, ephemeral_x25519_pub, nonce }
responder → initiator   session_hello { device_id, ephemeral_x25519_pub, nonce }
both                    session_key = HKDF-SHA256(
                            X25519(eph_priv, peer_eph_pub),
                            salt = nonce_i || nonce_r,
                            info = "xopo-session-v1")
                        verify each other's Ed25519 signature over the transcript
                        (ephemeral keys are signed with the long-term identity key)
```

* Ephemeral keys per session: compromising one session must not decrypt past sessions.
* Every frame after the handshake is sealed with ChaCha20-Poly1305, nonce = 96-bit counter per direction starting at zero. A repeated or out-of-order counter is rejected.
* Replay protection: monotonic counter plus a session identifier in the associated data.
* Transport is TCP on the LAN, but security does not depend on the transport: everything is authenticated at the application layer.

## Sync exchange

```text
frame: { version, type, sessionId, counter, ciphertext }
types:  hello, session_ok, manifest, manifest_ack, records, records_ack, conflict, bye, error
```

1. `manifest`: each side sends `{ entity, entity_id, revision }` for every syncable row (hashes of the manifest are compared first, so two identical devices exchange almost nothing).
2. Each side requests the entities whose remote revision is newer.
3. `records`: revisions requested, each carrying the full record and its `updated_at`, `origin_device` and `revision`.
4. Both sides apply the records inside one transaction, then send `records_ack` with the resulting revisions.
5. Interruption at any point is safe: the next session resumes from the manifest.

## Conflict detection and resolution

* Every syncable row carries `revision` (monotonic counter per row) and `origin` (device_id).
* A conflict is detected when both sides changed a row since their last common revision — that is, the remote revision is greater than the local *last-synced* revision but the local row also changed.
* Resolution v1 is per-field last-writer-wins using `updated_at`, with a deterministic tie-break on `device_id`. Wall-clock comparison is used only among paired devices, and a large clock skew (more than five minutes) downgrades the session to "conflict review required" instead of silently overwriting.
* Deletions are tombstones (`deleted_at`, retained for 180 days) so a deleted account is not resurrected by the other device.
* The UI reports how many conflicts were resolved automatically and offers an activity entry for each. Sensitive data is never involved, so automatic resolution cannot lose value — only labels and preferences.
* Anything the user must decide (for example, two different labels for the same wallet) is presented as a review item, not applied silently.

## Revocation

* Revoking a device on either side marks it revoked locally and propagates a revocation record to all other trusted devices.
* A revoked device's handshake is refused before any payload is decrypted; its identity is also blocked from re-pairing without an explicit "pair again" confirmation.
* Local revocation takes effect immediately even if the device is offline; the next session with the revoked device fails.

## Failure handling

| Situation | Behaviour |
| --- | --- |
| Wrong SAS | Pairing aborts, nothing stored |
| Peer not paired | Handshake refuses with `not_paired`; no data exchanged |
| Counter replay | Session terminated |
| Network drops mid-exchange | Records applied per transaction; resume on next connect |
| Clock skew > 5 min | Conflicts are queued for review, not auto-resolved |
| Unknown protocol version | Clear "update the other device" message; no downgrade |

## Test plan (Phase 5)

* Pairing: honest flow, SAS mismatch, replay of a reveal, timeout, one-sided confirmation.
* Session: MITM with substituted keys fails at signature verification; replayed frame rejected; revoked device refused.
* Sync: two-device round trip for labels and preferences; interruption mid-session; tombstone deletion; conflict on both sides; clock skew.
* Negative: assert that no sync payload type can carry a key, mnemonic or password — a serialization test inspects the schema of every message.
* Local network only: no message may contain a routable public address, and the service must not bind to anything but the LAN interface.
