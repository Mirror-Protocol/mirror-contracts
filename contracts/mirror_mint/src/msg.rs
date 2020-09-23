use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr};
use cw20::Cw20ReceiveMsg;
use uniswap::{Asset, AssetInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    pub oracle: HumanAddr,
    pub base_asset_info: AssetInfo,
    pub token_code_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    /// Update config; only owner is allowed to execute it
    UpdateConfig {
        owner: Option<HumanAddr>,
        token_code_id: Option<u64>,
    },
    /// Update asset related parameters
    UpdateAsset {
        asset_info: AssetInfo,
        auction_discount: Option<Decimal>,
        min_collateral_ratio: Option<Decimal>,
    },
    /// Generate asset token initialize msg and register required infos except token address
    RegisterAsset {
        asset_token: HumanAddr,
        auction_discount: Decimal,
        min_collateral_ratio: Decimal,
    },
    // create position to meet collateral ratio
    OpenPosition {
        collateral: Asset,
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
    },
    /// deposit more collateral
    Deposit {
        position_idx: u64,
        collateral: Asset,
    },
    /// withdraw collateral
    Withdraw {
        position_idx: u64,
        collateral: Asset,
    },
    /// convert all deposit collateral to asset
    Mint {
        position_idx: u64,
        asset: Asset,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // create position to meet collateral ratio
    OpenPosition {
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
    },
    /// deposit more collateral
    Deposit { position_idx: u64 },
    /// convert specified asset amount and send back to user
    Burn { position_idx: u64 },
    /// Auction a user can sell their asset tokens in discounted prices
    Auction { position_idx: u64 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    AssetConfig {
        asset_info: AssetInfo,
    },
    Position {
        position_idx: u64,
    },
    Positions {
        owner_addr: HumanAddr,
        start_after: Option<u64>,
        limit: Option<u32>,
    },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub oracle: HumanAddr,
    pub base_asset_info: AssetInfo,
    pub token_code_id: u64,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetConfigResponse {
    pub token: HumanAddr,
    pub auction_discount: Decimal,
    pub min_collateral_ratio: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    pub owner: HumanAddr,
    pub collateral: Asset,
    pub asset: Asset,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct PositionsResponse {
    pub positions: Vec<PositionResponse>,
}
