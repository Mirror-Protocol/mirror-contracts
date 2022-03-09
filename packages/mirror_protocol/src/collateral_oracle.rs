use cosmwasm_std::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use terraswap::asset::AssetInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub mint_contract: String,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        owner: Option<String>,
        mint_contract: Option<String>,
        base_denom: Option<String>,
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
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    CollateralPrice {
        asset: String,
        timeframe: Option<u64>,
    },
    CollateralAssetInfo {
        asset: String,
    },
    CollateralAssetInfos {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub mint_contract: String,
    pub base_denom: String,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {
    pub mirror_tefi_oracle_addr: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    TefiOracle {
        oracle_addr: String,
    },
    FixedPrice {
        price: Decimal,
    },
    AmmPair {
        pair_addr: String,
        intermediate_denom: Option<String>,
    },
    AnchorMarket {
        anchor_market_addr: String,
    },
    Native {
        native_denom: String,
    },
    Lunax {
        staking_contract_addr: String,
    },
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SourceType::TefiOracle { .. } => write!(f, "tefi_oracle"),
            SourceType::FixedPrice { .. } => write!(f, "fixed_price"),
            SourceType::AmmPair { .. } => write!(f, "amm_pair"),
            SourceType::AnchorMarket { .. } => write!(f, "anchor_market"),
            SourceType::Native { .. } => write!(f, "native"),
            SourceType::Lunax { .. } => write!(f, "lunax"),
        }
    }
}
