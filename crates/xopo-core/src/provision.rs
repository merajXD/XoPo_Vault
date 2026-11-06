//! Creating wallets from a key source.
//!
//! This module joins [`xopo_wallet`] (keys and address encoding) to the domain
//! aggregates: it derives one address per requested account, validates it
//! through [`xopo_domain::Address`], and only then writes the wallet into the
//! [`Vault`]. It is the only place in the core allowed to look at a mnemonic.
//!
//! ## Guarantees
//!
//! * The vault is untouched when anything fails: every fallible step runs before
//!   the first mutation, so a failed call never leaves a half-built wallet.
//! * A watch-only source creates a [`WalletKind::WatchOnly`] wallet with a single
//!   account and no derivation path of its own; there is no way to ask this
//!   module for a signing wallet from a bare address.
//! * Key material is not stored. The [`KeySource`] is handed back to the caller,
//!   which is the security layer's job to persist encrypted.
//!
//! ## Accounts and derivation paths
//!
//! An account stores its *account-level* path (`m/84'/0'/0'`), not the path of
//! one address: a [`xopo_domain::Account`] is a network account, and the address
//! recorded on it is its first receiving address (`…/0/0`). Address rotation and
//! gap-limit tracking arrive with balances and history.

use xopo_domain::{
    AccountId, Address, DerivationPath, DomainError, Label, NetworkId, Timestamp, WalletId,
    WalletKind,
};
use xopo_wallet::bitcoin::{self, BitcoinAddressType, Network as BitcoinNetwork};
use xopo_wallet::{ethereum, litecoin, KeySource, MasterKey};

use crate::{Vault, VaultError};

/// One account to create inside a new wallet.
#[derive(Clone, Debug)]
pub struct AccountRequest {
    network: NetworkId,
    label: Label,
    bitcoin_type: BitcoinAddressType,
}

impl AccountRequest {
    /// Requests the default address scheme for `network` (BIP-84 for Bitcoin).
    #[must_use]
    pub fn new(network: NetworkId, label: Label) -> Self {
        Self {
            network,
            label,
            bitcoin_type: BitcoinAddressType::NativeSegwit,
        }
    }

    /// Chooses the Bitcoin address family; ignored by other networks.
    #[must_use]
    pub fn with_bitcoin_type(mut self, bitcoin_type: BitcoinAddressType) -> Self {
        self.bitcoin_type = bitcoin_type;
        self
    }

    /// Network this account lives on.
    #[must_use]
    pub fn network(&self) -> NetworkId {
        self.network
    }

    /// Label for the account row.
    #[must_use]
    pub fn label(&self) -> &Label {
        &self.label
    }

    /// Bitcoin address family, if this is a Bitcoin account.
    #[must_use]
    pub fn bitcoin_type(&self) -> BitcoinAddressType {
        self.bitcoin_type
    }
}

/// The wallet that was written to the vault.
///
/// The source is returned rather than kept, because the vault does not hold key
/// material: the caller decides where the encrypted phrase lives.
#[derive(Debug)]
pub struct CreatedWallet {
    wallet_id: WalletId,
    accounts: Vec<AccountId>,
    source: KeySource,
}

impl CreatedWallet {
    /// Identifier of the new wallet.
    #[must_use]
    pub fn wallet_id(&self) -> WalletId {
        self.wallet_id
    }

    /// Accounts created inside the wallet, in request order.
    #[must_use]
    pub fn accounts(&self) -> &[AccountId] {
        &self.accounts
    }

    /// The key source the caller must store (or discard, for watch-only).
    #[must_use]
    pub fn source(&self) -> &KeySource {
        &self.source
    }

    /// Takes the key source out, leaving the identifiers behind.
    #[must_use]
    pub fn into_source(self) -> KeySource {
        self.source
    }
}

