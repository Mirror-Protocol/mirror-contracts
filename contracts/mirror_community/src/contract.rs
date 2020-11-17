use cosmwasm_std::{
    log, to_binary, Api, Binary, CosmosMsg, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    InitResponse, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::msg::{ConfigResponse, HandleMsg, InitMsg, QueryMsg};
use crate::state::{read_config, store_config, Config};

use cw20::Cw20HandleMsg;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&msg.owner)?,
            mirror_token: deps.api.canonical_address(&msg.mirror_token)?,
            spend_limit: msg.spend_limit,
        },
    )?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::UpdateConfig { owner, spend_limit } => {
            udpate_config(deps, env, owner, spend_limit)
        }
        HandleMsg::Spend { recipient, amount } => spend(deps, env, recipient, amount),
    }
}

pub fn udpate_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    spend_limit: Option<Uint128>,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(spend_limit) = spend_limit {
        config.spend_limit = spend_limit;
    }

    store_config(&mut deps.storage, &config)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

/// Spend
/// Owner can execute spend operation to send
/// `amount` of MIR token to `recipient` for community purpose
pub fn spend<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if config.spend_limit < amount {
        return Err(StdError::generic_err("Cannot spend more than spend_limit"));
    }

    let mirror_token = deps.api.human_address(&config.mirror_token)?;
    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: mirror_token,
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: recipient.clone(),
                amount,
            })?,
        })],
        log: vec![
            log("action", "spend"),
            log("recipient", recipient),
            log("amount", amount),
        ],
        data: None,
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        mirror_token: deps.api.human_address(&state.mirror_token)?,
        spend_limit: state.spend_limit,
    };

    Ok(resp)
}
