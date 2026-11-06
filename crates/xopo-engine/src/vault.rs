//! Creating, unlocking and using a vault.
//!
//! A [`VaultHandle`] exists only after the password has unwrapped the data key
//! and that key has opened the database, so holding one means the vault is
//! genuinely unlocked. Dropping it zeroizes the key; there is no "unlock flag"
//! to get out of sync with reality.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use xopo_core::provision::{self, AccountRequest};
use xopo_core::Vault;
use xopo_domain::{AccountId, Label, NetworkId, Timestamp, WalletId};
use xopo_security::{KdfParams, KeyEnvelope, SealedSecret, SecretKey, SecurityError};
use xopo_storage::{Database, DatabaseKey, SecretRepository, StorageError, VaultRepository};
use xopo_wallet::KeySource;
use zeroize::Zeroizing;

use crate::error::EngineError;

/// Suffix appended to the database path to find the key envelope.
pub const KEY_SUFFIX: &str = ".key";

/// The two files that make up one vault.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultPaths {
    database: PathBuf,
    key: PathBuf,
}

impl VaultPaths {
    /// Derives both paths from the database path.
    #[must_use]
    pub fn new(database: impl Into<PathBuf>) -> Self {
        let database = database.into();
        let mut key = database.clone().into_os_string();
        key.push(KEY_SUFFIX);
        Self {
            database,
            key: PathBuf::from(key),
        }
    }

    /// The SQLCipher file.
    #[must_use]
    pub fn database(&self) -> &Path {
        &self.database
    }

    /// The sidecar holding the password-wrapped data key.
    #[must_use]
    pub fn key_file(&self) -> &Path {
        &self.key
    }
}

/// The result of adding a wallet.
#[derive(Debug)]
pub struct AddedWallet {
    wallet_id: WalletId,
    accounts: Vec<AccountId>,
    sealed: bool,
}

impl AddedWallet {
    /// Identifier of the new wallet.
    #[must_use]
    pub fn wallet_id(&self) -> WalletId {
        self.wallet_id
    }

    /// Accounts created inside it.
    #[must_use]
    pub fn accounts(&self) -> &[AccountId] {
        &self.accounts
    }

    /// Whether a recovery phrase was sealed for this wallet. A watch-only wallet
    /// has none, by definition.
    #[must_use]
    pub fn sealed(&self) -> bool {
        self.sealed
    }
}

/// An unlocked vault.
pub struct VaultHandle {
    paths: VaultPaths,
    vault: Vault,
    database: Database,
    data_key: SecretKey,
}

impl fmt::Debug for VaultHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The path can contain the user's account name, and the key is secret.
        formatter
            .debug_struct("VaultHandle")
            .field("wallets", &self.vault.wallets().len())
            .finish_non_exhaustive()
    }
}

impl VaultHandle {
    /// The files behind this vault.
    #[must_use]
    pub fn paths(&self) -> &VaultPaths {
        &self.paths
    }

    /// The loaded aggregate. Reads only; writes go through the methods here.
    #[must_use]
    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    /// Schema version of the open file, for diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Storage`] if the pragma cannot be read.
    pub fn schema_version(&self) -> Result<u32, EngineError> {
        Ok(self.database.schema_version()?)
    }

