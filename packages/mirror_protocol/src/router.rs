use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr};
use cw20::Cw20ReceiveMsg;
use terraswap::asset::AssetInfo;

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
    Receive(Cw20ReceiveMsg),
    /// Execute following messages
    /// 1. swap half tokens
    /// 2. provide liquidity
    /// 3. stake lp token
    BuyAndStake {
        asset_token: HumanAddr,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
    },
    /// 1. mint tokens
    /// 2. provide liquidity
    /// 3. stake lp token
    MintAndStake {
        asset_token: HumanAddr,
        collateral_ratio: Decimal,
    },
    /// Execute multiple BuyOperation
    BuyWithRoutes {
        offer_asset_info: AssetInfo,
        routes: Vec<AssetInfo>,
        max_spread: Option<Decimal>,
    },

    BuyOperation {
        offer_asset_info: AssetInfo,
        ask_asset_info: AssetInfo,
        max_spread: Option<Decimal>,
        to: Option<HumanAddr>,
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
pub enum Cw20HookMsg {
    BuyWithRoutes {
        routes: Vec<AssetInfo>,
        max_spread: Option<Decimal>,
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
