use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    pub mint_contract: HumanAddr,
    pub base_denom: String,
    pub lockup_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    UpdateConfig {
        owner: Option<HumanAddr>,
        mint_contract: Option<HumanAddr>,
        base_denom: Option<String>,
        lockup_period: Option<u64>,
    },
    LockPositionFundsHook {
        position_idx: Uint128,
        receiver: HumanAddr,
    },
    UnlockPositionFunds {
        positions_idx: Vec<Uint128>,
    },
    ReleasePositionFunds {
        position_idx: Uint128,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PositionLockInfo {
        position_idx: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub mint_contract: HumanAddr,
    pub base_denom: String,
    pub lockup_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionLockInfoResponse {
    pub idx: Uint128,
    pub receiver: HumanAddr,
    pub locked_amount: Uint128,
    pub unlock_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}