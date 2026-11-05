//! BIP-39 mnemonic phrases.
//!
//! The word list, checksum and PBKDF2 seed stretching come from the audited
//! `bip39` crate; this module adds the policy XoPo needs:
//!
//! * English only, and only the five standard word counts.
//! * Surrounding and repeated whitespace is collapsed before validation, which
//!   matches what people actually paste; nothing else is "corrected".
//! * The phrase never has a `Display`, `Serialize` or plain-`String` accessor
//!   that a log line could accidentally print. [`Mnemonic::phrase`] returns a
//!   [`Zeroizing`] buffer that wipes itself.
//! * The inner `bip39::Mnemonic` zeroizes its words on drop (`zeroize`
//!   feature).
//!
//! This module does not generate keys. [`Mnemonic::seed`] performs the BIP-39
//! stretching step and returns a [`Seed`] that is wiped on drop.

use std::fmt;
use std::str::FromStr;

use bip39::{Language, Mnemonic as Bip39Mnemonic};
use zeroize::{Zeroize, Zeroizing};

use crate::error::WalletError;
use crate::key::Seed;

/// Number of words in a BIP-39 phrase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WordCount {
    /// 128 bits of entropy.
    Twelve,
    /// 160 bits of entropy.
    Fifteen,
    /// 192 bits of entropy.
    Eighteen,
    /// 224 bits of entropy.
    TwentyOne,
    /// 256 bits of entropy.
    TwentyFour,
}

impl WordCount {
    /// Every accepted word count, shortest first.
    pub const ALL: [Self; 5] = [
        Self::Twelve,
        Self::Fifteen,
        Self::Eighteen,
        Self::TwentyOne,
        Self::TwentyFour,
    ];

    /// Word count as a number.
    #[must_use]
    pub fn words(self) -> usize {
        match self {
            Self::Twelve => 12,
            Self::Fifteen => 15,
            Self::Eighteen => 18,
            Self::TwentyOne => 21,
            Self::TwentyFour => 24,
        }
    }

    /// Entropy bytes this word count encodes (`words / 3 * 4`).
    #[must_use]
    pub fn entropy_bytes(self) -> usize {
        self.words() / 3 * 4
    }

    /// Resolves the word count of an existing phrase.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::UnsupportedWordCount`] for any other count.
    pub fn from_words(words: usize) -> Result<Self, WalletError> {
        Self::ALL
            .into_iter()
            .find(|count| count.words() == words)
            .ok_or(WalletError::UnsupportedWordCount { words })
    }
}

/// A validated English BIP-39 phrase.
///
/// Validation is repeated by [`Mnemonic::parse`] on every construction, so an
/// instance proves the checksum was correct when it was created.
pub struct Mnemonic {
    inner: Bip39Mnemonic,
}

impl Mnemonic {
    /// Generates a fresh phrase from operating-system entropy.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::EntropySource`] if the OS entropy source fails,
    /// or [`WalletError::InvalidMnemonic`] if the crate rejects the entropy.
    pub fn generate(word_count: WordCount) -> Result<Self, WalletError> {
        let mut entropy = vec![0_u8; word_count.entropy_bytes()];
        getrandom::getrandom(&mut entropy).map_err(|error| WalletError::EntropySource {
            detail: error.to_string(),
        })?;
        let inner = Bip39Mnemonic::from_entropy_in(Language::English, &entropy);
        entropy.zeroize();
        Ok(Self {
            inner: inner.map_err(|_| WalletError::InvalidMnemonic)?,
        })
    }

    /// Validates a phrase: known English words, correct checksum.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::EmptyMnemonic`] for blank input,
    /// [`WalletError::UnsupportedWordCount`] for a non-BIP-39 length and
    /// [`WalletError::InvalidMnemonic`] for an unknown word or bad checksum.
    pub fn parse(phrase: &str) -> Result<Self, WalletError> {
        let normalized = normalize(phrase)?;
        let words = normalized.split(' ').count();
        WordCount::from_words(words)?;
        let inner = Bip39Mnemonic::parse_in_normalized(Language::English, &normalized)
            .map_err(|_| WalletError::InvalidMnemonic)?;
        Ok(Self { inner })
    }

