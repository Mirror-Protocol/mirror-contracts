use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, Decimal, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub token_code_id: u64,
    pub base_denom: String,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>, // [[start_time, end_time, distribution_amount], [], ...]
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ///////////////////
    /// Owner Operations
    ///////////////////
    PostInitialize {
        owner: String,
        terraswap_factory: String,
        mirror_token: String,
        staking_contract: String,
        oracle_contract: String,
        mint_contract: String,
        commission_collector: String,
    },
    UpdateConfig {
        owner: Option<String>,
        token_code_id: Option<u64>,
        distribution_schedule: Option<Vec<(u64, u64, Uint128)>>, // [[start_time, end_time, distribution_amount], [], ...]
    },
    UpdateWeight {
        asset_token: String,
        weight: u32,
    },
    Whitelist {
        /// asset name used to create token contract
        name: String,
        /// asset symbol used to create token contract
        symbol: String,
        /// authorized asset oracle feeder
        oracle_feeder: String,
        /// used to create all necessary contract or register asset
        params: Params,
    },
    PassCommand {
        contract_addr: String,
        msg: Binary,
    },

    //////////////////////
    /// Feeder Operations
    /// //////////////////

    /// Revoke asset from MIR rewards pool
    /// and register end_price to mint contract
    /// Only feeder can set end_price
    RevokeAsset {
        asset_token: String,
        end_price: Decimal,
    },
    /// Migrate asset to new asset by registering
    /// end_price to mint contract and add
    /// the new asset to MIR rewards pool
    MigrateAsset {
        name: String,
        symbol: String,
        from_token: String,
        end_price: Decimal,
    },

    ///////////////////
    /// User Operations
    ///////////////////
    Distribute {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    DistributionInfo {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub mirror_token: String,
    pub mint_contract: String,
    pub staking_contract: String,
    pub commission_collector: String,
    pub oracle_contract: String,
    pub terraswap_factory: String,
    pub token_code_id: u64,
    pub base_denom: String,
    pub genesis_time: u64,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionInfoResponse {
    pub weights: Vec<(String, u32)>,
    pub last_distributed: u64,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Params {
    /// Auction discount rate applied to asset mint
    pub auction_discount: Decimal,
    /// Minium collateral ratio applied to asset mint
    pub min_collateral_ratio: Decimal,
    /// Distribution weight (default is 30, which is 1/10 of MIR distribution weight)
    pub weight: Option<u32>,
    /// For pre-IPO assets, time period after asset creation in which minting is enabled
    pub mint_period: Option<u64>,
    /// For pre-IPO assets, collateral ratio for the asset after ipo
    pub min_collateral_ratio_after_ipo: Option<Decimal>,
    /// For pre-IPO assets, fixed price during minting period
    pub pre_ipo_price: Option<Decimal>,
}
