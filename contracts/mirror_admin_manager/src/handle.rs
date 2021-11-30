use cosmwasm_std::{
    attr, Binary, CanonicalAddr, CosmosMsg, DepsMut, Env, MessageInfo, Response, WasmMsg,
};

use crate::{
    error::ContractError,
    state::{
        create_auth_record, is_addr_authorized, Config, MigrationRecord, CONFIG,
        MIGRATION_RECORDS_BY_TIME,
    },
};

/// Updates the owner of the contract
pub fn update_owner(
    deps: DepsMut,
    info: MessageInfo,
    owner: String,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    let sender_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    if sender_raw != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // validate and convert to raw
    deps.api.addr_validate(owner.as_str())?;
    let new_owner_raw: CanonicalAddr = deps.api.addr_canonicalize(owner.as_str())?;

    config.owner = new_owner_raw;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_owner"))
}

/// Owner can authorize an `authorized_address` to execute `claim_admin` for a limited time period
pub fn authorize_claim(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    authorized_addr: String,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    let sender_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    if sender_raw != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // validate and convert authorized address
    deps.api.addr_validate(authorized_addr.as_str())?;
    let authorized_addr_raw: CanonicalAddr =
        deps.api.addr_canonicalize(authorized_addr.as_str())?;

    let claim_start = env.block.time.seconds();
    let claim_end = claim_start + config.admin_claim_period;
    create_auth_record(deps.storage, authorized_addr_raw, claim_start, claim_end)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "authorize_claim"),
        attr("claim_start", claim_start.to_string()),
        attr("claim_end", claim_end.to_string()),
    ]))
}

/// An `authorized_address` can claim admin privilages on a `contract` during the auth period
pub fn claim_admin(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    contract: String,
) -> Result<Response, ContractError> {
    let sender_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    if !is_addr_authorized(deps.storage, sender_raw, env.block.time.seconds()) {
        return Err(ContractError::Unauthorized {});
    }

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::UpdateAdmin {
            contract_addr: contract.to_string(),
            admin: info.sender.to_string(),
        }))
        .add_attributes(vec![
            attr("action", "claim_admin"),
            attr("contract", contract),
        ]))
}

/// Owner (gov contract) can execute_migrations on any of the managed contracts, creating a migration_record
pub fn execute_migrations(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    migrations: Vec<(String, u64, Binary)>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    let sender_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    if sender_raw != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut migration_msgs: Vec<CosmosMsg> = vec![];
    let mut migrations_raw: Vec<(CanonicalAddr, u64, Binary)> = vec![];

    for migration in migrations.iter() {
        migration_msgs.push(CosmosMsg::Wasm(WasmMsg::Migrate {
            contract_addr: migration.0.to_string(),
            new_code_id: migration.1,
            msg: migration.2.clone(),
        }));

        let contract_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(migration.0.as_str())?;
        migrations_raw.push((contract_addr_raw, migration.1, migration.2.clone()));
    }

    let migration_record = MigrationRecord {
        executor: sender_raw,
        time: env.block.time.seconds(),
        migrations: migrations_raw,
    };
    MIGRATION_RECORDS_BY_TIME.save(
        deps.storage,
        env.block.time.seconds().into(),
        &migration_record,
    )?;

    Ok(Response::new()
        .add_messages(migration_msgs)
        .add_attribute("action", "execute_migrations"))
}
