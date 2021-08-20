use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Uint128;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub mint_contract: String,
    pub base_denom: String,
    pub lockup_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        owner: Option<String>,
        mint_contract: Option<String>,
        base_denom: Option<String>,
        lockup_period: Option<u64>,
    },
    LockPositionFundsHook {
        position_idx: Uint128,
        receiver: String,
    },
    UnlockPositionFunds {
        positions_idx: Vec<Uint128>,
    },
    ReleasePositionFunds {
        position_idx: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PositionLockInfo { position_idx: Uint128 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub mint_contract: String,
    pub base_denom: String,
    pub lockup_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionLockInfoResponse {
    pub idx: Uint128,
    pub receiver: String,
    pub locked_amount: Uint128,
    pub unlock_time: u64,
}
