use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, Coin, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, InitResult, MigrateResponse,
    MigrateResult, Querier, QueryRequest, StdError, StdResult, Storage, Uint128, WasmMsg,
    WasmQuery,
};

use crate::math::{decimal_multiplication, reverse_decimal};
use crate::operations::{buy_operation, provide_operation, stake_operation};
use crate::querier::{compute_tax, query_price};
use crate::state::{read_config, store_config, Config};

use cw20::Cw20ReceiveMsg;
use integer_sqrt::IntegerSquareRoot;
use mirror_protocol::mint::HandleMsg as MintHandleMsg;
use mirror_protocol::router::{
    BuyWithRoutesResponse, ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, MigrateMsg, QueryMsg,
};
use terraswap::asset::{Asset, AssetInfo, PairInfo};
use terraswap::pair::{HandleMsg as PairHandleMsg, QueryMsg as PairQueryMsg, SimulationResponse};
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
        HandleMsg::Receive(msg) => receive_cw20(deps, env, msg),
        HandleMsg::BuyAndStake {
            asset_token,
            belief_price,
            max_spread,
        } => buy_and_stake(deps, env, asset_token, belief_price, max_spread),
        HandleMsg::MintAndStake {
            asset_token,
            collateral_ratio,
        } => mint_and_stake(deps, env, asset_token, collateral_ratio),
        HandleMsg::BuyWithRoutes {
            offer_asset_info,
            routes,
            max_spread,
        } => buy_with_routes(
            deps,
            env.clone(),
            env.message.sender,
            offer_asset_info,
            routes,
            max_spread,
        ),
        HandleMsg::BuyOperation {
            offer_asset_info,
            ask_asset_info,
            max_spread,
            to,
        } => buy_operation(deps, env, offer_asset_info, ask_asset_info, max_spread, to),
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

pub fn receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> HandleResult {
    if let Some(msg) = cw20_msg.msg {
        match from_binary(&msg)? {
            Cw20HookMsg::BuyWithRoutes { routes, max_spread } => {
                let offer_asset_info = AssetInfo::Token {
                    contract_addr: env.message.sender.clone(),
                };

                buy_with_routes(
                    deps,
                    env,
                    cw20_msg.sender,
                    offer_asset_info,
                    routes,
                    max_spread,
                )
            }
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

/// BuyAndStake
/// Execute following messages
/// 1. swap half tokens
/// 2. provide liquidity
/// 3. stake lp token
pub fn buy_and_stake<S: Storage, A: Api, Q: Querier>(
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
    // let swap_amount = sqrt(pool*(pool + deposit)) - pool
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
            log("action", "buy_and_stake"),
            log("deposit_amount", amount),
        ],
        data: None,
    })
}

/// BuyAndStake
/// Execute following messages
/// 1. mint asset tokens
/// 2. provide liquidity
/// 3. stake lp token
pub fn mint_and_stake<S: Storage, A: Api, Q: Querier>(
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
            log("action", "mint_and_stake"),
            log("deposit_amount", amount),
        ],
        data: None,
    })
}

pub fn buy_with_routes<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    offer_asset_info: AssetInfo,
    routes: Vec<AssetInfo>,
    max_spread: Option<Decimal>,
) -> HandleResult {
    let routes_len = routes.len();
    if routes_len == 0 {
        return Err(StdError::generic_err("must provide routes"));
    }

    let mut offer_asset_info = offer_asset_info;
    let mut route_idx = 0;
    let messages: Vec<CosmosMsg> = routes
        .into_iter()
        .map(|r| {
            route_idx += 1;
            let message = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.clone(),
                send: vec![],
                msg: to_binary(&HandleMsg::BuyOperation {
                    offer_asset_info: offer_asset_info.clone(),
                    ask_asset_info: r.clone(),
                    max_spread,
                    to: if routes_len == route_idx {
                        Some(sender.clone())
                    } else {
                        None
                    },
                })?,
            });

            offer_asset_info = r;
            Ok(message)
        })
        .collect::<StdResult<Vec<CosmosMsg>>>()?;

    //offer.into_msg(&deps, env.contract.address, pair_info.contract_addr)?
    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::BuyWithRoutes {
            offer_asset,
            routes,
        } => to_binary(&query_buy_with_routes(deps, offer_asset, routes)?),
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

fn query_buy_with_routes<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    offer_asset: Asset,
    routes: Vec<AssetInfo>,
) -> StdResult<BuyWithRoutesResponse> {
    let config: Config = read_config(&deps.storage)?;
    let terraswap_factory = deps.api.human_address(&config.terraswap_factory)?;

    let mut offer_asset = offer_asset;
    for route in routes.into_iter() {
        let pair_info: PairInfo = query_pair_info(
            &deps,
            &terraswap_factory,
            &[offer_asset.info.clone(), route.clone()],
        )?;

        // Deduct tax before querying simulation
        match offer_asset.info.clone() {
            AssetInfo::NativeToken { denom } => {
                offer_asset.amount =
                    (offer_asset.amount - compute_tax(&deps, offer_asset.amount, denom)?)?;
            }
            _ => {}
        }

        let mut res: SimulationResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: HumanAddr::from(pair_info.contract_addr),
                msg: to_binary(&PairQueryMsg::Simulation { offer_asset })?,
            }))?;

        // Deduct tax after querying simulation
        match route.clone() {
            AssetInfo::NativeToken { denom } => {
                res.return_amount =
                    (res.return_amount - compute_tax(&deps, res.return_amount, denom)?)?;
            }
            _ => {}
        }

        offer_asset = Asset {
            info: route,
            amount: res.return_amount,
        };
    }

    Ok(BuyWithRoutesResponse {
        amount: offer_asset.amount,
    })
}
