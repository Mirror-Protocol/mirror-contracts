use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr};
use terraswap::asset::AssetInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    pub mint_contract: HumanAddr,
    pub base_denom: String,
    pub mirror_oracle: HumanAddr,
    pub anchor_oracle: HumanAddr,
    pub band_oracle: HumanAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    UpdateConfig {
        owner: Option<HumanAddr>,
        mint_contract: Option<HumanAddr>,
        base_denom: Option<String>,
        mirror_oracle: Option<HumanAddr>,
        anchor_oracle: Option<HumanAddr>,
        band_oracle: Option<HumanAddr>,
    },
    RegisterCollateralAsset {
        asset: AssetInfo,
        price_source: SourceType,
        multiplier: Decimal,
    },
    RevokeCollateralAsset {
        asset: AssetInfo,
    },
    UpdateCollateralPriceSource {
        asset: AssetInfo,
        price_source: SourceType,
    },
    UpdateCollateralMultiplier {
        asset: AssetInfo,
        multiplier: Decimal,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    CollateralPrice {
        asset: String,
        block_height: Option<u64>, // needed for aUST rate
    },
    CollateralAssetInfo {
        asset: String,
    },
    CollateralAssetInfos {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub mint_contract: HumanAddr,
    pub base_denom: String,
    pub mirror_oracle: HumanAddr,
    pub anchor_oracle: HumanAddr,
    pub band_oracle: HumanAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollateralPriceResponse {
    pub asset: String,
    pub rate: Decimal,
    pub last_updated: u64,
    pub multiplier: Decimal,
    pub is_revoked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollateralInfoResponse {
    pub asset: String,
    pub multiplier: Decimal,
    pub source_type: String,
    pub is_revoked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollateralInfosResponse {
    pub collaterals: Vec<CollateralInfoResponse>,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    MirrorOracle {},
    AnchorOracle {},
    BandOracle {},
    FixedPrice {
        price: Decimal,
    },
    Terraswap {
        terraswap_pair_addr: HumanAddr,
        intermediate_denom: Option<String>,
    },
    AnchorMarket {
        anchor_market_addr: HumanAddr,
    },
    Native {
        native_denom: String,
    },
} 

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SourceType::MirrorOracle{..} => write!(f, "mirror_oracle"),
            SourceType::AnchorOracle{..} => write!(f, "anchor_oracle"),
            SourceType::BandOracle{..} => write!(f, "band_oracle"),
            SourceType::FixedPrice{..} => write!(f, "fixed_price"),
            SourceType::Terraswap{..} => write!(f, "terraswap"),
            SourceType::AnchorMarket{..} => write!(f, "anchor_market"),
            SourceType::Native {..} => write!(f, "native"),
        }
    }
}
