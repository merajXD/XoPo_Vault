//! Supported blockchain networks and their static metadata.
//!
//! A [`NetworkId`] is the stable identifier used across the core, storage and
//! the platform interfaces. Only networks listed here are recognised; adding a
//! network is an explicit, reviewed change to this module plus its chain
//! adapter.
//!
//! Metadata in this module is factual and independent of any RPC endpoint:
//! display names, native asset symbols, native decimals and SLIP-0044 coin
//! types. Chain adapters own everything that requires network access.

use std::fmt;

use serde::{Deserialize, Serialize};

/// How addresses and accounts are represented on a chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChainFamily {
    /// Bitcoin-style UTXO ledger.
    Utxo,
    /// EVM-compatible account ledger.
    Evm,
    /// TON account chain.
    Ton,
    /// Solana account chain.
    Solana,
    /// XRP Ledger account chain.
    XrpLedger,
}

/// Delivery priority of a network, mirroring the product roadmap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryTier {
    /// Part of the initial release scope.
    Initial,
    /// Planned for a later expansion.
    Planned,
}

/// Stable identifier of a supported network.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkId {
    /// Bitcoin mainnet.
    Bitcoin,
    /// Ethereum mainnet.
    Ethereum,
    /// TON mainnet.
    Ton,
    /// Litecoin mainnet.
    Litecoin,
    /// Solana mainnet.
    Solana,
    /// BNB Smart Chain mainnet.
    BnbChain,
    /// Polygon PoS mainnet.
    Polygon,
    /// Bitcoin Cash mainnet.
    BitcoinCash,
    /// XRP Ledger mainnet.
    Xrp,
    /// Dogecoin mainnet.
    Dogecoin,
}

/// Static, non-network metadata for a supported network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkInfo {
    /// Stable identifier.
    pub id: NetworkId,
    /// Human-readable name for interface text.
    pub display_name: &'static str,
    /// Symbol of the native asset.
    pub native_symbol: &'static str,
    /// Decimals of the native asset's atomic unit.
    pub native_decimals: u8,
    /// SLIP-0044 coin type used in BIP-44 derivation paths.
    pub slip44_coin_type: u32,
    /// Chain family, which determines address handling.
    pub family: ChainFamily,
    /// Roadmap delivery tier.
    pub tier: DeliveryTier,
}

impl NetworkId {
    /// Every supported network, in roadmap order.
    pub const ALL: [NetworkId; 10] = [
        NetworkId::Bitcoin,
        NetworkId::Ethereum,
        NetworkId::Ton,
        NetworkId::Litecoin,
        NetworkId::Solana,
        NetworkId::BnbChain,
        NetworkId::Polygon,
        NetworkId::BitcoinCash,
        NetworkId::Xrp,
        NetworkId::Dogecoin,
    ];

