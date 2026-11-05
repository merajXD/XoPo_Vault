//! Asset identity and metadata.
//!
//! An asset is always identified by its network plus whether it is the native
//! coin or a token contract. The same ticker can exist on several networks, so
//! the symbol is metadata and never an identity.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::address::Address;
use crate::amount::MAX_DECIMALS;
use crate::error::DomainError;
use crate::network::{ChainFamily, NetworkId};
use crate::validation::{Label, Symbol};

/// Token standards supported by the product scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TokenStandard {
    /// EVM ERC-20 token.
    Erc20,
    /// TON jetton.
    TonJetton,
}

impl TokenStandard {
    /// Human-readable name.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            TokenStandard::Erc20 => "ERC-20",
            TokenStandard::TonJetton => "TON Jetton",
        }
    }

    /// Whether this standard can exist on the given network.
    #[must_use]
    pub fn is_supported_on(self, network: NetworkId) -> bool {
        match self {
            TokenStandard::Erc20 => network.family() == ChainFamily::Evm,
            TokenStandard::TonJetton => network == NetworkId::Ton,
        }
    }
}

impl fmt::Display for TokenStandard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Whether an asset is a network's native coin or a token contract.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AssetKind {
    /// The network's native coin.
    Native,
    /// A token deployed at a contract address.
    Token {
        /// Token standard of the contract.
        standard: TokenStandard,
        /// Contract (or jetton master) address on the asset's network.
        contract: Address,
    },
}

/// Identity of an asset: network plus native/token discriminator.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetId {
    network: NetworkId,
    kind: AssetKind,
}

impl AssetId {
    /// Identity of a network's native asset.
    #[must_use]
    pub fn native(network: NetworkId) -> Self {
        Self {
            network,
            kind: AssetKind::Native,
        }
    }

    /// Network the asset lives on.
    #[must_use]
    pub fn network(&self) -> NetworkId {
        self.network
    }

    /// Native or token discriminator.
    #[must_use]
    pub fn kind(&self) -> &AssetKind {
        &self.kind
    }
}

/// Asset identity plus display metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    id: AssetId,
    symbol: Symbol,
    name: Label,
    decimals: u8,
}

impl Asset {
    /// Builds the native asset of a network from its static metadata.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the network metadata is malformed. This is
    /// a programming error in [`crate::network`], not user input, and is
    /// reported rather than papered over.
    pub fn native(network: NetworkId) -> Result<Self, DomainError> {
        let info = network.info();
        let symbol = Symbol::parse(info.native_symbol)?;
        let name = Label::parse(info.display_name)?;
        Ok(Self {
            id: AssetId::native(network),
            symbol,
            name,
            decimals: info.native_decimals,
        })
    }

    /// Builds a token asset.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::TokenStandardNotSupported`] when the standard
    /// cannot exist on the network, [`DomainError::AddressFormat`] when the
    /// contract address is invalid for the network, and label/symbol validation
    /// errors for the metadata.
    pub fn token(
        network: NetworkId,
        standard: TokenStandard,
        contract: &str,
        symbol: &str,
        name: &str,
        decimals: u8,
    ) -> Result<Self, DomainError> {
        if !standard.is_supported_on(network) {
            return Err(DomainError::TokenStandardNotSupported { standard, network });
        }
        if decimals > MAX_DECIMALS {
            return Err(DomainError::DecimalsOutOfRange {
                max: MAX_DECIMALS,
                actual: decimals,
            });
        }
        let contract = Address::parse(network, contract)?;
        Ok(Self {
            id: AssetId {
                network,
                kind: AssetKind::Token { standard, contract },
            },
            symbol: Symbol::parse(symbol)?,
            name: Label::parse(name)?,
            decimals,
        })
    }

    /// Asset identity.
    #[must_use]
    pub fn id(&self) -> &AssetId {
        &self.id
    }

    /// Network the asset lives on.
    #[must_use]
    pub fn network(&self) -> NetworkId {
        self.id.network
    }

    /// Whether this is the network's native coin.
    #[must_use]
    pub fn is_native(&self) -> bool {
        matches!(self.id.kind, AssetKind::Native)
    }

    /// Token standard, when the asset is a token.
    #[must_use]
    pub fn token_standard(&self) -> Option<TokenStandard> {
        match &self.id.kind {
            AssetKind::Native => None,
            AssetKind::Token { standard, .. } => Some(*standard),
        }
    }

    /// Ticker symbol.
    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    /// Display name.
    #[must_use]
    pub fn name(&self) -> &Label {
        &self.name
    }

    /// Decimal precision of the asset's atomic unit.
    #[must_use]
    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Validated zero balance in this asset's precision.
    ///
    /// A zero balance is a real value; callers must not present it as an
    /// unknown balance.
    ///
    /// # Errors
    ///
    /// Returns a validation error only if the asset's decimals are unsupported,
    /// which cannot happen for assets built through this API.
    pub fn zero_balance(&self) -> Result<crate::amount::Amount, DomainError> {
        crate::amount::Amount::zero(self.decimals)
    }
}

