use std::str::FromStr;

use crate::errors::ContractError;
use crate::state::{read_config, Config};
use cosmwasm_std::{
    attr, to_binary, Addr, Coin, CosmosMsg, Decimal, DepsMut, Env, Response, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use mirror_protocol::collector::ExecuteMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use terra_cosmwasm::{create_swap_msg, TerraMsgWrapper};
use terraswap::asset::{Asset, AssetInfo, PairInfo};
use terraswap::pair::{Cw20HookMsg as TerraswapCw20HookMsg, ExecuteMsg as TerraswapExecuteMsg};
use terraswap::querier::{query_balance, query_pair_info, query_token_balance};

const LUNA_DENOM: &str = "uluna";
const AMM_MAX_ALLOWED_SLIPPAGE: &str = "0.5";

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MoneyMarketCw20HookMsg {
    RedeemStable {},
}

/// Convert
/// Anyone can execute convert function to swap
/// asset token => collateral token
/// collateral token => MIR token
pub fn convert(
    deps: DepsMut,
    env: Env,
    asset_token: Addr,
) -> Result<Response<TerraMsgWrapper>, ContractError> {
    let config: Config = read_config(deps.storage)?;
    let asset_token_raw = deps.api.addr_canonicalize(asset_token.as_str())?;

    if asset_token_raw == config.aust_token {
        anchor_redeem(deps, env, &config, asset_token)
    } else if asset_token_raw == config.bluna_token {
        bluna_swap(deps, env, &config, asset_token)
    } else if asset_token_raw == config.lunax_token {
        lunax_swap(deps, env, &config, asset_token)
    } else {
        direct_swap(deps, env, &config, asset_token)
    }
}

fn direct_swap(
    deps: DepsMut,
    env: Env,
    config: &Config,
    asset_token: Addr,
) -> Result<Response<TerraMsgWrapper>, ContractError> {
    let terraswap_factory_addr = deps.api.addr_humanize(&config.terraswap_factory)?;
    let asset_token_raw = deps.api.addr_canonicalize(asset_token.as_str())?;

    let pair_addr: String =
        if asset_token_raw == config.mirror_token && config.mir_ust_pair.is_some() {
            deps.api
                .addr_humanize(config.mir_ust_pair.as_ref().unwrap())?
                .to_string()
        } else {
            let pair_info: PairInfo = query_pair_info(
                &deps.querier,
                terraswap_factory_addr,
                &[
                    AssetInfo::NativeToken {
                        denom: config.base_denom.clone(),
                    },
                    AssetInfo::Token {
                        contract_addr: asset_token.to_string(),
                    },
                ],
            )?;

            pair_info.contract_addr
        };

    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = vec![];
    if config.mirror_token == asset_token_raw {
        // collateral token => MIR token
        let amount = query_balance(
            &deps.querier,
            env.contract.address,
            config.base_denom.clone(),
        )?;

        if !amount.is_zero() {
            let swap_asset = Asset {
                info: AssetInfo::NativeToken {
                    denom: config.base_denom.clone(),
                },
                amount,
            };

            // deduct tax first
            let amount = (swap_asset.deduct_tax(&deps.querier)?).amount;
            messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_addr,
                msg: to_binary(&TerraswapExecuteMsg::Swap {
                    offer_asset: Asset {
                        amount,
                        ..swap_asset
                    },
                    max_spread: Some(Decimal::from_str(AMM_MAX_ALLOWED_SLIPPAGE)?), // currently need to set max_allowed_slippage for Astroport
                    belief_price: None,
                    to: None,
                })?,
                funds: vec![Coin {
                    denom: config.base_denom.clone(),
                    amount,
                }],
            })];
        }
    } else {
        // asset token => collateral token
        let amount = query_token_balance(&deps.querier, asset_token.clone(), env.contract.address)?;

        if !amount.is_zero() {
            messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: pair_addr,
                    amount,
                    msg: to_binary(&TerraswapCw20HookMsg::Swap {
                        max_spread: None, // currently all mAsset swaps are on terraswap, so we set max_spread to None
                        belief_price: None,
                        to: None,
                    })?,
                })?,
                funds: vec![],
            })];
        }
    }

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "convert"),
            attr("swap_type", "direct"),
            attr("asset_token", asset_token.as_str()),
        ])
        .add_messages(messages))
}

