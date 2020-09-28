use cosmwasm_std::{
    log, to_binary, Api, Binary, Coin, CosmosMsg, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, InitResponse, Querier, StdResult, Storage, WasmMsg,
};

use crate::msg::{
    ConfigResponse, HandleMsg, InitMsg, QueryMsg, TerraSwapCw20HookMsg, TerraSwapHandleMsg,
};
use crate::state::{read_config, store_config, Config};

use cw20::Cw20HandleMsg;
use terraswap::{load_balance, load_pair_contract, load_token_balance, Asset, AssetInfo};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            distribution_contract: deps.api.canonical_address(&msg.distribution_contract)?,
            terraswap_factory: deps.api.canonical_address(&msg.terraswap_factory)?,
            mirror_token: deps.api.canonical_address(&msg.mirror_token)?,
            base_denom: msg.base_denom,
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
        HandleMsg::Convert { asset_token } => try_convert(deps, env, asset_token),
        HandleMsg::Send {} => try_send(deps, env),
    }
}

/// Convert
/// Anyone can execute convert function to swap
/// asset token => collateral denom
/// collateral denom => mirror token
pub fn try_convert<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let terraswap_factory_raw = deps.api.human_address(&config.terraswap_factory)?;

    let pair_contract = load_pair_contract(
        &deps,
        &terraswap_factory_raw,
        &[
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: asset_token.clone(),
            },
        ],
    )?;

    let messages: Vec<CosmosMsg>;
    if config.mirror_token == asset_token_raw {
        // uusd => staking token
        let amount = load_balance(&deps, &env.contract.address, config.base_denom.to_string())?;
        let swap_asset = Asset {
            info: AssetInfo::NativeToken {
                denom: config.base_denom.clone(),
            },
            amount,
        };

        // deduct tax first
        let amount = (swap_asset.deduct_tax(&deps)?).amount;
        messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pair_contract,
            msg: to_binary(&TerraSwapHandleMsg::Swap {
                offer_asset: Asset {
                    amount,
                    ..swap_asset
                },
                max_spread: None,
            })?,
            send: vec![Coin {
                denom: config.base_denom,
                amount,
            }],
        })];
    } else {
        // asset token => uusd
        let amount = load_token_balance(&deps, &asset_token, &env.contract.address)?;

        messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token.clone(),
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: pair_contract.clone(),
                amount,
                msg: Some(to_binary(&TerraSwapCw20HookMsg::Swap { max_spread: None })?),
            })?,
            send: vec![],
        })];
    }

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "convert"),
            log("asset_token", asset_token.as_str()),
        ],
        data: None,
    })
}

// Anyone can execute send function to receive staking token rewards
pub fn try_send<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let amount = load_token_balance(
        &deps,
        &deps.api.human_address(&config.mirror_token)?,
        &env.contract.address,
    )?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.mirror_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: deps.api.human_address(&config.distribution_contract)?,
                amount,
            })?,
            send: vec![],
        })],
        log: vec![log("action", "send"), log("amount", amount.to_string())],
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
        distribution_contract: deps.api.human_address(&state.distribution_contract)?,
        terraswap_factory: deps.api.human_address(&state.terraswap_factory)?,
        mirror_token: deps.api.human_address(&state.mirror_token)?,
        base_denom: state.base_denom,
    };

    Ok(resp)
}
