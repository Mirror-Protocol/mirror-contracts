use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr, Uint128};
use cw20::Cw20ReceiveMsg;
use terraswap::asset::{Asset, AssetInfo};

use crate::common::OrderBy;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    pub oracle: HumanAddr,
    pub collector: HumanAddr,
    pub collateral_oracle: HumanAddr,
    pub staking: HumanAddr,
    pub terraswap_factory: HumanAddr,
    pub lock: HumanAddr,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),

    //////////////////////
    /// Owner Operations
    //////////////////////

    /// Update config; only owner is allowed to execute it
    UpdateConfig {
        owner: Option<HumanAddr>,
        oracle: Option<HumanAddr>,
        collector: Option<HumanAddr>,
        collateral_oracle: Option<HumanAddr>,
        terraswap_factory: Option<HumanAddr>,
        lock: Option<HumanAddr>,
        token_code_id: Option<u64>,
        protocol_fee_rate: Option<Decimal>,
    },
    /// Update asset related parameters
    UpdateAsset {
        asset_token: HumanAddr,
        auction_discount: Option<Decimal>,
        min_collateral_ratio: Option<Decimal>,
        ipo_params: Option<IPOParams>,
    },
    /// Generate asset token initialize msg and register required infos except token address
    RegisterAsset {
        asset_token: HumanAddr,
        auction_discount: Decimal,
        min_collateral_ratio: Decimal,
        ipo_params: Option<IPOParams>,
    },
    RegisterMigration {
        asset_token: HumanAddr,
        end_price: Decimal,
    },
    /// Asset feeder is allowed to trigger IPO event on preIPO assets
    TriggerIPO {
        asset_token: HumanAddr,
    },

    //////////////////////
    /// User Operations
    //////////////////////
    // Create position to meet collateral ratio
    OpenPosition {
        collateral: Asset,
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
        short_params: Option<ShortParams>,
    },
    /// Deposit more collateral
    Deposit {
        position_idx: Uint128,
        collateral: Asset,
    },
    /// Withdraw collateral
    Withdraw {
        position_idx: Uint128,
        collateral: Option<Asset>,
    },
    /// Convert all deposit collateral to asset
    Mint {
        position_idx: Uint128,
        asset: Asset,
        short_params: Option<ShortParams>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ShortParams {
    pub belief_price: Option<Decimal>,
    pub max_spread: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct IPOParams {
    pub mint_end: u64,
    pub pre_ipo_price: Decimal,
    pub min_collateral_ratio_after_ipo: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // Create position to meet collateral ratio
    OpenPosition {
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
        short_params: Option<ShortParams>,
    },
    /// Deposit more collateral
    Deposit { position_idx: Uint128 },
    /// Convert specified asset amount and send back to user
    Burn { position_idx: Uint128 },
    /// Buy discounted collateral from the contract with their asset tokens
    Auction { position_idx: Uint128 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    AssetConfig {
        asset_token: HumanAddr,
    },
    Position {
        position_idx: Uint128,
    },
    Positions {
        owner_addr: Option<HumanAddr>,
        asset_token: Option<HumanAddr>,
        start_after: Option<Uint128>,
        limit: Option<u32>,
        order_by: Option<OrderBy>,
    },
    NextPositionIdx {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub oracle: HumanAddr,
    pub collector: HumanAddr,
    pub collateral_oracle: HumanAddr,
    pub staking: HumanAddr,
    pub terraswap_factory: HumanAddr,
    pub lock: HumanAddr,
    pub base_denom: String,
    pub token_code_id: u64,
    pub protocol_fee_rate: Decimal,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetConfigResponse {
    pub token: HumanAddr,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
    pub end_price: Option<Decimal>,
    pub ipo_params: Option<IPOParams>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    pub idx: Uint128,
    pub owner: HumanAddr,
    pub collateral: Asset,
    pub asset: Asset,
    pub is_short: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct PositionsResponse {
    pub positions: Vec<PositionResponse>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct NextPositionIdxResponse {
    pub next_position_idx: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