/// Creates a wallet and its accounts from `source`.
///
/// # Errors
///
/// * [`VaultError::NoAccounts`] when a signing wallet is requested with no
///   accounts.
/// * [`VaultError::WatchOnlyWithAccounts`] when a watch-only source is combined
///   with account requests; a watch-only wallet is defined by its address.
/// * [`VaultError::UnsupportedNetwork`] for a network the key layer cannot
///   derive yet.
/// * [`VaultError::Wallet`] for a derivation or encoding failure, and
///   [`VaultError::Domain`] for a rejected address, a duplicate address or a
///   malformed label.
pub fn create_wallet(
    vault: &mut Vault,
    source: KeySource,
    label: Label,
    passphrase: &str,
    requests: &[AccountRequest],
    created_at: Timestamp,
) -> Result<CreatedWallet, VaultError> {
    if let KeySource::WatchOnly(watched) = &source {
        if !requests.is_empty() {
            return Err(VaultError::WatchOnlyWithAccounts);
        }
        let wallet_id = vault.create_wallet(WalletKind::WatchOnly, label.clone(), created_at);
        let account = vault.add_account(
            wallet_id,
            watched.address().network(),
            label,
            watched.address().clone(),
            watched.path().cloned(),
            created_at,
        )?;
        return Ok(CreatedWallet {
            wallet_id,
            accounts: vec![account],
            source,
        });
    }

    let mnemonic = source.mnemonic().ok_or(VaultError::WatchOnlyWithAccounts)?;
    if requests.is_empty() {
        return Err(VaultError::NoAccounts);
    }

    let master = MasterKey::from_mnemonic(mnemonic, passphrase, BitcoinNetwork::Bitcoin)?;

    // Derive and validate everything first: nothing below mutates the vault.
    let mut prepared: Vec<(NetworkId, Label, Address, DerivationPath)> =
        Vec::with_capacity(requests.len());
    for request in requests {
        let (network, path, raw) = derive_account(&master, request)?;
        let address = Address::parse(network, &raw)?;
        if prepared.iter().any(|(existing_network, _, existing, _)| {
            *existing_network == network && existing == &address
        }) {
            return Err(VaultError::Domain(DomainError::DuplicateAccountAddress));
        }
        prepared.push((network, request.label.clone(), address, path));
    }

    let wallet_id = vault.create_wallet(WalletKind::Hierarchical, label, created_at);
    let mut accounts = Vec::with_capacity(prepared.len());
    for (network, account_label, address, path) in prepared {
        accounts.push(vault.add_account(
            wallet_id,
            network,
            account_label,
            address,
            Some(path),
            created_at,
        )?);
    }

    Ok(CreatedWallet {
        wallet_id,
        accounts,
        source,
    })
}

/// Derives the first receiving address of one account, plus its account path.
fn derive_account(
    master: &MasterKey,
    request: &AccountRequest,
) -> Result<(NetworkId, DerivationPath, String), VaultError> {
    match request.network {
        NetworkId::Bitcoin => {
            let address_type = request.bitcoin_type;
            let account = master.account(
                address_type.purpose(),
                bitcoin::coin_type(BitcoinNetwork::Bitcoin),
                0,
            )?;
            let raw =
                bitcoin::derive_address(master, BitcoinNetwork::Bitcoin, address_type, 0, 0, 0)?;
            Ok((NetworkId::Bitcoin, account.path().clone(), raw))
        }
        NetworkId::Ethereum => {
            let account = master.account(44, ethereum::COIN_TYPE, 0)?;
            let raw = ethereum::derive_address(master, 0, 0)?;
            Ok((NetworkId::Ethereum, account.path().clone(), raw))
        }
        NetworkId::Litecoin => {
            let account = master.account(44, litecoin::COIN_TYPE, 0)?;
            let raw = litecoin::derive_address(master, 0, 0, 0)?;
            Ok((NetworkId::Litecoin, account.path().clone(), raw))
        }
        other => Err(VaultError::UnsupportedNetwork { network: other }),
    }
}
