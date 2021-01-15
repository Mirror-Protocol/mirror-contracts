use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Binary, Coin, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, InitResponse, InitResult, MigrateResponse, MigrateResult, Querier,
    StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::math::{decimal_multiplication, reverse_decimal};
use crate::querier::{compute_tax, query_price};
use crate::state::{read_config, store_config, Config};

use cw20::Cw20HandleMsg;
use integer_sqrt::IntegerSquareRoot;
use mirror_protocol::mint::HandleMsg as MintHandleMsg;
use mirror_protocol::staker::{ConfigResponse, HandleMsg, InitMsg, MigrateMsg, QueryMsg};
use mirror_protocol::staking::Cw20HookMsg as StakingCw20HookMsg;
use terraswap::asset::{Asset, AssetInfo, PairInfo};
use terraswap::pair::HandleMsg as PairHandleMsg;
use terraswap::querier::{query_balance, query_pair_info, query_token_balance};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> InitResult {
    store_config(
        &mut deps.storage,
        &Config {
            mint_contract: deps.api.canonical_address(&msg.mint_contract)?,
            oracle_contract: deps.api.canonical_address(&msg.oracle_contract)?,
            staking_contract: deps.api.canonical_address(&msg.staking_contract)?,
            terraswap_factory: deps.api.canonical_address(&msg.terraswap_factory)?,
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
        HandleMsg::ExecuteBuyOperations {
            asset_token,
            belief_price,
            max_spread,
        } => execute_buy_operations(deps, env, asset_token, belief_price, max_spread),
        HandleMsg::ExecuteMintOperations {
            asset_token,
            collateral_ratio,
        } => execute_mint_operations(deps, env, asset_token, collateral_ratio),
        HandleMsg::ProvideOperation {
            asset_token,
            pair_contract,
        } => provide_operation(deps, env, asset_token, pair_contract),
        HandleMsg::StakeOperation {
            asset_token,
            liquidity_token,
            staker,
        } => stake_operation(deps, env, asset_token, liquidity_token, staker),
    }
}

/// ExecuteBuyOperations
/// Execute following messages
/// 1. swap half tokens
/// 2. provide liquidity
/// 3. stake lp token
pub fn execute_buy_operations<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let terraswap_factory = deps.api.human_address(&config.terraswap_factory)?;
    let pair_info: PairInfo = query_pair_info(
        &deps,
        &terraswap_factory,
        &[
            AssetInfo::NativeToken {
                denom: config.base_denom.to_string(),
            },
            AssetInfo::Token {
                contract_addr: asset_token.clone(),
            },
        ],
    )?;

    let amount: Uint128 = env
        .message
        .sent_funds
        .iter()
        .find(|c| c.denom == config.base_denom)
        .map(|c| c.amount)
        .unwrap_or_else(|| Uint128::zero());

    if amount.is_zero() {
        return Err(StdError::generic_err(
            "Cannot execute operations with zero balance",
        ));
    }

    // estimate required tax and pre-deduct it to prevent over swap
    // amount = amount - tax_amount_for_half_sending * 2
    let amount = Uint128(
        amount.u128()
            - compute_tax(
                &deps,
                Uint128(amount.u128() / 2),
                config.base_denom.to_string(),
            )?
            .u128()
                * 2,
    );

    // Load pool balance
    let native_pool_balance = query_balance(
        &deps,
        &pair_info.contract_addr,
        config.base_denom.to_string(),
    )?;

    // Extimated required asset amount without consideration of commission and tax
    // let swap_amount = sqrt(pool*(pool + deposit)) * sqrt(pool) - pool
    let swap_amount = Uint128(
        (native_pool_balance.u128() * (native_pool_balance.u128() + amount.u128())).integer_sqrt()
            - native_pool_balance.u128(),
    );

    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_info.contract_addr.clone(),
                msg: to_binary(&PairHandleMsg::Swap {
                    offer_asset: Asset {
                        amount: swap_amount,
                        info: AssetInfo::NativeToken {
                            denom: config.base_denom.to_string(),
                        },
                    },
                    belief_price,
                    max_spread,
                    to: None,
                })?,
                send: vec![Coin {
                    denom: config.base_denom,
                    amount: swap_amount,
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.clone(),
                msg: to_binary(&HandleMsg::ProvideOperation {
                    asset_token: asset_token.clone(),
                    pair_contract: pair_info.contract_addr,
                })?,
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address,
                msg: to_binary(&HandleMsg::StakeOperation {
                    asset_token,
                    liquidity_token: pair_info.liquidity_token,
                    staker: env.message.sender,
                })?,
                send: vec![],
            }),
        ],
        log: vec![
            log("action", "execute_buy_operations"),
            log("deposit_amount", amount),
        ],
        data: None,
    })
}

