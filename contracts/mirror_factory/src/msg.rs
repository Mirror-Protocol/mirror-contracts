use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::Params;
use cosmwasm_std::{Binary, Decimal, HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub mint_per_block: Uint128,
    pub token_code_id: u64,
    pub base_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
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
        mint_per_block: Option<Uint128>,
        token_code_id: Option<u64>,
    },
    UpdateWeight {
        asset_token: HumanAddr,
        weight: Decimal,
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
    TokenCreationHook {
        oracle_feeder: HumanAddr,
    },
    TerraswapCreationHook {
        asset_token: HumanAddr,
    },
    PassCommand {
        contract_addr: HumanAddr,
        msg: Binary,
    },
    Mint {
        asset_token: HumanAddr,
    },
    MigrateAsset {
        name: String,
        symbol: String,
        from_token: HumanAddr,
        end_price: Decimal,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    DistributionInfo { asset_token: HumanAddr },
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
    pub mint_per_block: Uint128,
    pub token_code_id: u64,
    pub base_denom: String,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DistributionInfoResponse {
    pub weight: Decimal,
    pub last_height: u64,
}

////////////////////////
/// Staking contract hook
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingCw20HookMsg {
    DepositReward { asset_token: HumanAddr },
}
