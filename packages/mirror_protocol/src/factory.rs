use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, Decimal, HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub token_code_id: u64,
    pub base_denom: String,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>, // [[start_time, end_time, distribution_amount], [], ...]
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    ///////////////////
    /// Owner Operations
    ///////////////////
    PostInitialize {
        owner: HumanAddr,
        terraswap_factory: HumanAddr,
        mirror_token: HumanAddr,
        staking_contract: HumanAddr,
        oracle_contract: HumanAddr,
        mint_contract: HumanAddr,
        commission_collector: HumanAddr,
    },
    UpdateConfig {
        owner: Option<HumanAddr>,
        token_code_id: Option<u64>,
        distribution_schedule: Option<Vec<(u64, u64, Uint128)>>, // [[start_time, end_time, distribution_amount], [], ...]
    },
    UpdateWeight {
        asset_token: HumanAddr,
        weight: u32,
    },
    Whitelist {
        /// asset name used to create token contract
        name: String,
        /// asset symbol used to create token contract
        symbol: String,
        /// authorized asset oracle feeder
        oracle_feeder: HumanAddr,
        /// used to create all necessary contract or register asset
        params: Params,
    },
    /// Internal use
    TokenCreationHook {
        oracle_feeder: HumanAddr,
    },
    /// Internal use except MIR registration
    TerraswapCreationHook {
        asset_token: HumanAddr,
    },
    PassCommand {
        contract_addr: HumanAddr,
        msg: Binary,
    },

    //////////////////////
    /// Feeder Operations
    /// //////////////////

    /// Revoke asset from MIR rewards pool
    /// and register end_price to mint contract
    RevokeAsset {
        asset_token: HumanAddr,
        end_price: Decimal,
    },
    /// Migrate asset to new asset by registering
    /// end_price to mint contract and add
    /// the new asset to MIR rewards pool
    MigrateAsset {
        name: String,
        symbol: String,
        from_token: HumanAddr,
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
    pub owner: HumanAddr,
    pub mirror_token: HumanAddr,
    pub mint_contract: HumanAddr,
    pub staking_contract: HumanAddr,
    pub commission_collector: HumanAddr,
    pub oracle_contract: HumanAddr,
    pub terraswap_factory: HumanAddr,
    pub token_code_id: u64,
    pub base_denom: String,
    pub genesis_time: u64,
    pub distribution_schedule: Vec<(u64, u64, Uint128)>,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionInfoResponse {
    pub weights: Vec<(HumanAddr, u32)>,
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
    /// For pre-IPO assets, collateral ratio for the asset after migration
    pub min_collateral_ratio_after_migration: Option<Decimal>,
}
