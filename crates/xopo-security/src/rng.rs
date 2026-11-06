//! Randomness: one source, no fallback.

use crate::error::SecurityError;

/// Fills a fixed-size buffer from the operating system's CSPRNG.
///
/// Failure is returned, never worked around: seeding a userspace generator from
/// the clock is exactly the mistake `docs/SECURITY.md` forbids.
pub(crate) fn random_bytes<const N: usize>() -> Result<[u8; N], SecurityError> {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes).map_err(|_| SecurityError::EntropySource)?;
    Ok(bytes)
}
