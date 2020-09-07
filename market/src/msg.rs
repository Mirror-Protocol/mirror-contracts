use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Coin, Decimal, HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// Liquidity token, required to withdraw liquidity position
    pub liquidity_token: HumanAddr,
    /// Inactive commission collector
    pub commission_collector: HumanAddr,
    /// Asset token address
    pub asset_token: HumanAddr,
    /// Asset oracle address
    pub asset_oracle: HumanAddr,
    /// Asset symbol
    pub asset_symbol: String,
    /// Collateral denom
    pub collateral_denom: String,
    /// Commission rate for active liquidity provider
    pub active_commission: Decimal,
    /// Commission rate for mirror token stakers
    pub inactive_commission: Decimal,
    /// Maximum spread to protect trader
    pub max_minus_spread: Decimal,
    /// Maximum minus spread to protect arbitrage attack
    pub max_spread: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    UpdateConfig {
        /// Later it will be set by gov contract
        owner: Option<HumanAddr>,
        active_commission: Option<Decimal>,
        inactive_commission: Option<Decimal>,
        max_minus_spread: Option<Decimal>,
        max_spread: Option<Decimal>,
    },
    /// ProvideLiquidity a user provides pool liquidity
    ProvideLiquidity { coins: Vec<Coin> },
    /// WithdrawLiquidity a liquidity provider can withdraw the asset
    WithdrawLiquidity { amount: Uint128 },
    /// Buy an asset
    Buy { max_spread: Option<Decimal> },
    /// Sell a given amount of asset
    Sell {
        amount: Uint128,
        max_spread: Option<Decimal>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ConfigGeneral {},
    ConfigAsset {},
    ConfigSwap {},
    Pool {},
    Provider {
        address: HumanAddr,
    },
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
    pub max_minus_spread: Decimal,
    pub max_spread: Decimal,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigAssetResponse {
    pub oracle: HumanAddr,
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

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ProviderResponse {
    pub share: Uint128,
}

/// SimulationResponse returns swap simulation response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SimulationResponse {
    pub return_amount: Coin,
    pub spread_amount: Coin,
    pub minus_spread_amount: Coin,
    pub commission_amount: Coin,
}

/// ReverseSimulationResponse returns reverse swap simulation response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ReverseSimulationResponse {
    pub offer_amount: Coin,
    pub spread_amount: Coin,
    pub minus_spread_amount: Coin,
    pub commission_amount: Coin,
}
