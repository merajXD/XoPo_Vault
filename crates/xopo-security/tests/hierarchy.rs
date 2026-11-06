//! The password → KEK → DEK → secret hierarchy, end to end.
//!
//! Argon2 runs at the policy floor here to keep the suite fast; production code
//! uses `KdfParams::interactive()`, which is checked against the floor in the
//! unit tests.

use xopo_security::{
    KdfParams, KeyEnvelope, SealedSecret, SecurityError, ENVELOPE_LEN, MAX_SECRET_LEN,
};

const PASSWORD: &str = "correct horse battery staple";
const CONTEXT: &str = "wallet:0192f0c4-0000-7000-8000-000000000001";
const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

fn params() -> KdfParams {
    KdfParams::new(19 * 1024, 2, 1).expect("the policy floor is usable")
}

#[test]
fn a_password_wraps_and_unwraps_the_data_key() {
    let (envelope, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    assert_eq!(envelope.to_bytes().len(), ENVELOPE_LEN);
    assert_eq!(envelope.params().expect("params"), params());

    let recovered = envelope.unseal(PASSWORD).expect("unseal");
    assert_eq!(recovered, data_key);
}

#[test]
fn the_wrong_password_leaves_the_data_key_out_of_reach() {
    let (envelope, _) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    assert_eq!(
        envelope.unseal("correct horse battery stapl"),
        Err(SecurityError::WrongPasswordOrCorrupt)
    );
    assert_eq!(
        envelope.unseal(""),
        Err(SecurityError::WrongPasswordOrCorrupt)
    );
}

#[test]
fn tampering_with_any_header_field_is_caught() {
    let (envelope, _) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");

    // A flipped salt or wrapped-key byte must not yield a key, and must be
    // reported exactly like a wrong password.
    for offset in [20usize, 44, 89] {
        let mut bytes = envelope.to_bytes();
        bytes[offset] ^= 0x01;
        let tampered = KeyEnvelope::from_bytes(&bytes).expect("still a valid record");
        assert_eq!(
            tampered.unseal(PASSWORD),
            Err(SecurityError::WrongPasswordOrCorrupt),
            "byte {offset} was not authenticated"
        );
    }

    // The magic and version are rejected structurally.
    let mut bytes = envelope.to_bytes();
    bytes[0] ^= 0xff;
    assert!(matches!(
        KeyEnvelope::from_bytes(&bytes),
        Err(SecurityError::Malformed { .. })
    ));

    let mut bytes = envelope.to_bytes();
    bytes[4] = 99;
    assert!(matches!(
        KeyEnvelope::from_bytes(&bytes),
        Err(SecurityError::Malformed { .. })
    ));
}

#[test]
fn a_header_cannot_downgrade_the_work_factor() {
    let (envelope, _) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");

    // 8 MiB is below the 19 MiB floor.
    let mut bytes = envelope.to_bytes();
    bytes[5..9].copy_from_slice(&(8u32 * 1024).to_be_bytes());
    assert!(matches!(
        KeyEnvelope::from_bytes(&bytes),
        Err(SecurityError::ParameterPolicy { .. })
    ));

    // One iteration is below the floor.
    let mut bytes = envelope.to_bytes();
    bytes[9..13].copy_from_slice(&1u32.to_be_bytes());
    assert!(matches!(
        KeyEnvelope::from_bytes(&bytes),
        Err(SecurityError::ParameterPolicy { .. })
    ));
}

#[test]
fn truncated_and_oversized_records_are_rejected() {
    let (envelope, _) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let bytes = envelope.to_bytes();

    assert!(matches!(
        KeyEnvelope::from_bytes(&bytes[..ENVELOPE_LEN - 1]),
        Err(SecurityError::Malformed { .. })
    ));
    let mut oversized = bytes.to_vec();
    oversized.push(0);
    assert!(matches!(
        KeyEnvelope::from_bytes(&oversized),
        Err(SecurityError::Malformed { .. })
    ));
    assert!(matches!(
        KeyEnvelope::from_bytes(&[]),
        Err(SecurityError::Malformed { .. })
    ));
}

#[test]
fn each_seal_uses_a_fresh_salt_nonce_and_data_key() {
    let (first, first_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let (second, second_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");

    assert_ne!(first.to_bytes(), second.to_bytes());
    assert_ne!(first.to_bytes()[14..30], second.to_bytes()[14..30]);
    assert_ne!(first_key, second_key);
    assert_eq!(second.unseal(PASSWORD).expect("unseal"), second_key);
}

#[test]
fn changing_the_password_keeps_the_data_key_and_its_secrets() {
    let (envelope, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");

    let new_password = "an entirely different passphrase";
    let rewrapped = KeyEnvelope::wrap(&data_key, new_password, params()).expect("rewrap");

    assert_eq!(
        envelope
            .unseal(PASSWORD)
            .expect("old password still opens the old envelope"),
        data_key
    );
    assert_eq!(
        rewrapped.unseal(PASSWORD),
        Err(SecurityError::WrongPasswordOrCorrupt),
        "the old password must not open the new envelope"
    );

    let recovered = rewrapped.unseal(new_password).expect("new password");
    assert_eq!(recovered, data_key);

    let opened = sealed.open(&recovered, CONTEXT).expect("secret survives");
    assert_eq!(&opened[..], PHRASE.as_bytes());
}

#[test]
fn a_secret_only_opens_in_its_own_context() {
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");

    assert_eq!(
        sealed.open(&data_key, "wallet:other"),
        Err(SecurityError::AuthenticationFailed)
    );
    assert_eq!(
        sealed.open(&data_key, &format!("{CONTEXT} ")),
        Err(SecurityError::AuthenticationFailed)
    );
    assert!(sealed.open(&data_key, CONTEXT).is_ok());
}

#[test]
fn a_secret_only_opens_with_its_own_data_key() {
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let (_, other_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");

    assert_eq!(
        sealed.open(&other_key, CONTEXT),
        Err(SecurityError::AuthenticationFailed)
    );
}

#[test]
fn tampering_with_a_sealed_secret_is_caught() {
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");

    let mut bytes = sealed.to_bytes();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    let tampered = SealedSecret::from_bytes(&bytes).expect("still a valid record");
    assert_eq!(
        tampered.open(&data_key, CONTEXT),
        Err(SecurityError::AuthenticationFailed)
    );

    let mut bytes = sealed.to_bytes();
    bytes[0] ^= 0x01;
    let tampered = SealedSecret::from_bytes(&bytes).expect("still a valid record");
    assert_eq!(
        tampered.open(&data_key, CONTEXT),
        Err(SecurityError::AuthenticationFailed)
    );
}

#[test]
fn secrets_survive_serialization() {
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");
    let restored = SealedSecret::from_bytes(&sealed.to_bytes()).expect("parse");
    let opened = restored.open(&data_key, CONTEXT).expect("open");
    assert_eq!(&opened[..], PHRASE.as_bytes());

    assert!(matches!(
        SealedSecret::from_bytes(&[0u8; 20]),
        Err(SecurityError::Malformed { .. })
    ));
}

#[test]
fn oversized_and_empty_secrets_are_handled() {
    let (_, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");

    assert_eq!(
        SealedSecret::seal(&data_key, CONTEXT, &vec![0u8; MAX_SECRET_LEN + 1]),
        Err(SecurityError::SecretTooLarge {
            max: MAX_SECRET_LEN
        })
    );

    let empty = SealedSecret::seal(&data_key, CONTEXT, b"").expect("seal empty");
    let opened = empty.open(&data_key, CONTEXT).expect("open empty");
    assert!(opened.is_empty());
}

#[test]
fn nothing_sensitive_is_printed() {
    let (envelope, data_key) = KeyEnvelope::seal(PASSWORD, params()).expect("seal");

    let envelope_text = format!("{envelope:?}");
    assert!(envelope_text.contains("redacted"));
    assert!(!envelope_text.contains(PASSWORD));
    assert!(!envelope_text.contains("XDVK"));

    let key_text = format!("{data_key:?}");
    assert!(!key_text.contains(PASSWORD));
    assert_eq!(key_text, "SecretKey(redacted)");

    // The ciphertext itself is not a secret, but the record must still not
    // expose the plaintext phrase.
    let sealed = SealedSecret::seal(&data_key, CONTEXT, PHRASE.as_bytes()).expect("seal secret");
    assert!(!format!("{sealed:?}").contains("abandon"));
}