fn anchor_redeem(
    deps: DepsMut,
    env: Env,
    config: &Config,
    asset_token: Addr,
) -> Result<Response<TerraMsgWrapper>, ContractError> {
    let amount = query_token_balance(&deps.querier, asset_token.clone(), env.contract.address)?;

    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = vec![];
    if !amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: deps.api.addr_humanize(&config.anchor_market)?.to_string(),
                amount,
                msg: to_binary(&MoneyMarketCw20HookMsg::RedeemStable {})?,
            })?,
            funds: vec![],
        }));
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "convert"),
        attr("swap_type", "anchor_redeem"),
        attr("asset_token", asset_token.as_str()),
    ]))
}

fn lunax_swap(
    deps: DepsMut,
    env: Env,
    config: &Config,
    asset_token: Addr,
) -> Result<Response<TerraMsgWrapper>, ContractError> {
    let terraswap_factory_addr = deps.api.addr_humanize(&config.terraswap_factory)?;

    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        terraswap_factory_addr,
        &[
            AssetInfo::NativeToken {
                denom: LUNA_DENOM.to_string(),
            },
            AssetInfo::Token {
                contract_addr: asset_token.to_string(),
            },
        ],
    )?;

    let amount = query_token_balance(
        &deps.querier,
        asset_token.clone(),
        env.contract.address.clone(),
    )?;

    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = vec![];
    if !amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: pair_info.contract_addr,
                amount,
                msg: to_binary(&TerraswapCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: None,
                })?,
            })?,
            funds: vec![],
        }));
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::LunaSwapHook {})?,
            funds: vec![],
        }));
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "convert"),
        attr("swap_type", "lunax_swap"),
        attr("asset_token", asset_token.as_str()),
    ]))
}

fn bluna_swap(
    deps: DepsMut,
    env: Env,
    config: &Config,
    asset_token: Addr,
) -> Result<Response<TerraMsgWrapper>, ContractError> {
    let terraswap_factory_addr = deps.api.addr_humanize(&config.terraswap_factory)?;

    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        terraswap_factory_addr,
        &[
            AssetInfo::NativeToken {
                denom: LUNA_DENOM.to_string(),
            },
            AssetInfo::Token {
                contract_addr: asset_token.to_string(),
            },
        ],
    )?;

    let amount = query_token_balance(
        &deps.querier,
        asset_token.clone(),
        env.contract.address.clone(),
    )?;

    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = vec![];
    if !amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: pair_info.contract_addr,
                amount,
                msg: to_binary(&TerraswapCw20HookMsg::Swap {
                    max_spread: None,
                    belief_price: None,
                    to: None,
                })?,
            })?,
            funds: vec![],
        }));
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::LunaSwapHook {})?,
            funds: vec![],
        }));
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "convert"),
        attr("swap_type", "bluna_swap"),
        attr("asset_token", asset_token.as_str()),
    ]))
}

pub fn luna_swap_hook(deps: DepsMut, env: Env) -> Result<Response<TerraMsgWrapper>, ContractError> {
    let config: Config = read_config(deps.storage)?;

    let amount = query_balance(&deps.querier, env.contract.address, "uluna".to_string())?;

    let mut messages: Vec<CosmosMsg<TerraMsgWrapper>> = vec![];
    if !amount.is_zero() {
        let offer_coin = Coin {
            amount,
            denom: LUNA_DENOM.to_string(),
        };
        messages.push(create_swap_msg(offer_coin, config.base_denom));
    }

    Ok(Response::new()
        .add_attributes(vec![attr("action", "luna_swap_hook")])
        .add_messages(messages))
}
