use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub deposit_target: HumanAddr,
    pub staking_symbol: String,
    pub collateral_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Convert { symbol: String },
    Send {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub deposit_target: HumanAddr,
    pub staking_symbol: String,
    pub collateral_denom: String,
}

////////////////////////
/// Staking contract hook
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingCw20HookMsg {
    Bond {},
    DepositReward {},
}

//////////////////////////////
/// Market contract handle msg
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketHandleMsg {
    /// Buy an asset
    Buy { max_spread: Option<Decimal> },
    /// Sell a given amount of asset
    Sell {
        amount: Uint128,
        max_spread: Option<Decimal>,
    },
}
