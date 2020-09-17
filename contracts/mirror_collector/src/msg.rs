use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub gov_contract: HumanAddr, // collected rewards receiver
    pub factory_contract: HumanAddr,
    pub mirror_token: HumanAddr,
    pub collateral_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Convert { asset_token: HumanAddr },
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
    pub gov_contract: HumanAddr, // collected rewards receiver
    pub factory_contract: HumanAddr,
    pub mirror_token: HumanAddr,
    pub collateral_denom: String,
}

////////////////////////
/// Staking contract hook
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingCw20HookMsg {
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