    /// Adds a wallet, seals its recovery phrase and saves both in one step.
    ///
    /// The phrase is sealed under the wallet's identifier before anything is
    /// saved, so a wallet is never stored without its secret and a secret is
    /// never stored for a wallet that does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Domain`] for a bad label or address,
    /// [`EngineError::Core`] for a provisioning failure,
    /// [`EngineError::Security`] if sealing fails and [`EngineError::Storage`]
    /// if saving fails. On any failure the in-memory vault is reloaded from
    /// disk, so memory never claims a wallet the file does not have.
    pub fn add_wallet(
        &mut self,
        source: KeySource,
        label: &str,
        networks: &[NetworkId],
    ) -> Result<AddedWallet, EngineError> {
        let outcome = (|| {
            let label = Label::parse(label)?;
            let requests: Vec<AccountRequest> = networks
                .iter()
                .map(|network| AccountRequest::new(*network, label.clone()))
                .collect();
            // The BIP-39 passphrase is empty: it is a second factor the interface
            // does not offer yet, not something to invent here.
            let created = provision::create_wallet(
                &mut self.vault,
                source,
                label,
                "",
                &requests,
                Timestamp::now()?,
            )?;

            let wallet_id = created.wallet_id();
            let context = wallet_context(wallet_id);
            let sealed = match created.source().mnemonic() {
                Some(mnemonic) => {
                    let phrase = mnemonic.phrase();
                    let secret = SealedSecret::seal(&self.data_key, &context, phrase.as_bytes())?;
                    SecretRepository::save(
                        self.database.connection_mut(),
                        &context,
                        &secret,
                        Timestamp::now()?,
                    )?;
                    true
                }
                None => false,
            };

            VaultRepository::save(&mut self.database, &self.vault)?;
            Ok(AddedWallet {
                wallet_id,
                accounts: created.accounts().to_vec(),
                sealed,
            })
        })();

        if outcome.is_err() {
            // Best effort: the original error is the one worth reporting.
            if let Ok(vault) = VaultRepository::load(&self.database) {
                self.vault = vault;
            }
        }
        outcome
    }

    /// Decrypts a wallet's recovery phrase.
    ///
    /// The returned buffer wipes itself when dropped. Callers should print it
    /// only when the user explicitly asked for it.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::NoRecoveryPhrase`] for a watch-only wallet or a
    /// missing secret, and [`EngineError::Security`] when the payload does not
    /// open with this vault's key.
    pub fn reveal_phrase(&self, wallet_id: WalletId) -> Result<Zeroizing<String>, EngineError> {
        let context = wallet_context(wallet_id);
        let sealed = SecretRepository::load(self.database.connection(), &context)?
            .ok_or(EngineError::NoRecoveryPhrase(wallet_id))?;
        let bytes = sealed.open(&self.data_key, &context)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| EngineError::CorruptPhrase(wallet_id))?;
        Ok(Zeroizing::new(text.to_owned()))
    }

    /// Re-wraps the data key under a new password.
    ///
    /// Only the 90-byte envelope is rewritten: the database is not touched and
    /// the sealed phrases stay valid, which is the point of the two-level key
    /// hierarchy.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::WeakPassword`] for a short new password,
    /// [`EngineError::PasswordUnchanged`] when both are the same,
    /// [`EngineError::WrongPassword`] when the current one does not unwrap the
    /// key, and [`EngineError::Io`] if the envelope cannot be written.
    pub fn change_password(&mut self, current: &str, new: &str) -> Result<(), EngineError> {
        check_password(new)?;
        if current == new {
            return Err(EngineError::PasswordUnchanged);
        }
        let envelope = read_key_envelope(&self.paths)?;
        let unwrapped = envelope.unseal(current).map_err(map_envelope_error)?;
        // Constant-time comparison: the envelope already authenticated the
        // password, so this only guards against mixing up two vaults.
        if unwrapped != self.data_key {
            return Err(EngineError::WrongPassword);
        }
        let rewrapped = KeyEnvelope::wrap(&self.data_key, new, KdfParams::interactive())?;
        write_key_file(&self.paths.key, &rewrapped)
    }

    /// Drops the handle. The data key is zeroized as part of the drop; this
    /// method exists so callers can say "lock" out loud.
    pub fn lock(self) {}
}

/// Creates a new vault.
///
/// # Errors
///
/// Returns [`EngineError::AlreadyExists`] when either file is present,
/// [`EngineError::WeakPassword`] for a short password, and the security or
/// storage error when creation fails. A failure removes the files it created,
/// so a retry starts from a clean directory.
pub fn create_vault(
    paths: VaultPaths,
    name: &str,
    password: &str,
) -> Result<VaultHandle, EngineError> {
    create_vault_with_params(paths, name, password, KdfParams::interactive())
}