    /// Number of words in the phrase.
    #[must_use]
    pub fn words(&self) -> usize {
        self.inner.word_count()
    }

    /// The phrase as text, wrapped in a buffer that wipes itself on drop.
    ///
    /// This is the only way to read the words back, and it exists for the
    /// backup and restore screens. Treat the returned buffer as a secret.
    #[must_use]
    pub fn phrase(&self) -> Zeroizing<String> {
        Zeroizing::new(self.inner.to_string())
    }

    /// Stretches the phrase into a 64-byte BIP-39 seed.
    ///
    /// An empty passphrase is a supported BIP-39 choice, not an error; it just
    /// means the seed is protected by the phrase alone.
    #[must_use]
    pub fn seed(&self, passphrase: &str) -> Seed {
        Seed::new(self.inner.to_seed(passphrase))
    }
}

impl fmt::Debug for Mnemonic {
    /// Prints the shape of the secret, never the secret.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Mnemonic({} words, redacted)", self.words())
    }
}

impl FromStr for Mnemonic {
    type Err = WalletError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// Collapses whitespace and rejects blank input.
///
/// BIP-39 words are separated by a single space. People paste double spaces and
/// trailing newlines, so those are normalised; a changed word is not.
fn normalize(phrase: &str) -> Result<String, WalletError> {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    if words.is_empty() {
        return Err(WalletError::EmptyMnemonic);
    }
    Ok(words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Official BIP-39 English vector #1 (Trezor `vectors.json`).
    const VECTOR_PHRASE: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    const VECTOR_SEED: &str = "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e5349553\
1f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04";

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn matches_the_official_bip39_vector() {
        let mnemonic = Mnemonic::parse(VECTOR_PHRASE).expect("official vector parses");
        assert_eq!(mnemonic.words(), 12);
        let seed = mnemonic.seed("TREZOR");
        assert_eq!(hex(seed.as_bytes()), VECTOR_SEED);
    }

    #[test]
    fn the_same_phrase_and_passphrase_always_give_the_same_seed() {
        let mnemonic = Mnemonic::parse(VECTOR_PHRASE).expect("parse");
        assert_eq!(mnemonic.seed("").as_bytes(), mnemonic.seed("").as_bytes());
        assert_ne!(mnemonic.seed("").as_bytes(), mnemonic.seed("x").as_bytes());
    }

    #[test]
    fn whitespace_is_normalised_but_words_are_not() {
        let padded = format!("  {VECTOR_PHRASE}\n");
        let mnemonic = Mnemonic::parse(&padded).expect("normalised");
        assert_eq!(mnemonic.phrase().as_str(), VECTOR_PHRASE);
    }

    #[test]
    fn a_bad_checksum_is_rejected() {
        let tampered = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        assert_eq!(
            Mnemonic::parse(tampered).expect_err("checksum must fail"),
            WalletError::InvalidMnemonic
        );
    }

    #[test]
    fn unknown_words_and_wrong_lengths_are_rejected() {
        assert_eq!(
            Mnemonic::parse("").expect_err("empty"),
            WalletError::EmptyMnemonic
        );
        assert_eq!(
            Mnemonic::parse("abandon abandon abandon").expect_err("too short"),
            WalletError::UnsupportedWordCount { words: 3 }
        );
        assert_eq!(
            Mnemonic::parse(
                "notaword abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            )
            .expect_err("unknown word"),
            WalletError::InvalidMnemonic
        );
    }

    #[test]
    fn generated_phrases_are_valid_at_every_length() {
        for count in WordCount::ALL {
            let mnemonic = Mnemonic::generate(count).expect("generate");
            assert_eq!(mnemonic.words(), count.words());
            let reparsed = Mnemonic::parse(mnemonic.phrase().as_str()).expect("reparse");
            assert_eq!(reparsed.words(), count.words());
        }
    }

    #[test]
    fn debug_output_never_contains_words() {
        let mnemonic = Mnemonic::parse(VECTOR_PHRASE).expect("parse");
        let rendered = format!("{mnemonic:?}");
        assert!(!rendered.contains("abandon"));
        assert!(rendered.contains("12 words"));
    }
}
