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
        auction_threshold_ratio: Option<Decimal>,
        min_collateral_ratio: Option<Decimal>,
    },
    /// Generate asset token initialize msg and register required infos except token address
    RegisterAsset {
        asset_token_addr: HumanAddr,
        auction_discount: Decimal,
        auction_threshold_ratio: Decimal,
        min_collateral_ratio: Decimal,
    },
    /// Deposit collateral asset to mint an asset
    Deposit {
        collateral: Asset,
        asset_info: AssetInfo,
    },
    /// Withdarw collateral asset, when there is enough
    /// buffer to cover min_collateral_ratio
    Withdraw {
        collateral: Asset,
        asset_info: AssetInfo,
    },
    /// Mint a user sends the collateral token to mint an asset
    /// If the collateral cannot cover min_collateral_ratio,
    /// the operation must be failed
    Mint {
        asset: Asset,
        collateral_info: AssetInfo,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Deposit a user also can deposit the collateral to any position
    Deposit { asset_info: AssetInfo },
    /// Burn a user sends the asset tokens to the contract to get back the collteral tokens
    Burn { collateral_info: AssetInfo },
    /// Auction a user can sell their asset tokens in discounted prices
    Auction {
        collateral_info: AssetInfo,
        position_owner: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    AssetConfig {
        asset_info: AssetInfo,
    },
    Position {
        minter: HumanAddr,
        asset_info: AssetInfo,
        collateral_info: AssetInfo,
    },
    Positions {
        minter: HumanAddr,
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
    pub auction_threshold_ratio: Decimal,
    pub min_collateral_ratio: Decimal,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionsResponse {
    pub minter: HumanAddr,
    pub positions: Vec<PositionResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    pub collateral: Asset,
    pub asset: Asset,
    pub is_auction_open: bool,
}
