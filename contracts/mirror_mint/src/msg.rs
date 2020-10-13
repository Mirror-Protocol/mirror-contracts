use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, HumanAddr, Uint128};
use cw20::Cw20ReceiveMsg;
use terraswap::{Asset, AssetInfo};

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
        asset_token: HumanAddr,
        auction_discount: Option<Decimal>,
        min_collateral_ratio: Option<Decimal>,
    },
    /// Generate asset token initialize msg and register required infos except token address
    RegisterAsset {
        asset_token: HumanAddr,
        auction_discount: Decimal,
        min_collateral_ratio: Decimal,
    },
    RegisterMigration {
        from_token: HumanAddr,
        to_token: HumanAddr,
        conversion_rate: Decimal,
    },
    // Create position to meet collateral ratio
    OpenPosition {
        collateral: Asset,
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
    },
    /// Deposit more collateral
    Deposit {
        position_idx: Uint128,
        collateral: Asset,
    },
    /// Withdraw collateral
    Withdraw {
        position_idx: Uint128,
        collateral: Asset,
    },
    /// Convert all deposit collateral to asset
    Mint {
        position_idx: Uint128,
        asset: Asset,
    },
    /// Migrate position asset to new asset
    /// Try migrate asset and collateral both
    MigratePosition {
        position_idx: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    // Create position to meet collateral ratio
    OpenPosition {
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
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
        owner_addr: HumanAddr,
        start_after: Option<Uint128>,
        limit: Option<u32>,
    },
    Migration {
        asset_token: HumanAddr,
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
pub struct MigrationResponse {
    pub from_token: HumanAddr,
    pub to_token: HumanAddr,
    pub conversion_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    pub idx: Uint128,
    pub owner: HumanAddr,
    pub collateral: Asset,
    pub asset: Asset,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct PositionsResponse {
    pub positions: Vec<PositionResponse>,
}
