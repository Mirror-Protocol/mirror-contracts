use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Binary;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub admin_claim_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateOwner {
        owner: String,
    },
    ExecuteMigrations {
        migrations: Vec<(String, u64, Binary)>,
    },
    AuthorizeClaim {
        authorized_addr: String,
    },
    ClaimAdmin {
        contract: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    MigrationRecords {
        start_after: Option<u64>, // timestamp (seconds)
        limit: Option<u32>,
    },
    AuthRecords {
        start_after: Option<u64>, // timestamp (seconds)
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub admin_claim_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AuthRecordResponse {
    pub address: String,
    pub start_time: u64,
    pub end_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AuthRecordsResponse {
    pub records: Vec<AuthRecordResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrationRecordResponse {
    pub executor: String,
    pub time: u64,
    pub migrations: Vec<MigrationItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrationItem {
    pub contract: String,
    pub new_code_id: u64,
    pub msg: Binary,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrationRecordsResponse {
    pub records: Vec<MigrationRecordResponse>,
}