/// Creates a vault with explicit Argon2id parameters.
///
/// The default ([`KdfParams::interactive`]) is what callers should use. Tests use
/// the policy floor so a suite of vaults does not take minutes.
///
/// # Errors
///
/// As [`create_vault`].
pub fn create_vault_with_params(
    paths: VaultPaths,
    name: &str,
    password: &str,
    params: KdfParams,
) -> Result<VaultHandle, EngineError> {
    check_password(password)?;
    let label = Label::parse(name)?;
    if paths.database.exists() || paths.key.exists() {
        return Err(EngineError::AlreadyExists(paths.database));
    }
    if let Some(parent) = paths.database.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let creation = (|| {
        let (envelope, data_key) = KeyEnvelope::seal(password, params)?;
        let mut database = Database::open(
            &paths.database,
            &DatabaseKey::from_bytes(*data_key.expose()),
        )?;
        database.migrate()?;
        let vault = Vault::new(label, Timestamp::now()?);
        VaultRepository::save(&mut database, &vault)?;
        write_key_file(&paths.key, &envelope)?;
        Ok(VaultHandle {
            paths: paths.clone(),
            vault,
            database,
            data_key,
        })
    })();

    if creation.is_err() {
        remove_vault_files(&paths);
    }
    creation
}

/// Opens a vault with its password.
///
/// # Errors
///
/// Returns [`EngineError::VaultNotFound`] when the database is missing,
/// [`EngineError::MissingKeyFile`] when only the envelope is gone,
/// [`EngineError::WrongPassword`] for a wrong password or a damaged envelope,
/// and the storage error when the file itself cannot be opened.
pub fn open_vault(paths: VaultPaths, password: &str) -> Result<VaultHandle, EngineError> {
    if !paths.database.exists() {
        return Err(EngineError::VaultNotFound(paths.database));
    }
    if !paths.key.exists() {
        return Err(EngineError::MissingKeyFile(paths.key));
    }

    let envelope = read_key_envelope(&paths)?;
    let data_key = envelope.unseal(password).map_err(map_envelope_error)?;

    let mut database = Database::open(
        &paths.database,
        &DatabaseKey::from_bytes(*data_key.expose()),
    )
    .map_err(map_storage_error)?;
    database.migrate()?;
    let vault = VaultRepository::load(&database)?;

    Ok(VaultHandle {
        paths,
        vault,
        database,
        data_key,
    })
}

/// Context string a wallet's secret is bound to.
fn wallet_context(wallet_id: WalletId) -> String {
    format!("wallet:{wallet_id}")
}

fn check_password(password: &str) -> Result<(), EngineError> {
    if password.chars().count() < crate::MIN_PASSWORD_LEN {
        return Err(EngineError::WeakPassword {
            minimum: crate::MIN_PASSWORD_LEN,
        });
    }
    Ok(())
}

fn read_key_envelope(paths: &VaultPaths) -> Result<KeyEnvelope, EngineError> {
    let bytes = fs::read(&paths.key)?;
    // A file that cannot be parsed is treated exactly like a wrong password:
    // telling them apart would tell an attacker whether the file is intact.
    KeyEnvelope::from_bytes(&bytes).map_err(|_| EngineError::WrongPassword)
}

/// Writes the envelope through a temporary file, so a crash never leaves half an
/// envelope where a valid one used to be.
fn write_key_file(path: &Path, envelope: &KeyEnvelope) -> Result<(), EngineError> {
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);

    {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // The envelope is encrypted, but there is no reason to let other
            // users read (or even count guesses against) it.
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&envelope.to_bytes())?;
        file.sync_all()?;
    }

    fs::rename(&temporary, path)?;
    Ok(())
}

fn remove_vault_files(paths: &VaultPaths) {
    for path in [paths.database(), paths.key_file()] {
        let _ = fs::remove_file(path);
    }
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = paths.database().as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }
}

fn map_envelope_error(error: SecurityError) -> EngineError {
    match error {
        SecurityError::WrongPasswordOrCorrupt => EngineError::WrongPassword,
        other => EngineError::Security(other),
    }
}

fn map_storage_error(error: StorageError) -> EngineError {
    match error {
        // The envelope opened, so this means the key file and the database do
        // not belong together, or the database was replaced.
        StorageError::WrongKeyOrCorrupt => EngineError::WrongPassword,
        other => EngineError::Storage(other),
    }
}
