use crate::errors::ContractError;
use crate::state::{read_config, store_config, Config};
use crate::swap::{convert, luna_swap_hook};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use mirror_protocol::collector::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};
use mirror_protocol::gov::Cw20HookMsg::DepositReward;
use terra_cosmwasm::TerraMsgWrapper;
use terraswap::querier::query_token_balance;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    store_config(
        deps.storage,
        &Config {
            owner: deps.api.addr_canonicalize(&msg.owner)?,
            distribution_contract: deps.api.addr_canonicalize(&msg.distribution_contract)?,
            terraswap_factory: deps.api.addr_canonicalize(&msg.terraswap_factory)?,
            mirror_token: deps.api.addr_canonicalize(&msg.mirror_token)?,
            base_denom: msg.base_denom,
            aust_token: deps.api.addr_canonicalize(&msg.aust_token)?,
            anchor_market: deps.api.addr_canonicalize(&msg.anchor_market)?,
            bluna_token: deps.api.addr_canonicalize(&msg.bluna_token)?,
            bluna_swap_denom: msg.bluna_swap_denom,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<TerraMsgWrapper>, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
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
            info,
            owner,
            distribution_contract,
            terraswap_factory,
            mirror_token,
            base_denom,
            aust_token,
            anchor_market,
            bluna_token,
            bluna_swap_denom,
        ),
        ExecuteMsg::Convert { asset_token } => {
            let asset_addr = deps.api.addr_validate(&asset_token)?;
            convert(deps, env, asset_addr)
        }
        ExecuteMsg::Distribute {} => distribute(deps, env),
        ExecuteMsg::LunaSwapHook {} => luna_swap_hook(deps, env, info),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    distribution_contract: Option<String>,
    terraswap_factory: Option<String>,
    mirror_token: Option<String>,
    base_denom: Option<String>,
    aust_token: Option<String>,
    anchor_market: Option<String>,
    bluna_token: Option<String>,
    bluna_swap_denom: Option<String>,
) -> Result<Response<TerraMsgWrapper>, ContractError> {
    let mut config: Config = read_config(deps.storage)?;
    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(distribution_contract) = distribution_contract {
        config.distribution_contract = deps.api.addr_canonicalize(&distribution_contract)?;
    }

    if let Some(terraswap_factory) = terraswap_factory {
        config.terraswap_factory = deps.api.addr_canonicalize(&terraswap_factory)?;
    }

    if let Some(mirror_token) = mirror_token {
        config.mirror_token = deps.api.addr_canonicalize(&mirror_token)?;
    }

    if let Some(base_denom) = base_denom {
        config.base_denom = base_denom;
    }

    if let Some(aust_token) = aust_token {
        config.aust_token = deps.api.addr_canonicalize(&aust_token)?;
    }

    if let Some(anchor_market) = anchor_market {
        config.anchor_market = deps.api.addr_canonicalize(&anchor_market)?;
    }

    if let Some(bluna_token) = bluna_token {
        config.bluna_token = deps.api.addr_canonicalize(&bluna_token)?;
    }

    if let Some(bluna_swap_denom) = bluna_swap_denom {
        config.bluna_swap_denom = bluna_swap_denom;
    }

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

// Anyone can execute send function to receive staking token rewards
pub fn distribute(deps: DepsMut, env: Env) -> Result<Response<TerraMsgWrapper>, ContractError> {
    let config: Config = read_config(deps.storage)?;
    let amount = query_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.mirror_token)?,
        env.contract.address,
    )?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.mirror_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: deps
                    .api
                    .addr_humanize(&config.distribution_contract)?
                    .to_string(),
                amount,
                msg: to_binary(&DepositReward {})?,
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            attr("action", "distribute"),
            attr("amount", amount.to_string()),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.addr_humanize(&state.owner)?.to_string(),
        distribution_contract: deps
            .api
            .addr_humanize(&state.distribution_contract)?
            .to_string(),
        terraswap_factory: deps
            .api
            .addr_humanize(&state.terraswap_factory)?
            .to_string(),
        mirror_token: deps.api.addr_humanize(&state.mirror_token)?.to_string(),
        base_denom: state.base_denom,
        aust_token: deps.api.addr_humanize(&state.aust_token)?.to_string(),
        anchor_market: deps.api.addr_humanize(&state.anchor_market)?.to_string(),
        bluna_token: deps.api.addr_humanize(&state.bluna_token)?.to_string(),
        bluna_swap_denom: state.bluna_swap_denom,
    };

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
