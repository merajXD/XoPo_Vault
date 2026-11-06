//! XoPo Vault developer CLI.
//!
//! This is a temporary tool for exercising the Rust core, not the product
//! interface. It contains no vault logic: every command is a thin wrapper around
//! `xopo-engine`, which owns the ordering rules (derive, seal, save) and the
//! password handling. If a rule needs changing, it changes in the engine, not
//! here.
//!
//! ## Password input
//!
//! The password is prompted for without echo. For scripted demos only,
//! `XOPO_PASSWORD` (and `XOPO_NEW_PASSWORD` for `passwd`) can supply it instead;
//! that is documented as unsafe because a process environment is readable by the
//! same user and can end up in a crash report.
//!
//! ## Exit codes
//!
//! `0` success, `1` any other failure, `2` wrong password or damaged key file.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use xopo_domain::{Address, NetworkId, Wallet};
use xopo_engine::{create_vault, open_vault, EngineError, VaultHandle, VaultPaths};
use xopo_wallet::{KeySource, Mnemonic, WatchOnlyAccount};
use zeroize::Zeroizing;

/// Networks the provisioning path can derive today.
const DERIVABLE: &str = "bitcoin, ethereum, litecoin";

/// Writes one line to stdout, ignoring a closed pipe.
///
/// `println!` panics when stdout is gone, so `xopo unlock … | head` (or
/// `| Select-Object -First 5`) ended in a panic message instead of a quiet exit.
fn say(arguments: fmt::Arguments<'_>) {
    use std::io::Write as _;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = out.write_fmt(arguments);
    let _ = out.write_all(b"\n");
}

macro_rules! say {
    () => {
        say(format_args!(""))
    };
    ($($argument:tt)*) => {
        say(format_args!($($argument)*))
    };
}

#[derive(Parser)]
#[command(
    name = "xopo",
    version,
    about = "XoPo Vault developer CLI (not the product interface)",
    long_about = "A temporary tool for exercising the XoPo Vault core. It creates and unlocks an \
                  encrypted vault, stores a demo wallet with real derived addresses, and keeps the \
                  recovery phrase sealed. Do not use it with real funds."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create an empty vault protected by a password.
    Init {
        /// Path of the vault database, for example `demo.xopo`.
        vault: PathBuf,
        /// Display name stored in the vault.
        #[arg(long, default_value = "XoPo Vault")]
        name: String,
    },
    /// Unlock a vault, prove the password and print what it holds.
    Unlock {
        /// Path of the vault database.
        vault: PathBuf,
    },
    /// Change the password. Only the key envelope is rewritten.
    Passwd {
        /// Path of the vault database.
        vault: PathBuf,
    },
    /// Build a ready-to-open demo vault with one sample wallet, no prompts.
    ///
    /// The wallet comes from the public BIP-39 test vector, never from a real
    /// seed phrase, and the password is generated and printed once. This is the
    /// quickest way to see a vault and its addresses on a new machine.
    Demo {
        /// Directory that will hold `demo.xopo` and `demo.xopo.key`.
        directory: PathBuf,
        /// Display name stored in the vault.
        #[arg(long, default_value = "XoPo Demo Vault")]
        name: String,
        /// Password to use instead of generating one. Insecure; demos only.
        #[arg(long)]
        password: Option<String>,
    },
    /// Wallet commands.
    Wallet {
        #[command(subcommand)]
        action: WalletAction,
    },
}

