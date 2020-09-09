use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub mint_per_block: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    PostInitlize {
        mirror_token: HumanAddr,
    },
    UpdateConfig {
        owner: Option<HumanAddr>,
        mint_per_block: Option<Uint128>,
    },
    UpdateWeight {
        symbol: String,
        weight: Decimal,
    },
    Whitelist {
        symbol: String,
        weight: Decimal,
        token_contract: HumanAddr,
        mint_contract: HumanAddr,
        market_contract: HumanAddr,
        oracle_contract: HumanAddr,
        staking_contract: HumanAddr,
    },
    Mint {
        symbol: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    WhitelistInfo { symbol: String },
    DistributionInfo { symbol: String },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub mirror_token: HumanAddr,
    pub mint_per_block: Uint128,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WhitelistInfoResponse {
    pub token_contract: HumanAddr,
    pub mint_contract: HumanAddr,
    pub market_contract: HumanAddr,
    pub oracle_contract: HumanAddr,
    pub staking_contract: HumanAddr,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionInfoResponse {
    pub weight: Decimal,
    pub last_height: u64,
}

////////////////////////
/// Staking contract hook
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingCw20HookMsg {
    DepositReward {},
}
