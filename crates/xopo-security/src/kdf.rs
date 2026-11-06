//! Argon2id password-based key derivation with a documented parameter floor.

use std::fmt;

use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use crate::error::SecurityError;
use crate::key::SecretKey;

/// Length of the per-envelope Argon2 salt.
pub(crate) const SALT_LEN: usize = 16;

/// Argon2id parameters that travel with every sealed key.
///
/// The values are recorded in the envelope header, so a file written with
/// stronger parameters keeps them: raising the policy later does not make old
/// vaults unreadable, but reading a file never silently weakens the parameters
/// either. Anything below [`KdfParams::MIN_MEMORY_KIB`] and
/// [`KdfParams::MIN_ITERATIONS`] is rejected on the way in *and* on the way out,
/// which is what stops a tampered header from downgrading the password
/// stretching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl KdfParams {
    /// Policy floor for memory: 19 MiB, the OWASP Argon2id minimum.
    pub const MIN_MEMORY_KIB: u32 = 19 * 1024;
    /// Policy floor for iterations.
    pub const MIN_ITERATIONS: u32 = 2;
    /// Upper bound accepted from a header, so a corrupt file cannot ask for
    /// gigabytes of memory at unlock time.
    pub const MAX_MEMORY_KIB: u32 = 1024 * 1024;
    /// Upper bound for iterations from a header.
    pub const MAX_ITERATIONS: u32 = 16;
    /// Upper bound for lanes, which is also Argon2's own practical limit here.
    pub const MAX_PARALLELISM: u32 = 8;

    /// Builds a parameter set, rejecting anything outside the accepted range.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityError::ParameterPolicy`] when a value is below the
    /// floor or above the accepted maximum.
    pub fn new(memory_kib: u32, iterations: u32, parallelism: u32) -> Result<Self, SecurityError> {
        if memory_kib < Self::MIN_MEMORY_KIB {
            return Err(SecurityError::ParameterPolicy {
                reason: "the memory cost is below the 19 MiB floor",
            });
        }
        if memory_kib > Self::MAX_MEMORY_KIB {
            return Err(SecurityError::ParameterPolicy {
                reason: "the memory cost is above the accepted maximum",
            });
        }
        if iterations < Self::MIN_ITERATIONS {
            return Err(SecurityError::ParameterPolicy {
                reason: "fewer than two iterations is below the policy floor",
            });
        }
        if iterations > Self::MAX_ITERATIONS {
            return Err(SecurityError::ParameterPolicy {
                reason: "the iteration count is above the accepted maximum",
            });
        }
        if parallelism == 0 || parallelism > Self::MAX_PARALLELISM {
            return Err(SecurityError::ParameterPolicy {
                reason: "the parallelism must be between 1 and 8",
            });
        }
        // Argon2 itself requires at least 8 KiB of memory per lane.
        if memory_kib / parallelism < 8 {
            return Err(SecurityError::ParameterPolicy {
                reason: "the memory cost is too small for the requested parallelism",
            });
        }
        Ok(Self {
            memory_kib,
            iterations,
            parallelism,
        })
    }

    /// The default for a desktop or phone vault: 64 MiB, three passes, one lane.
    ///
    /// This is the RFC 9106 second recommended option and is intended to take a
    /// noticeable fraction of a second on the target devices. It is stored in
    /// the envelope, so it is a starting point, not a constant written into the
    /// file format.
    #[must_use]
    pub fn interactive() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }

    /// Memory cost in KiB.
    #[must_use]
    pub fn memory_kib(&self) -> u32 {
        self.memory_kib
    }

    /// Number of passes.
    #[must_use]
    pub fn iterations(&self) -> u32 {
        self.iterations
    }

    /// Number of lanes.
    #[must_use]
    pub fn parallelism(&self) -> u32 {
        self.parallelism
    }

    /// Derives the 32-byte key-encryption key for this password and salt.
    pub(crate) fn derive(
        &self,
        password: &str,
        salt: &[u8; SALT_LEN],
    ) -> Result<SecretKey, SecurityError> {
        let params = Params::new(self.memory_kib, self.iterations, self.parallelism, Some(32))
            .map_err(|_| SecurityError::ParameterPolicy {
                reason: "the key-derivation library rejected these parameters",
            })?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = Zeroizing::new([0u8; 32]);
        argon
            .hash_password_into(password.as_bytes(), salt, &mut key[..])
            .map_err(|_| SecurityError::ParameterPolicy {
                reason: "the key-derivation library rejected this salt or password",
            })?;
        Ok(SecretKey::from_bytes(*key))
    }
}

impl Default for KdfParams {
    fn default() -> Self {
        Self::interactive()
    }
}

impl fmt::Display for KdfParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Argon2id(m={} KiB, t={}, p={})",
            self.memory_kib, self.iterations, self.parallelism
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_meets_the_policy_floor() {
        let params = KdfParams::interactive();
        assert_eq!(params, KdfParams::default());
        assert_eq!(params.memory_kib(), 64 * 1024);
        assert_eq!(params.iterations(), 3);
        assert_eq!(params.parallelism(), 1);
        assert!(params.to_string().contains("Argon2id"));
    }

    #[test]
    fn weak_parameters_are_rejected() {
        assert!(KdfParams::new(8 * 1024, 3, 1).is_err());
        assert!(KdfParams::new(KdfParams::MIN_MEMORY_KIB, 1, 1).is_err());
        assert!(KdfParams::new(KdfParams::MIN_MEMORY_KIB, 3, 0).is_err());
        assert!(KdfParams::new(u32::MAX, 3, 1).is_err());
        assert!(KdfParams::new(KdfParams::MIN_MEMORY_KIB, 3, 9).is_err());
    }

    #[test]
    fn the_floor_is_accepted() {
        let params = KdfParams::new(KdfParams::MIN_MEMORY_KIB, KdfParams::MIN_ITERATIONS, 1)
            .expect("the documented floor must be usable");
        assert_eq!(params.memory_kib(), 19 * 1024);
    }

    #[test]
    fn the_same_password_and_salt_derive_the_same_key() {
        let params = KdfParams::new(19 * 1024, 2, 1).expect("floor parameters");
        let salt = [7u8; SALT_LEN];
        let first = params.derive("correct horse", &salt).expect("derive");
        let second = params.derive("correct horse", &salt).expect("derive");
        assert_eq!(first, second);

        let other = params.derive("correct horsf", &salt).expect("derive");
        assert_ne!(first, other);
    }

    #[test]
    fn a_different_salt_derives_a_different_key() {
        let params = KdfParams::new(19 * 1024, 2, 1).expect("floor parameters");
        let first = params
            .derive("same password", &[1u8; SALT_LEN])
            .expect("derive");
        let second = params
            .derive("same password", &[2u8; SALT_LEN])
            .expect("derive");
        assert_ne!(first, second);
    }
}
