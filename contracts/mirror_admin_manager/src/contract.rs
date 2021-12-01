#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use mirror_protocol::admin_manager::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::{
    error::ContractError,
    handle::{authorize_claim, claim_admin, execute_migrations, update_owner},
    query::{query_auth_records, query_config, query_migration_records},
    state::{Config, CONFIG},
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        owner: deps.api.addr_canonicalize(&msg.owner)?,
        admin_claim_period: msg.admin_claim_period,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateOwner { owner } => update_owner(deps, info, owner),
        ExecuteMsg::AuthorizeClaim { authorized_addr } => {
            authorize_claim(deps, info, env, authorized_addr)
        }
        ExecuteMsg::ClaimAdmin { contract } => claim_admin(deps, info, env, contract),
        ExecuteMsg::ExecuteMigrations { migrations } => {
            execute_migrations(deps, info, env, migrations)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::AuthRecords { start_after, limit } => {
            to_binary(&query_auth_records(deps, start_after, limit)?)
        }
        QueryMsg::MigrationRecords { start_after, limit } => {
            to_binary(&query_migration_records(deps, start_after, limit)?)
        }
    }
}
