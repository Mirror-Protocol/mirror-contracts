use cosmwasm_std::{
    log, to_binary, Api, Coin, CosmosMsg, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    Querier, StdError, Storage, WasmMsg,
};

use crate::state::{read_config, Config};

use cw20::Cw20HandleMsg;
use mirror_protocol::collector::HandleMsg;
use moneymarket::market::Cw20HookMsg::RedeemStable;
use terra_cosmwasm::{create_swap_msg, TerraMsgWrapper};
use terraswap::asset::{Asset, AssetInfo, PairInfo};
use terraswap::pair::{Cw20HookMsg as TerraswapCw20HookMsg, HandleMsg as TerraswapHandleMsg};
use terraswap::querier::{query_balance, query_pair_info, query_token_balance};

/// Convert
/// Anyone can execute convert function to swap
/// asset token => collateral token
/// collateral token => MIR token
pub fn convert<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
) -> HandleResult<TerraMsgWrapper> {
    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;

    if asset_token_raw == config.aust_token {
        anchor_redeem(deps, env, &config, asset_token)
    } else if asset_token_raw == config.bluna_token {
        bluna_swap(deps, env, &config, asset_token)
    } else {
        direct_swap(deps, env, &config, asset_token)
    }
}

fn direct_swap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    config: &Config,
    asset_token: HumanAddr,
) -> HandleResult<TerraMsgWrapper> {
    let terraswap_factory_raw = deps.api.human_address(&config.terraswap_factory)?;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;

    let pair_info: PairInfo = query_pair_info(
        &deps,
        &terraswap_factory_raw,
        &[
            AssetInfo::NativeToken {
                denom: config.base_denom.clone(),
            },
            AssetInfo::Token {
                contract_addr: asset_token.clone(),
            },
        ],
    )?;

    let messages: Vec<CosmosMsg<TerraMsgWrapper>>;
    if config.mirror_token == asset_token_raw {
        // collateral token => MIR token
        let amount = query_balance(&deps, &env.contract.address, config.base_denom.clone())?;
        let swap_asset = Asset {
            info: AssetInfo::NativeToken {
                denom: config.base_denom.clone(),
            },
            amount,
        };

        // deduct tax first
        let amount = (swap_asset.deduct_tax(&deps)?).amount;
        messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pair_info.contract_addr,
            msg: to_binary(&TerraswapHandleMsg::Swap {
                offer_asset: Asset {
                    amount,
                    ..swap_asset
                },
                max_spread: None,
                belief_price: None,
                to: None,
            })?,
            send: vec![Coin {
                denom: config.base_denom.clone(),
                amount,
            }],
        })];
    } else {
        // asset token => collateral token
        let amount = query_token_balance(&deps, &asset_token, &env.contract.address)?;

        messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token.clone(),
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: pair_info.contract_addr,
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

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "convert"),
            log("swap_type", "direct"),
            log("asset_token", asset_token.as_str()),
        ],
        data: None,
    })
}

fn anchor_redeem<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    config: &Config,
    asset_token: HumanAddr,
) -> HandleResult<TerraMsgWrapper> {
    let amount = query_token_balance(&deps, &asset_token, &env.contract.address)?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token.clone(),
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: deps.api.human_address(&config.anchor_market)?,
                amount,
                msg: Some(to_binary(&RedeemStable {})?),
            })?,
            send: vec![],
        })],
        log: vec![
            log("action", "convert"),
            log("swap_type", "anchor_redeem"),
            log("asset_token", asset_token.as_str()),
        ],
        data: None,
    })
}

fn bluna_swap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    config: &Config,
    asset_token: HumanAddr,
) -> HandleResult<TerraMsgWrapper> {
    let terraswap_factory_raw = deps.api.human_address(&config.terraswap_factory)?;

    let pair_info: PairInfo = query_pair_info(
        &deps,
        &terraswap_factory_raw,
        &[
            AssetInfo::NativeToken {
                denom: config.bluna_swap_denom.clone(),
            },
            AssetInfo::Token {
                contract_addr: asset_token.clone(),
            },
        ],
    )?;

    let amount = query_token_balance(&deps, &asset_token, &env.contract.address)?;

    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: pair_info.contract_addr,
                    amount,
                    msg: Some(to_binary(&TerraswapCw20HookMsg::Swap {
                        max_spread: None,
                        belief_price: None,
                        to: None,
                    })?),
                })?,
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address,
                msg: to_binary(&HandleMsg::LunaSwapHook {})?,
                send: vec![],
            }),
        ],
        log: vec![
            log("action", "convert"),
            log("swap_type", "bluna_swap"),
            log("asset_token", asset_token.as_str()),
        ],
        data: None,
    })
}

pub fn luna_swap_hook<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult<TerraMsgWrapper> {
    let config: Config = read_config(&deps.storage)?;

    if env.message.sender != deps.api.human_address(&config.owner)? {
        return Err(StdError::unauthorized());
    }

    let amount = query_balance(deps, &env.contract.address, config.bluna_swap_denom.clone())?;
    let offer_coin = Coin {
        amount,
        denom: config.bluna_swap_denom,
    };

    Ok(HandleResponse {
        messages: vec![create_swap_msg(
            env.contract.address,
            offer_coin,
            config.base_denom,
        )],
        log: vec![log("action", "luna_swap_hook")],
        data: None,
    })
}
