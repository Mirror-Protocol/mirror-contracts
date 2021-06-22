use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::HumanAddr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    pub distribution_contract: HumanAddr, // collected rewards receiver
    pub terraswap_factory: HumanAddr,
    pub mirror_token: HumanAddr,
    pub base_denom: String,
    // aUST params
    pub aust_token: HumanAddr,
    pub anchor_market: HumanAddr,
    // bLuna params
    pub bluna_token: HumanAddr,
    pub bluna_swap_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    UpdateConfig {
        owner: Option<HumanAddr>,
        distribution_contract: Option<HumanAddr>,
        terraswap_factory: Option<HumanAddr>,
        mirror_token: Option<HumanAddr>,
        base_denom: Option<String>,
        aust_token: Option<HumanAddr>,
        anchor_market: Option<HumanAddr>,
        bluna_token: Option<HumanAddr>,
        bluna_swap_denom: Option<String>,  
    },
    Convert { asset_token: HumanAddr },
    Distribute {},
    LunaSwapHook {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub distribution_contract: HumanAddr, // collected rewards receiver
    pub terraswap_factory: HumanAddr,
    pub mirror_token: HumanAddr,
    pub base_denom: String,
    pub aust_token: HumanAddr,
    pub anchor_market: HumanAddr,
    pub bluna_token: HumanAddr,
    pub bluna_swap_denom: String,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {
    pub owner: HumanAddr,
    pub aust_token: HumanAddr,
    pub anchor_market: HumanAddr,
    pub bluna_token: HumanAddr,
    pub bluna_swap_denom: String,
}
