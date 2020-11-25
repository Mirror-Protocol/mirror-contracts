use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr};
use terraswap::Asset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub distribution_contract: HumanAddr, // collected rewards receiver
    pub terraswap_factory: HumanAddr,
    pub mirror_token: HumanAddr,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Convert { asset_token: HumanAddr },
    Distribute {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub distribution_contract: HumanAddr, // collected rewards receiver
    pub terraswap_factory: HumanAddr,
    pub mirror_token: HumanAddr,
    pub base_denom: String,
}

////////////////////////
/// Staking contract hook
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingCw20HookMsg {
    DepositReward {},
}

//////////////////////////////
/// Terraswap contract handle msg
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerraswapHandleMsg {
    /// Swap an offer asset to the other
    Swap {
        offer_asset: Asset,
        max_spread: Option<Decimal>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerraswapCw20HookMsg {
    /// Sell a given amount of asset
    Swap { max_spread: Option<Decimal> },
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