#[derive(Subcommand)]
enum WalletAction {
    /// Import a demo wallet from a phrase, or watch one address with no key.
    Add {
        /// Path of the vault database.
        vault: PathBuf,
        /// Label shown for the wallet and its accounts.
        #[arg(long)]
        label: String,
        /// Comma-separated networks, for example `bitcoin,ethereum`.
        #[arg(long = "network", value_delimiter = ',', default_value = "bitcoin")]
        networks: Vec<String>,
        /// File holding the recovery phrase. Without it the phrase is prompted.
        #[arg(long)]
        phrase_file: Option<PathBuf>,
        /// Watch this address instead of importing a phrase.
        #[arg(long, conflicts_with = "phrase_file")]
        watch: Option<String>,
    },
    /// List wallets and their accounts.
    List {
        /// Path of the vault database.
        vault: PathBuf,
    },
    /// Show one wallet, and optionally its recovery phrase.
    Show {
        /// Path of the vault database.
        vault: PathBuf,
        /// Wallet identifier.
        #[arg(long)]
        id: String,
        /// Print the sealed recovery phrase. It is a secret.
        #[arg(long)]
        phrase: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let code = error.exit_code();
            eprintln!("error: {error}");
            ExitCode::from(code)
        }
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Init { vault, name } => init(vault, &name),
        Command::Unlock { vault } => unlock(vault),
        Command::Passwd { vault } => passwd(vault),
        Command::Demo {
            directory,
            name,
            password,
        } => demo(directory, &name, password),
        Command::Wallet { action } => wallet(action),
    }
}

fn init(vault: PathBuf, name: &str) -> Result<(), CliError> {
    let password = asked_password("New vault password: ")?;
    if std::env::var_os("XOPO_PASSWORD").is_none() {
        let again = asked_password("Repeat password: ")?;
        if *again != *password {
            return Err(CliError::Message("the passwords do not match".into()));
        }
    }

    let paths = VaultPaths::new(&vault);
    let handle = create_vault(paths, name, &password)?;
    say!("Vault created.");
    say!("  name      : {}", handle.vault().name());
    say!("  database  : {}", handle.paths().database().display());
    say!("  key file  : {}", handle.paths().key_file().display());
    say!(
        "  schema    : v{}",
        handle.schema_version().unwrap_or_default()
    );
    say!();
    say!("The key file holds the data key wrapped with your password, using Argon2id");
    say!("at its default parameters. Keep both files; either one alone is useless.");
    handle.lock();
    Ok(())
}

fn unlock(vault: PathBuf) -> Result<(), CliError> {
    let password = asked_password("Vault password: ")?;
    let handle = open_vault(VaultPaths::new(vault), &password)?;
    print_handle(&handle);
    handle.lock();
    Ok(())
}

fn passwd(vault: PathBuf) -> Result<(), CliError> {
    let current = asked_password("Current password: ")?;
    let mut handle = open_vault(VaultPaths::new(&vault), &current)?;

    let new = asked_password_from("XOPO_NEW_PASSWORD", "New password: ")?;
    if std::env::var_os("XOPO_NEW_PASSWORD").is_none() {
        let again = asked_password("Repeat new password: ")?;
        if *again != *new {
            return Err(CliError::Message("the new passwords do not match".into()));
        }
    }

    handle.change_password(&current, &new)?;
    say!("Password changed.");
    say!("Only the key envelope was rewritten; the vault and its sealed secrets are untouched.");
    handle.lock();
    Ok(())
}

/// Official BIP-39 English vector #1: public data, never a real wallet.
const DEMO_PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

fn demo(directory: PathBuf, name: &str, password: Option<String>) -> Result<(), CliError> {
    fs::create_dir_all(&directory).map_err(|error| {
        CliError::Message(format!("cannot create {}: {error}", directory.display()))
    })?;

    let (password, generated) = match password {
        Some(password) => (Zeroizing::new(password), false),
        None => (Zeroizing::new(random_password()?), true),
    };

    let paths = VaultPaths::new(directory.join("demo.xopo"));
    let mut handle = create_vault(paths, name, &password)?;
    let added = handle.add_wallet(
        KeySource::Imported(Mnemonic::parse(DEMO_PHRASE)?),
        "Demo Wallet",
        &[NetworkId::Bitcoin, NetworkId::Ethereum],
    )?;

    let database = handle.paths().database().display().to_string();
    say!("Demo vault created.");
    say!("  database : {database}");
    say!("  key file : {}.key", database);
    say!(
        "  wallet   : {} ({})",
        added.wallet_id(),
        added.accounts().len()
    );
    say!();
    if generated {
        say!("Password for this demo vault, generated and shown only once:");
        say!("  {}", password.as_str());
        say!();
    }
    say!("The wallet uses the public BIP-39 test vector, not a real seed phrase.");
    say!("Addresses are derived, so they hold nothing; do not send funds to them.");
    say!();
    say!("Next:");
    say!("  xopo unlock {database}");
    say!("  xopo wallet show {database} --id {}", added.wallet_id());
    handle.lock();
    Ok(())
}

