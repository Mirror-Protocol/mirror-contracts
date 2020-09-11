use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Coin, Decimal, HumanAddr, Uint128};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// Inactive commission collector
    pub commission_collector: HumanAddr,
    /// Asset token address
    pub asset_token: HumanAddr,
    /// Asset symbol
    pub asset_symbol: String,
    /// Collateral denom
    pub collateral_denom: String,
    /// Commission rate for active liquidity provider
    pub active_commission: Decimal,
    /// Commission rate for mirror token stakers
    pub inactive_commission: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    /// Post initize step to allow user to set controlled contract address after creating it
    PostInitialize {
        /// Liquidity token, required to withdraw liquidity position
        liquidity_token: HumanAddr,
    },
    UpdateConfig {
        /// Later it will be set by gov contract
        owner: Option<HumanAddr>,
        active_commission: Option<Decimal>,
        inactive_commission: Option<Decimal>,
    },
    /// ProvideLiquidity a user provides pool liquidity
    ProvideLiquidity {
        coins: Vec<Coin>,
    },
    WithdrawLiquidity {
        amount: Uint128,
    },
    /// Buy an asset
    Buy {
        max_spread: Option<Decimal>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Sell a given amount of asset
    Sell { max_spread: Option<Decimal> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ConfigGeneral {},
    ConfigAsset {},
    ConfigSwap {},
    Pool {},
    Simulation {
        offer_amount: Uint128,
        operation: SwapOperation,
    },
    ReverseSimulation {
        ask_amount: Uint128,
        operation: SwapOperation,
    },
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwapOperation {
    /// Buy operation
    Buy,
    /// Sell operation
    Sell,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigGeneralResponse {
    pub owner: HumanAddr,
    pub liquidity_token: HumanAddr,
    pub commission_collector: HumanAddr,
    pub collateral_denom: String,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigSwapResponse {
    pub active_commission: Decimal,
    pub inactive_commission: Decimal,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigAssetResponse {
    pub token: HumanAddr,
    pub symbol: String,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolResponse {
    pub asset_pool: Uint128,
    pub collateral_pool: Uint128,
    pub total_share: Uint128,
}

/// SimulationResponse returns swap simulation response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SimulationResponse {
    pub return_amount: Coin,
    pub spread_amount: Coin,
    pub commission_amount: Coin,
}

/// ReverseSimulationResponse returns reverse swap simulation response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ReverseSimulationResponse {
    pub offer_amount: Coin,
    pub spread_amount: Coin,
    pub commission_amount: Coin,
}
