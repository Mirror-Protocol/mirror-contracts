#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, WasmMsg,
};

use crate::state::{read_config, store_config, Config};

use cw20::Cw20ExecuteMsg;
use mirror_protocol::collector::{ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use mirror_protocol::gov::Cw20HookMsg::DepositReward;
use terraswap::asset::{Asset, AssetInfo, PairInfo};
use terraswap::pair::{Cw20HookMsg as TerraswapCw20HookMsg, ExecuteMsg as TerraswapExecuteMsg};
use terraswap::querier::{query_balance, query_pair_info, query_token_balance};

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
            distribution_contract: deps.api.addr_canonicalize(&msg.distribution_contract)?,
            terraswap_factory: deps.api.addr_canonicalize(&msg.terraswap_factory)?,
            mirror_token: deps.api.addr_canonicalize(&msg.mirror_token)?,
            base_denom: msg.base_denom,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Convert { asset_token } => convert(deps, env, asset_token),
        ExecuteMsg::Distribute {} => distribute(deps, env),
    }
}

/// Convert
/// Anyone can execute convert function to swap
/// asset token => collateral token
/// collateral token => MIR token
pub fn convert(
    deps: DepsMut,
    env: Env,
    asset_token: String,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let asset_token_raw = deps.api.addr_canonicalize(&asset_token)?;
    let asset_token_validated = deps.api.addr_validate(&asset_token)?;
    let terraswap_factory_raw = deps.api.addr_humanize(&config.terraswap_factory)?;

    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        terraswap_factory_raw,
        &[
            AssetInfo::NativeToken {
                denom: config.base_denom.to_string(),
            },
            AssetInfo::Token {
                contract_addr: asset_token_validated.clone(),
            },
        ],
    )?;

    let messages: Vec<CosmosMsg>;
    if config.mirror_token == asset_token_raw {
        // collateral token => MIR token
        let amount = query_balance(&deps.querier, env.contract.address, config.base_denom.to_string())?;
        let swap_asset = Asset {
            info: AssetInfo::NativeToken {
                denom: config.base_denom.clone(),
            },
            amount,
        };

        // deduct tax first
        let amount = (swap_asset.deduct_tax(&deps.querier)?).amount;
        messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pair_info.contract_addr.to_string(),
            msg: to_binary(&TerraswapExecuteMsg::Swap {
                offer_asset: Asset {
                    amount,
                    ..swap_asset
                },
                max_spread: None,
                belief_price: None,
                to: None,
            })?,
            send: vec![Coin {
                denom: config.base_denom,
                amount,
            }],
        })];
    } else {
        // asset token => collateral token
        let amount = query_token_balance(&deps.querier, deps.api, asset_token_validated.clone(), env.contract.address)?;

        messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token.clone(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: pair_info.contract_addr.to_string(),
                amount,
                msg: Some(to_binary(&TerraswapCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: None,
                })?),
            })?,
            send: vec![],
        })];
    }

    Ok(Response {
        messages,
        submessages: vec![],
        attributes: vec![
            attr("action", "convert"),
            attr("asset_token", asset_token.as_str()),
        ],
        data: None,
    })
}

// Anyone can execute send function to receive staking token rewards
pub fn distribute(
    deps: DepsMut,
    env: Env,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let amount = query_token_balance(
        &deps.querier,
        deps.api,
        deps.api.addr_humanize(&config.mirror_token)?,
        env.contract.address,
    )?;

    Ok(Response {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.mirror_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: deps.api.addr_humanize(&config.distribution_contract)?.to_string(),
                amount,
                msg: Some(to_binary(&DepositReward {})?),
            })?,
            send: vec![],
        })],
        submessages: vec![],
        attributes: vec![
            attr("action", "distribute"),
            attr("amount", amount.to_string()),
        ],
        data: None,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(
    deps: Deps,
    _env: Env,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config(
    deps: Deps,
) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        distribution_contract: deps.api.addr_humanize(&state.distribution_contract)?.to_string(),
        terraswap_factory: deps.api.addr_humanize(&state.terraswap_factory)?.to_string(),
        mirror_token: deps.api.addr_humanize(&state.mirror_token)?.to_string(),
        base_denom: state.base_denom,
    };

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(
    _deps: DepsMut,
    _env: Env,
    _msg: MigrateMsg,
) -> StdResult<Response> {
    Ok(Response::default())
}
