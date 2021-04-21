use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr, Binary, WasmQuery};
use terraswap::asset::AssetInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    UpdateConfig {
        owner: Option<HumanAddr>,
        base_denom: Option<String>,
    },
    RegisterCollateralAsset {
        asset: AssetInfo,
        query_request: Binary,
        collateral_premium: Decimal,
    },
    UpdateCollateralAsset {
        asset: AssetInfo,
        query_request: Option<Binary>,    
        collateral_premium: Option<Decimal>,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    CollateralPrice {
        asset: String,
    },
    CollateralAssetInfo {
        asset: String,
    },
    CollateralAssetInfos {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollateralPriceResponse {
    pub asset: String,
    pub rate: Decimal,
    pub collateral_premium: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollateralInfoResponse {
    pub asset: String,
    pub collateral_premium: Decimal,
    pub query_request: WasmQuery,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollateralInfosResponse {
    pub collaterals: Vec<CollateralInfoResponse>,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
