use cosmwasm_std::{Api, Binary, CanonicalAddr, Order, StdResult, Storage};
use cw_storage_plus::{Bound, Item, Map, U64Key};
use mirror_protocol::admin_manager::{
    AuthRecordResponse, AuthRecordsResponse, ConfigResponse, MigrationItem,
    MigrationRecordResponse, MigrationRecordsResponse,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const MIGRATION_RECORDS_BY_TIME: Map<U64Key, MigrationRecord> = Map::new("migration_records");
pub const AUTH_RECORDS_BY_TIME: Map<U64Key, AuthRecord> = Map::new("auth_records");
pub const AUTH_LIST: Map<&[u8], u64> = Map::new("auth_list");

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

//////////////////////////////////////////////////////////////////////
/// CONFIG
//////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub admin_claim_period: u64,
}

impl Config {
    pub fn as_res(&self, api: &dyn Api) -> StdResult<ConfigResponse> {
        let res = ConfigResponse {
            owner: api.addr_humanize(&self.owner)?.to_string(),
            admin_claim_period: self.admin_claim_period,
        };
        Ok(res)
    }
}

//////////////////////////////////////////////////////////////////////
/// AUTH RECORDS
//////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AuthRecord {
    pub address: CanonicalAddr,
    pub start_time: u64,
    pub end_time: u64,
}

impl AuthRecord {
    pub fn as_res(&self, api: &dyn Api) -> StdResult<AuthRecordResponse> {
        let res = AuthRecordResponse {
            address: api.addr_humanize(&self.address)?.to_string(),
            start_time: self.start_time,
            end_time: self.end_time,
        };
        Ok(res)
    }
}

pub fn create_auth_record(
    storage: &mut dyn Storage,
    addr_raw: CanonicalAddr,
    claim_start: u64,
    claim_end: u64,
) -> StdResult<()> {
    let record = AuthRecord {
        address: addr_raw.clone(),
        start_time: claim_start,
        end_time: claim_end,
    };

    // stores the record and adds a new entry to the list
    AUTH_LIST.save(storage, addr_raw.as_slice(), &claim_end)?;
    AUTH_RECORDS_BY_TIME.save(storage, claim_start.into(), &record)?;

    Ok(())
}

pub fn is_addr_authorized(
    storage: &dyn Storage,
    addr_raw: CanonicalAddr,
    current_time: u64,
) -> bool {
    match AUTH_LIST.load(storage, addr_raw.as_slice()) {
        Ok(claim_end) => claim_end >= current_time,
        Err(_) => false,
    }
}

pub fn read_latest_auth_records(
    storage: &dyn Storage,
    api: &dyn Api,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<AuthRecordsResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let end = calc_range_end(start_after).map(Bound::exclusive);

    let records: Vec<AuthRecordResponse> = AUTH_RECORDS_BY_TIME
        .range(storage, None, end, Order::Descending)
        .take(limit)
        .map(|item| {
            let (_, record) = item?;

            record.as_res(api)
        })
        .collect::<StdResult<Vec<AuthRecordResponse>>>()?;

    Ok(AuthRecordsResponse { records })
}

//////////////////////////////////////////////////////////////////////
/// MIGRATION RECORDS
//////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrationRecord {
    pub executor: CanonicalAddr,
    pub time: u64,
    pub migrations: Vec<(CanonicalAddr, u64, Binary)>,
}

impl MigrationRecord {
    pub fn as_res(&self, api: &dyn Api) -> StdResult<MigrationRecordResponse> {
        let migration_items: Vec<MigrationItem> = self
            .migrations
            .iter()
            .map(|item| {
                let res = MigrationItem {
                    contract: api.addr_humanize(&item.0)?.to_string(),
                    new_code_id: item.1,
                    msg: item.2.clone(),
                };
                Ok(res)
            })
            .collect::<StdResult<Vec<MigrationItem>>>()?;
        let res = MigrationRecordResponse {
            executor: api.addr_humanize(&self.executor)?.to_string(),
            time: self.time,
            migrations: migration_items,
        };
        Ok(res)
    }
}

pub fn read_latest_migration_records(
    storage: &dyn Storage,
    api: &dyn Api,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<MigrationRecordsResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let end = calc_range_end(start_after).map(Bound::exclusive);

    let records: Vec<MigrationRecordResponse> = MIGRATION_RECORDS_BY_TIME
        .range(storage, None, end, Order::Descending)
        .take(limit)
        .map(|item| {
            let (_, record) = item?;

            record.as_res(api)
        })
        .collect::<StdResult<Vec<MigrationRecordResponse>>>()?;

    Ok(MigrationRecordsResponse { records })
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_end(start_after: Option<u64>) -> Option<Vec<u8>> {
    start_after.map(|id| id.to_be_bytes().to_vec())
}