/// A throwaway demo password from the OS random source, as 64 hex characters.
fn random_password() -> Result<String, CliError> {
    use std::fmt::Write as _;
    let key = xopo_security::SecretKey::generate()?;
    let mut text = String::with_capacity(64);
    for byte in key.expose() {
        let _ = write!(text, "{byte:02x}");
    }
    Ok(text)
}

fn wallet(action: WalletAction) -> Result<(), CliError> {
    match action {
        WalletAction::Add {
            vault,
            label,
            networks,
            phrase_file,
            watch,
        } => add_wallet(vault, &label, &networks, phrase_file.as_deref(), watch),
        WalletAction::List { vault } => list_wallets(vault),
        WalletAction::Show { vault, id, phrase } => show_wallet(vault, &id, phrase),
    }
}

fn add_wallet(
    vault: PathBuf,
    label: &str,
    networks: &[String],
    phrase_file: Option<&Path>,
    watch: Option<String>,
) -> Result<(), CliError> {
    let password = asked_password("Vault password: ")?;
    let mut handle = open_vault(VaultPaths::new(vault), &password)?;

    let (source, networks) = match watch {
        // A watched address is defined by itself; account requests would claim a
        // signing path that does not exist.
        Some(address) => {
            let network = parse_networks(networks)?
                .first()
                .copied()
                .ok_or_else(|| CliError::Message("a watched address needs one network".into()))?;
            let address = Address::parse(network, &address)?;
            (
                KeySource::WatchOnly(WatchOnlyAccount::new(address)),
                Vec::new(),
            )
        }
        None => {
            let phrase = read_phrase(phrase_file)?;
            let mnemonic = Mnemonic::parse(phrase.as_str())?;
            (KeySource::Imported(mnemonic), parse_networks(networks)?)
        }
    };

    let added = handle.add_wallet(source, label, &networks)?;
    say!("Wallet added.");
    say!("  id       : {}", added.wallet_id());
    say!("  accounts : {}", added.accounts().len());
    say!(
        "  phrase   : {}",
        if added.sealed() {
            "sealed with the vault's data key"
        } else {
            "none (watch-only wallet)"
        }
    );
    print_handle(&handle);
    handle.lock();
    Ok(())
}

fn list_wallets(vault: PathBuf) -> Result<(), CliError> {
    let password = asked_password("Vault password: ")?;
    let handle = open_vault(VaultPaths::new(vault), &password)?;
    print_handle(&handle);
    handle.lock();
    Ok(())
}

fn show_wallet(vault: PathBuf, id: &str, reveal: bool) -> Result<(), CliError> {
    let password = asked_password("Vault password: ")?;
    let handle = open_vault(VaultPaths::new(vault), &password)?;

    let wallet_id = id
        .parse()
        .map_err(|_| CliError::Message(format!("'{id}' is not a wallet identifier")))?;
    let wallet = handle
        .vault()
        .wallets()
        .iter()
        .find(|wallet| wallet.id() == wallet_id)
        .ok_or_else(|| CliError::Message(format!("no wallet with id {id}")))?;

    say!("Wallet \"{}\"", wallet.label());
    say!("  id       : {}", wallet.id());
    say!("  kind     : {}", wallet.kind().display_name());
    say!("  accounts : {}", wallet.account_count());
    for account in wallet.accounts() {
        say!();
        say!("  account \"{}\"", account.label());
        say!("    id      : {}", account.id());
        say!("    network : {}", account.network());
        say!("    address : {}", account.address().as_str());
        say!(
            "    path    : {}",
            account
                .derivation()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".into())
        );
    }

    if reveal {
        let phrase = handle.reveal_phrase(wallet_id)?;
        say!();
        say!("Recovery phrase (a secret; anyone who sees it owns this wallet):");
        say!("{}", phrase.as_str());
    }
    handle.lock();
    Ok(())
}