impl<'de> Deserialize<'de> for Asset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawAsset {
            id: AssetId,
            symbol: Symbol,
            name: Label,
            decimals: u8,
        }

        let raw = RawAsset::deserialize(deserializer)?;
        if raw.decimals > MAX_DECIMALS {
            return Err(serde::de::Error::custom(DomainError::DecimalsOutOfRange {
                max: MAX_DECIMALS,
                actual: raw.decimals,
            }));
        }
        if let AssetKind::Token { standard, .. } = &raw.id.kind {
            if !standard.is_supported_on(raw.id.network) {
                return Err(serde::de::Error::custom(
                    DomainError::TokenStandardNotSupported {
                        standard: *standard,
                        network: raw.id.network,
                    },
                ));
            }
        }
        Ok(Self {
            id: raw.id,
            symbol: raw.symbol,
            name: raw.name,
            decimals: raw.decimals,
        })
    }
}

impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawAssetId {
            network: NetworkId,
            kind: AssetKind,
        }

        let raw = RawAssetId::deserialize(deserializer)?;
        // Address parsing validates the contract's own network, so the contract
        // must additionally be checked against the asset's declared network.
        if let AssetKind::Token { contract, standard } = &raw.kind {
            if contract.network() != raw.network {
                return Err(serde::de::Error::custom(DomainError::NetworkMismatch {
                    expected: raw.network,
                    actual: contract.network(),
                }));
            }
            if !standard.is_supported_on(raw.network) {
                return Err(serde::de::Error::custom(
                    DomainError::TokenStandardNotSupported {
                        standard: *standard,
                        network: raw.network,
                    },
                ));
            }
        }
        Ok(Self {
            network: raw.network,
            kind: raw.kind,
        })
    }
}

impl<'de> Deserialize<'de> for AssetKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "camelCase")]
        enum RawKind {
            Native,
            Token {
                standard: TokenStandard,
                contract: Address,
            },
        }

        Ok(match RawKind::deserialize(deserializer)? {
            RawKind::Native => Self::Native,
            RawKind::Token { standard, contract } => Self::Token { standard, contract },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_assets_follow_network_metadata() {
        let bitcoin = Asset::native(NetworkId::Bitcoin).expect("native asset");
        assert!(bitcoin.is_native());
        assert_eq!(bitcoin.symbol().as_str(), "BTC");
        assert_eq!(bitcoin.decimals(), 8);
        assert_eq!(bitcoin.name().as_str(), "Bitcoin");
        assert_eq!(bitcoin.token_standard(), None);
    }

    #[test]
    fn every_network_has_a_constructible_native_asset() {
        for network in NetworkId::ALL {
            let asset = Asset::native(network).expect("native asset metadata is valid");
            assert_eq!(asset.network(), network);
        }
    }

    #[test]
    fn token_identity_includes_network_and_contract() {
        let contract = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
        let usdt = Asset::token(
            NetworkId::Ethereum,
            TokenStandard::Erc20,
            contract,
            "USDT",
            "Tether USD",
            6,
        )
        .expect("token asset");
        assert_eq!(usdt.symbol().as_str(), "USDT");
        assert_eq!(usdt.decimals(), 6);
        assert_eq!(usdt.token_standard(), Some(TokenStandard::Erc20));
        assert!(!usdt.is_native());
        assert_ne!(
            usdt.id(),
            Asset::native(NetworkId::Ethereum)
                .expect("native asset")
                .id()
        );
    }

    #[test]
    fn erc20_is_rejected_on_non_evm_networks() {
        let contract = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
        assert_eq!(
            Asset::token(
                NetworkId::Bitcoin,
                TokenStandard::Erc20,
                contract,
                "USDT",
                "Tether",
                6
            ),
            Err(DomainError::TokenStandardNotSupported {
                standard: TokenStandard::Erc20,
                network: NetworkId::Bitcoin
            })
        );
    }

    #[test]
    fn jetton_contract_must_be_a_ton_address() {
        let ethereum_contract = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
        assert!(Asset::token(
            NetworkId::Ton,
            TokenStandard::TonJetton,
            ethereum_contract,
            "JET",
            "Jetton",
            9
        )
        .is_err());
    }

    #[test]
    fn token_metadata_is_validated() {
        let contract = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
        assert_eq!(
            Asset::token(
                NetworkId::Ethereum,
                TokenStandard::Erc20,
                contract,
                "usdt",
                "Tether",
                6
            ),
            Err(DomainError::InvalidSymbol)
        );
        assert!(Asset::token(
            NetworkId::Ethereum,
            TokenStandard::Erc20,
            contract,
            "USDT",
            "",
            6
        )
        .is_err());
        assert!(Asset::token(
            NetworkId::Ethereum,
            TokenStandard::Erc20,
            contract,
            "USDT",
            "Tether",
            19
        )
        .is_err());
    }

    #[test]
    fn asset_round_trips_through_json() {
        let asset = Asset::native(NetworkId::Ton).expect("native asset");
        let json = serde_json::to_string(&asset).expect("serialize asset");
        let parsed: Asset = serde_json::from_str(&json).expect("deserialize asset");
        assert_eq!(parsed, asset);
    }
}