    /// Stable text identifier, also used in serialized data.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            NetworkId::Bitcoin => "bitcoin",
            NetworkId::Ethereum => "ethereum",
            NetworkId::Ton => "ton",
            NetworkId::Litecoin => "litecoin",
            NetworkId::Solana => "solana",
            NetworkId::BnbChain => "bnb-chain",
            NetworkId::Polygon => "polygon",
            NetworkId::BitcoinCash => "bitcoin-cash",
            NetworkId::Xrp => "xrp",
            NetworkId::Dogecoin => "dogecoin",
        }
    }

    /// Static metadata for this network.
    #[must_use]
    pub fn info(self) -> NetworkInfo {
        let (display_name, native_symbol, native_decimals, slip44_coin_type, family, tier) =
            match self {
                NetworkId::Bitcoin => (
                    "Bitcoin",
                    "BTC",
                    8,
                    0,
                    ChainFamily::Utxo,
                    DeliveryTier::Initial,
                ),
                NetworkId::Ethereum => (
                    "Ethereum",
                    "ETH",
                    18,
                    60,
                    ChainFamily::Evm,
                    DeliveryTier::Initial,
                ),
                NetworkId::Ton => (
                    "TON",
                    "TON",
                    9,
                    607,
                    ChainFamily::Ton,
                    DeliveryTier::Initial,
                ),
                NetworkId::Litecoin => (
                    "Litecoin",
                    "LTC",
                    8,
                    2,
                    ChainFamily::Utxo,
                    DeliveryTier::Initial,
                ),
                NetworkId::Solana => (
                    "Solana",
                    "SOL",
                    9,
                    501,
                    ChainFamily::Solana,
                    DeliveryTier::Planned,
                ),
                // BNB Smart Chain and Polygon are EVM chains and therefore share
                // SLIP-0044 coin type 60 with Ethereum. Accounts are separated by
                // the wallet's network selection, not by the coin type alone.
                NetworkId::BnbChain => (
                    "BNB Smart Chain",
                    "BNB",
                    18,
                    60,
                    ChainFamily::Evm,
                    DeliveryTier::Planned,
                ),
                NetworkId::Polygon => (
                    "Polygon",
                    "POL",
                    18,
                    60,
                    ChainFamily::Evm,
                    DeliveryTier::Planned,
                ),
                NetworkId::BitcoinCash => (
                    "Bitcoin Cash",
                    "BCH",
                    8,
                    145,
                    ChainFamily::Utxo,
                    DeliveryTier::Planned,
                ),
                NetworkId::Xrp => (
                    "XRP Ledger",
                    "XRP",
                    6,
                    144,
                    ChainFamily::XrpLedger,
                    DeliveryTier::Planned,
                ),
                NetworkId::Dogecoin => (
                    "Dogecoin",
                    "DOGE",
                    8,
                    3,
                    ChainFamily::Utxo,
                    DeliveryTier::Planned,
                ),
            };

        NetworkInfo {
            id: self,
            display_name,
            native_symbol,
            native_decimals,
            slip44_coin_type,
            family,
            tier,
        }
    }

    /// Human-readable network name.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        self.info().display_name
    }

    /// Symbol of the native asset.
    #[must_use]
    pub fn native_symbol(self) -> &'static str {
        self.info().native_symbol
    }

    /// Decimals of the native asset.
    #[must_use]
    pub fn native_decimals(self) -> u8 {
        self.info().native_decimals
    }

    /// SLIP-0044 coin type.
    #[must_use]
    pub fn slip44_coin_type(self) -> u32 {
        self.info().slip44_coin_type
    }

    /// Chain family.
    #[must_use]
    pub fn family(self) -> ChainFamily {
        self.info().family
    }

    /// Roadmap delivery tier.
    #[must_use]
    pub fn tier(self) -> DeliveryTier {
        self.info().tier
    }

    /// Resolves a stable text identifier back to a network.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|network| network.id() == id)
    }
}

impl fmt::Display for NetworkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_ids_are_unique_and_resolve_back() {
        for network in NetworkId::ALL {
            assert_eq!(NetworkId::from_id(network.id()), Some(network));
        }
        for (index, network) in NetworkId::ALL.iter().enumerate() {
            for other in &NetworkId::ALL[index + 1..] {
                assert_ne!(network.id(), other.id());
            }
        }
    }

    #[test]
    fn initial_tier_matches_the_product_scope() {
        let initial: Vec<NetworkId> = NetworkId::ALL
            .into_iter()
            .filter(|network| network.tier() == DeliveryTier::Initial)
            .collect();
        assert_eq!(
            initial,
            vec![
                NetworkId::Bitcoin,
                NetworkId::Ethereum,
                NetworkId::Ton,
                NetworkId::Litecoin
            ]
        );
    }

    #[test]
    fn native_decimals_are_within_the_amount_limit() {
        for network in NetworkId::ALL {
            assert!(network.native_decimals() <= crate::amount::MAX_DECIMALS);
        }
    }

    #[test]
    fn network_serializes_with_its_stable_id() {
        let json = serde_json::to_string(&NetworkId::BnbChain).expect("serialize network");
        assert_eq!(json, "\"bnb-chain\"");
        let parsed: NetworkId = serde_json::from_str("\"bitcoin-cash\"").expect("deserialize");
        assert_eq!(parsed, NetworkId::BitcoinCash);
    }

    #[test]
    fn unknown_network_ids_do_not_resolve() {
        assert_eq!(NetworkId::from_id("cardano"), None);
    }
}
