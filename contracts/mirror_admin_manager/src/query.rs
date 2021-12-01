use cosmwasm_std::{Deps, StdResult};
use mirror_protocol::admin_manager::{
    AuthRecordsResponse, ConfigResponse, MigrationRecordsResponse,
};

use crate::state::{read_latest_auth_records, read_latest_migration_records, Config, CONFIG};

/// Queries contract Config
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    config.as_res(deps.api)
}

/// Queries all auth records, ordered by timestamp (desc)
pub fn query_auth_records(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<AuthRecordsResponse> {
    read_latest_auth_records(deps.storage, deps.api, start_after, limit)
}

/// Queries all migration records, ordered by timestamp (desc)
pub fn query_migration_records(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<MigrationRecordsResponse> {
    read_latest_migration_records(deps.storage, deps.api, start_after, limit)
}
