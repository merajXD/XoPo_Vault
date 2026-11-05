//! Chain adapters: what the application layer is allowed to ask a chain.
//!
//! An adapter answers two questions in this phase: which network it serves and
//! whether an address is real. Balance and history queries belong to the phase
//! that introduces network access, so they are deliberately absent rather than
//! stubbed with a fake value.
//!
//! Each adapter upgrades the structural check in
//! [`xopo_domain::Address::parse`] with the network's real checksum.

use xopo_domain::{Address, ChainFamily, NetworkId};

use crate::error::WalletError;

/// A network that can validate its own addresses.
pub trait ChainAdapter: Send + Sync {
    /// Network this adapter serves.
    fn network(&self) -> NetworkId;

    /// Validates an address, checksum included.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::InvalidAddress`] for anything the network would
    /// not accept.
    fn validate_address(&self, raw: &str) -> Result<Address, WalletError>;
}

/// Bitcoin mainnet addresses.
#[cfg(feature = "bitcoin")]
#[derive(Debug, Clone, Copy, Default)]
pub struct BitcoinAdapter;

#[cfg(feature = "bitcoin")]
impl ChainAdapter for BitcoinAdapter {
    fn network(&self) -> NetworkId {
        NetworkId::Bitcoin
    }

    fn validate_address(&self, raw: &str) -> Result<Address, WalletError> {
        crate::bitcoin::parse_address(bitcoin::Network::Bitcoin, raw)?;
        Ok(Address::parse(NetworkId::Bitcoin, raw)?)
    }
}

/// An EVM chain (Ethereum, BNB Smart Chain, Polygon), which share one address
/// format.
#[cfg(feature = "ethereum")]
#[derive(Debug, Clone, Copy)]
pub struct EvmAdapter {
    network: NetworkId,
}

#[cfg(feature = "ethereum")]
impl EvmAdapter {
    /// Builds an adapter for an EVM network.
    ///
    /// # Errors
    ///
    /// Returns [`WalletError::UnsupportedNetwork`] for a non-EVM network, so a
    /// Bitcoin address can never be validated by this adapter.
    pub fn new(network: NetworkId) -> Result<Self, WalletError> {
        if network.family() == ChainFamily::Evm {
            Ok(Self { network })
        } else {
            Err(WalletError::UnsupportedNetwork { network })
        }
    }
}

#[cfg(feature = "ethereum")]
impl ChainAdapter for EvmAdapter {
    fn network(&self) -> NetworkId {
        self.network
    }

    fn validate_address(&self, raw: &str) -> Result<Address, WalletError> {
        crate::ethereum::parse_address(raw).map_err(|_| WalletError::InvalidAddress {
            network: self.network,
        })?;
        Ok(Address::parse(self.network, raw)?)
    }
}

/// Litecoin mainnet addresses.
#[cfg(feature = "litecoin")]
#[derive(Debug, Clone, Copy, Default)]
pub struct LitecoinAdapter;

#[cfg(feature = "litecoin")]
impl ChainAdapter for LitecoinAdapter {
    fn network(&self) -> NetworkId {
        NetworkId::Litecoin
    }

    fn validate_address(&self, raw: &str) -> Result<Address, WalletError> {
        crate::litecoin::parse_address(raw)?;
        Ok(Address::parse(NetworkId::Litecoin, raw)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "bitcoin")]
    #[test]
    fn the_bitcoin_adapter_rejects_a_bad_checksum() {
        assert!(BitcoinAdapter
            .validate_address("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyv")
            .is_err());
        assert!(BitcoinAdapter
            .validate_address("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu")
            .is_ok());
    }

    #[cfg(feature = "ethereum")]
    #[test]
    fn the_evm_adapter_only_serves_evm_networks() {
        assert!(EvmAdapter::new(NetworkId::Bitcoin).is_err());
        let adapter = EvmAdapter::new(NetworkId::Polygon).expect("polygon");
        assert_eq!(adapter.network(), NetworkId::Polygon);
        assert!(adapter
            .validate_address("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed")
            .is_ok());
    }
}
