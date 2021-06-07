use crate::migration::migrate_config;
use crate::state::{read_config, store_config, Config};
use crate::swap::{convert, luna_swap_hook};
use cosmwasm_std::{
    log, to_binary, Api, Binary, CosmosMsg, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    InitResponse, MigrateResponse, MigrateResult, Querier, StdError, StdResult, Storage, WasmMsg,
};
use cw20::Cw20HandleMsg;
use mirror_protocol::collector::{ConfigResponse, HandleMsg, InitMsg, MigrateMsg, QueryMsg};
use mirror_protocol::gov::Cw20HookMsg::DepositReward;
use terra_cosmwasm::TerraMsgWrapper;
use terraswap::querier::query_token_balance;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&msg.owner)?,
            distribution_contract: deps.api.canonical_address(&msg.distribution_contract)?,
            terraswap_factory: deps.api.canonical_address(&msg.terraswap_factory)?,
            mirror_token: deps.api.canonical_address(&msg.mirror_token)?,
            base_denom: msg.base_denom,
            aust_token: deps.api.canonical_address(&msg.aust_token)?,
            anchor_market: deps.api.canonical_address(&msg.anchor_market)?,
            bluna_token: deps.api.canonical_address(&msg.bluna_token)?,
            bluna_swap_denom: msg.bluna_swap_denom,
        },
    )?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult<TerraMsgWrapper> {
    match msg {
        HandleMsg::UpdateConfig {
            owner,
            distribution_contract,
            terraswap_factory,
            mirror_token,
            base_denom,
            aust_token,
            anchor_market,
            bluna_token,
            bluna_swap_denom,
        } => update_config(
            deps,
            env,
            distribution_contract,
            owner,
            terraswap_factory,
            mirror_token,
            base_denom,
            aust_token,
            anchor_market,
            bluna_token,
            bluna_swap_denom,
        ),
        HandleMsg::Convert { asset_token } => convert(deps, env, asset_token),
        HandleMsg::Distribute {} => distribute(deps, env),
        HandleMsg::LunaSwapHook {} => luna_swap_hook(deps, env),
    }
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    distribution_contract: Option<HumanAddr>,
    terraswap_factory: Option<HumanAddr>,
    mirror_token: Option<HumanAddr>,
    base_denom: Option<String>,
    aust_token: Option<HumanAddr>,
    anchor_market: Option<HumanAddr>,
    bluna_token: Option<HumanAddr>,
    bluna_swap_denom: Option<String>,
) -> HandleResult<TerraMsgWrapper> {
    let mut config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(distribution_contract) = distribution_contract {
        config.distribution_contract = deps.api.canonical_address(&distribution_contract)?;
    }

    if let Some(terraswap_factory) = terraswap_factory {
        config.terraswap_factory = deps.api.canonical_address(&terraswap_factory)?;
    }

    if let Some(mirror_token) = mirror_token {
        config.mirror_token = deps.api.canonical_address(&mirror_token)?;
    }

    if let Some(base_denom) = base_denom {
        config.base_denom = base_denom;
    }

    if let Some(aust_token) = aust_token {
        config.aust_token = deps.api.canonical_address(&aust_token)?;
    }

    if let Some(anchor_market) = anchor_market {
        config.anchor_market = deps.api.canonical_address(&anchor_market)?;
    }

    if let Some(bluna_token) = bluna_token {
        config.bluna_token = deps.api.canonical_address(&bluna_token)?;
    }

    if let Some(bluna_swap_denom) = bluna_swap_denom {
        config.bluna_swap_denom = bluna_swap_denom;
    }

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

// Anyone can execute send function to receive staking token rewards
pub fn distribute<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult<TerraMsgWrapper> {
    let config: Config = read_config(&deps.storage)?;
    let amount = query_token_balance(
        &deps,
        &deps.api.human_address(&config.mirror_token)?,
        &env.contract.address,
    )?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.mirror_token)?,
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: deps.api.human_address(&config.distribution_contract)?,
                amount,
                msg: Some(to_binary(&DepositReward {})?),
            })?,
            send: vec![],
        })],
        log: vec![
            log("action", "distribute"),
            log("amount", amount.to_string()),
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
        distribution_contract: deps.api.human_address(&state.distribution_contract)?,
        terraswap_factory: deps.api.human_address(&state.terraswap_factory)?,
        mirror_token: deps.api.human_address(&state.mirror_token)?,
        base_denom: state.base_denom,
        aust_token: deps.api.human_address(&state.aust_token)?,
        anchor_market: deps.api.human_address(&state.anchor_market)?,
        bluna_token: deps.api.human_address(&state.bluna_token)?,
        bluna_swap_denom: state.bluna_swap_denom,
    };

    Ok(resp)
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: MigrateMsg,
) -> MigrateResult {
    // migrate config
    migrate_config(
        &mut deps.storage,
        deps.api.canonical_address(&msg.owner)?,
        deps.api.canonical_address(&msg.aust_token)?,
        deps.api.canonical_address(&msg.anchor_market)?,
        deps.api.canonical_address(&msg.bluna_token)?,
        msg.bluna_swap_denom,
    )?;

    Ok(MigrateResponse::default())
}
