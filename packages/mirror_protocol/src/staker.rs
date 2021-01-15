use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub mint_contract: HumanAddr,
    pub oracle_contract: HumanAddr,
    pub staking_contract: HumanAddr,
    pub terraswap_factory: HumanAddr,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// Execute following messages
    /// 1. swap half tokens
    /// 2. provide liquidity
    /// 3. stake lp token
    ExecuteBuyOperations {
        asset_token: HumanAddr,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
    },
    /// 1. mint tokens
    /// 2. provide liquidity
    /// 3. stake lp token
    ExecuteMintOperations {
        asset_token: HumanAddr,
        collateral_ratio: Decimal,
    },

    ProvideOperation {
        asset_token: HumanAddr,
        pair_contract: HumanAddr,
    },
    StakeOperation {
        asset_token: HumanAddr,
        liquidity_token: HumanAddr,
        staker: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub mint_contract: HumanAddr,
    pub oracle_contract: HumanAddr,
    pub staking_contract: HumanAddr,
    pub terraswap_factory: HumanAddr,
    pub base_denom: String,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
