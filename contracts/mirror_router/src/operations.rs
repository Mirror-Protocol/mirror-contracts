use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Coin, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, Querier, StdError, StdResult, Storage, WasmMsg,
};

use crate::querier::compute_tax;
use crate::state::{read_config, Config};

use cw20::Cw20HandleMsg;
use mirror_protocol::staking::Cw20HookMsg as StakingCw20HookMsg;
use terraswap::asset::{Asset, AssetInfo, PairInfo};
use terraswap::pair::HandleMsg as PairHandleMsg;
use terraswap::querier::{query_balance, query_pair_info, query_token_balance};

/// Execute buy operation
/// swap all offer asset to ask asset
pub fn buy_operation<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    offer_asset_info: AssetInfo,
    ask_asset_info: AssetInfo,
    max_spread: Option<Decimal>,
    to: Option<HumanAddr>,
) -> HandleResult {
    if env.contract.address != env.message.sender {
        return Err(StdError::unauthorized());
    }

    let config: Config = read_config(&deps.storage)?;
    let terraswap_factory = deps.api.human_address(&config.terraswap_factory)?;
    let pair_info: PairInfo = query_pair_info(
        &deps,
        &terraswap_factory,
        &[offer_asset_info.clone(), ask_asset_info.clone()],
    )?;

    let amount = match offer_asset_info.clone() {
        AssetInfo::NativeToken { denom } => {
            query_balance(&deps, &env.contract.address, denom.to_string())?
        }
        AssetInfo::Token { contract_addr } => {
            query_token_balance(&deps, &contract_addr, &env.contract.address)?
        }
    };

    let offer_asset: Asset = Asset {
        info: offer_asset_info,
        amount,
    };

    Ok(HandleResponse {
        messages: vec![asset_into_swap_msg(
            &deps,
            pair_info.contract_addr,
            offer_asset,
            max_spread,
            to,
        )?],
        log: vec![],
        data: None,
    })
}

/// Execute provide operation
pub fn provide_operation<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    pair_contract: HumanAddr,
) -> HandleResult {
    if env.message.sender != env.contract.address {
        return Err(StdError::unauthorized());
    }

    let config: Config = read_config(&deps.storage)?;
    let native_balance =
        query_balance(&deps, &env.contract.address, config.base_denom.to_string())?;
    let asset_balance = query_token_balance(&deps, &asset_token, &env.contract.address)?;

    let native_balance_pair = query_balance(&deps, &pair_contract, config.base_denom.to_string())?;
    let asset_balance_pair = query_token_balance(&deps, &asset_token, &pair_contract)?;

    let tax_amount = compute_tax(&deps, native_balance, config.base_denom.to_string())?;
    let native_balance = (native_balance - tax_amount)?;

    let required_native_balance =
        Decimal::from_ratio(native_balance_pair, asset_balance_pair) * asset_balance;
    let (native_balance, asset_balance) = if native_balance < required_native_balance {
        (
            native_balance,
            Decimal::from_ratio(asset_balance_pair, native_balance_pair) * native_balance,
        )
    } else {
        (required_native_balance, asset_balance)
    };

    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                msg: to_binary(&Cw20HandleMsg::IncreaseAllowance {
                    spender: pair_contract.clone(),
                    amount: asset_balance,
                    expires: None,
                })?,
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_contract,
                msg: to_binary(&PairHandleMsg::ProvideLiquidity {
                    assets: [
                        Asset {
                            info: AssetInfo::NativeToken {
                                denom: config.base_denom.to_string(),
                            },
                            amount: native_balance,
                        },
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: asset_token,
                            },
                            amount: asset_balance,
                        },
                    ],
                    slippage_tolerance: Some(Decimal::percent(1)),
                })?,
                send: vec![Coin {
                    denom: config.base_denom,
                    amount: native_balance,
                }],
            }),
        ],
        log: vec![],
        data: None,
    })
}

/// Execute stake operation
pub fn stake_operation<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    liquidity_token: HumanAddr,
    staker: HumanAddr,
) -> HandleResult {
    if env.message.sender != env.contract.address {
        return Err(StdError::unauthorized());
    }

    let config: Config = read_config(&deps.storage)?;
    let lp_amount = query_token_balance(&deps, &liquidity_token, &env.contract.address)?;
    let native_balance =
        query_balance(&deps, &env.contract.address, config.base_denom.to_string())?;
    let asset_balance = query_token_balance(&deps, &asset_token, &env.contract.address)?;

    // Refund left native tokens to staker
    let mut messages: Vec<CosmosMsg> = vec![];
    if !native_balance.is_zero() {
        let tax_amount = compute_tax(&deps, native_balance, config.base_denom.to_string())?;
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address.clone(),
            to_address: staker.clone(),
            amount: vec![Coin {
                denom: config.base_denom.clone(),
                amount: (native_balance - tax_amount)?,
            }],
        }));
    }

    // Refund left asset token to staker
    if !asset_balance.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token.clone(),
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: staker.clone(),
                amount: asset_balance,
            })?,
        }));
    }

    // Execute staking operation
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: liquidity_token,
        send: vec![],
        msg: to_binary(&Cw20HandleMsg::Send {
            contract: deps.api.human_address(&config.staking_contract)?,
            amount: lp_amount,
            msg: Some(to_binary(&StakingCw20HookMsg::Bond {
                asset_token: asset_token.clone(),
                staker: Some(staker),
            })?),
        })?,
    }));

    Ok(HandleResponse {
        messages,
        log: vec![
            log(
                "refund_native_amount",
                native_balance.to_string() + &config.base_denom,
            ),
            log(
                "refund_asset_amount",
                native_balance.to_string() + asset_token.as_str(),
            ),
        ],
        data: None,
    })
}

pub fn asset_into_swap_msg<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair_contract: HumanAddr,
    offer_asset: Asset,
    max_spread: Option<Decimal>,
    to: Option<HumanAddr>,
) -> StdResult<CosmosMsg> {
    match offer_asset.info.clone() {
        AssetInfo::NativeToken { denom } => {
            // deduct tax first
            let amount = (offer_asset.deduct_tax(&deps)?).amount;
            Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_contract,
                send: vec![Coin {
                    denom: denom,
                    amount,
                }],
                msg: to_binary(&PairHandleMsg::Swap {
                    offer_asset: Asset {
                        amount,
                        ..offer_asset
                    },
                    belief_price: None,
                    max_spread,
                    to,
                })?,
            }))
        }
        AssetInfo::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: pair_contract,
                amount: offer_asset.amount,
                msg: Some(to_binary(&PairHandleMsg::Swap {
                    offer_asset,
                    belief_price: None,
                    max_spread,
                    to,
                })?),
            })?,
        })),
    }
}