/// ExecuteBuyOperations
/// Execute following messages
/// 1. mint asset tokens
/// 2. provide liquidity
/// 3. stake lp token
pub fn execute_mint_operations<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    collateral_ratio: Decimal,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let terraswap_factory = deps.api.human_address(&config.terraswap_factory)?;
    let pair_info: PairInfo = query_pair_info(
        &deps,
        &terraswap_factory,
        &[
            AssetInfo::NativeToken {
                denom: config.base_denom.to_string(),
            },
            AssetInfo::Token {
                contract_addr: asset_token.clone(),
            },
        ],
    )?;

    let amount: Uint128 = env
        .message
        .sent_funds
        .iter()
        .find(|c| c.denom == config.base_denom)
        .map(|c| c.amount)
        .unwrap_or_else(|| Uint128::zero());

    if amount.is_zero() {
        return Err(StdError::generic_err(
            "Cannot execute operations with zero balance",
        ));
    }

    // estimate required tax and pre-deduct it to prevent over mint
    // amount = amount - tax_amount_for_half_sending * 2
    let amount = Uint128(
        amount.u128()
            - compute_tax(
                &deps,
                Uint128(amount.u128() / 2),
                config.base_denom.to_string(),
            )?
            .u128()
                * 2,
    );

    // oracle price
    let oracle_price = query_price(
        &deps,
        &deps.api.human_address(&config.oracle_contract)?,
        asset_token.to_string(),
        config.base_denom.to_string(),
        Some(env.block.time),
    )?;

    // pair price
    let native_balance_pair = query_balance(
        &deps,
        &pair_info.contract_addr,
        config.base_denom.to_string(),
    )?;
    let asset_balance_pair = query_token_balance(&deps, &asset_token, &pair_info.contract_addr)?;
    let pair_price = Decimal::from_ratio(native_balance_pair, asset_balance_pair);

    // collateral_amount
    //  = amount * collateral_ratio * oracle_price
    //    / (collateral_ratio * oracle_price + pair_price)
    let oracle_price = decimal_multiplication(oracle_price, collateral_ratio);
    let collateral_amount =
        amount * decimal_multiplication(oracle_price, reverse_decimal(oracle_price + pair_price));

    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.mint_contract)?,
                msg: to_binary(&MintHandleMsg::OpenPosition {
                    owner: Some(env.message.sender.clone()),
                    collateral: Asset {
                        info: AssetInfo::NativeToken {
                            denom: config.base_denom.to_string(),
                        },
                        amount: collateral_amount,
                    },
                    asset_info: AssetInfo::Token {
                        contract_addr: asset_token.clone(),
                    },
                    collateral_ratio,
                })?,
                send: vec![Coin {
                    denom: config.base_denom,
                    amount: collateral_amount,
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.clone(),
                msg: to_binary(&HandleMsg::ProvideOperation {
                    asset_token: asset_token.clone(),
                    pair_contract: pair_info.contract_addr,
                })?,
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address,
                msg: to_binary(&HandleMsg::StakeOperation {
                    asset_token,
                    liquidity_token: pair_info.liquidity_token,
                    staker: env.message.sender,
                })?,
                send: vec![],
            }),
        ],
        log: vec![
            log("action", "execute_mint_operations"),
            log("deposit_amount", amount),
        ],
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

    let native_balance = std::cmp::min(
        native_balance,
        Decimal::from_ratio(native_balance_pair, asset_balance_pair) * asset_balance,
    );

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
                    slippage_tolerance: None,
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
        mint_contract: deps.api.human_address(&state.mint_contract)?,
        oracle_contract: deps.api.human_address(&state.oracle_contract)?,
        staking_contract: deps.api.human_address(&state.staking_contract)?,
        terraswap_factory: deps.api.human_address(&state.terraswap_factory)?,
        base_denom: state.base_denom,
    };

    Ok(resp)
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> MigrateResult {
    Ok(MigrateResponse::default())
}