fn print_handle(handle: &VaultHandle) {
    let vault = handle.vault();
    say!("Unlocked \"{}\"", vault.name());
    say!("  wallets  : {}", vault.wallets().len());
    say!("  accounts : {}", vault.account_count());
    say!(
        "  schema   : v{}",
        handle.schema_version().unwrap_or_default()
    );
    for wallet in vault.wallets() {
        print_wallet(wallet);
    }
    if vault.wallets().is_empty() {
        say!();
        say!("No wallets yet. Add one with:");
        say!("  xopo wallet add <vault> --label Demo --network bitcoin,ethereum");
    }
}

fn print_wallet(wallet: &Wallet) {
    let id = wallet.id().to_string();
    say!();
    say!(
        "  {}  \"{}\"  ({})",
        &id[..8.min(id.len())],
        wallet.label(),
        wallet.kind().display_name()
    );
    for account in wallet.accounts() {
        say!(
            "    {:<9} {:<46} {}",
            account.network().to_string(),
            account.address().as_str(),
            account
                .derivation()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".into())
        );
    }
}

fn parse_networks(raw: &[String]) -> Result<Vec<NetworkId>, CliError> {
    raw.iter()
        .map(|id| {
            NetworkId::from_id(id).ok_or_else(|| {
                CliError::Message(format!(
                    "unknown network '{id}'; this build derives {DERIVABLE}"
                ))
            })
        })
        .collect()
}

fn read_phrase(path: Option<&Path>) -> Result<Zeroizing<String>, CliError> {
    match path {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|error| {
                CliError::Message(format!("cannot read {}: {error}", path.display()))
            })?;
            Ok(Zeroizing::new(text))
        }
        None => {
            if std::env::var_os("XOPO_PASSWORD").is_some() {
                return Err(CliError::Message(
                    "XOPO_PASSWORD is set for scripting, so the phrase has no prompt; \
                     pass --phrase-file instead"
                        .into(),
                ));
            }
            let phrase = rpassword::prompt_password("Recovery phrase (hidden): ")
                .map_err(|error| CliError::Message(format!("cannot read phrase: {error}")))?;
            Ok(Zeroizing::new(phrase))
        }
    }
}

/// Reads a password without echoing it, or from `XOPO_PASSWORD` for demos.
fn asked_password(prompt: &str) -> Result<Zeroizing<String>, CliError> {
    asked_password_from("XOPO_PASSWORD", prompt)
}

/// Reads a password from `variable`, or prompts for it.
///
/// The environment variable is a scripting escape hatch for demos, never a
/// feature: a process environment is readable by the same user and shows up in
/// crash dumps.
fn asked_password_from(variable: &str, prompt: &str) -> Result<Zeroizing<String>, CliError> {
    if let Some(value) = std::env::var_os(variable) {
        let value = value
            .into_string()
            .map_err(|_| CliError::Message(format!("{variable} is not valid UTF-8")))?;
        return Ok(Zeroizing::new(value));
    }
    let value = rpassword::prompt_password(prompt)
        .map_err(|error| CliError::Message(format!("cannot read password: {error}")))?;
    Ok(Zeroizing::new(value))
}

/// Failures the CLI can report, with the engine's error kept intact.
enum CliError {
    Engine(EngineError),
    Message(String),
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Engine(EngineError::WrongPassword) => 2,
            _ => 1,
        }
    }
}

impl From<EngineError> for CliError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

impl From<xopo_domain::DomainError> for CliError {
    fn from(error: xopo_domain::DomainError) -> Self {
        Self::Engine(EngineError::Domain(error))
    }
}

impl From<xopo_security::SecurityError> for CliError {
    fn from(error: xopo_security::SecurityError) -> Self {
        Self::Engine(EngineError::Security(error))
    }
}

impl From<xopo_wallet::WalletError> for CliError {
    fn from(error: xopo_wallet::WalletError) -> Self {
        Self::Engine(EngineError::Wallet(error))
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => write!(formatter, "{error}"),
            Self::Message(message) => formatter.write_str(message),
        }
    }
}
