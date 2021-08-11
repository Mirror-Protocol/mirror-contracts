#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Addr, Attribute, Binary, CanonicalAddr, CosmosMsg, Decimal, Deps, DepsMut,
    Env, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};

use crate::querier::{load_mint_asset_config, load_oracle_feeder};
use crate::response::MsgInstantiateContractResponse;
use crate::state::{
    decrease_total_weight, increase_total_weight, read_all_weight, read_config,
    read_last_distributed, read_params, read_tmp_asset, read_tmp_oracle, read_total_weight,
    read_weight, remove_params, remove_weight, store_config, store_last_distributed, store_params,
    store_tmp_asset, store_tmp_oracle, store_total_weight, store_weight, Config,
};

use mirror_protocol::factory::{
    ConfigResponse, DistributionInfoResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, Params,
    QueryMsg,
};
use mirror_protocol::mint::{ExecuteMsg as MintExecuteMsg, IPOParams};
use mirror_protocol::oracle::ExecuteMsg as OracleExecuteMsg;
use mirror_protocol::staking::Cw20HookMsg as StakingCw20HookMsg;
use mirror_protocol::staking::ExecuteMsg as StakingExecuteMsg;

use protobuf::Message;

use cw20::{Cw20ExecuteMsg, MinterResponse};
use terraswap::asset::{AssetInfo, PairInfo};
use terraswap::factory::ExecuteMsg as TerraswapFactoryExecuteMsg;
use terraswap::querier::query_pair_info;
use terraswap::token::InstantiateMsg as TokenInstantiateMsg;

const MIRROR_TOKEN_WEIGHT: u32 = 300u32;
const NORMAL_TOKEN_WEIGHT: u32 = 30u32;
const DISTRIBUTION_INTERVAL: u64 = 60u64;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    store_config(
        deps.storage,
        &Config {
            owner: CanonicalAddr::from(vec![]),
            mirror_token: CanonicalAddr::from(vec![]),
            mint_contract: CanonicalAddr::from(vec![]),
            oracle_contract: CanonicalAddr::from(vec![]),
            terraswap_factory: CanonicalAddr::from(vec![]),
            staking_contract: CanonicalAddr::from(vec![]),
            commission_collector: CanonicalAddr::from(vec![]),
            token_code_id: msg.token_code_id,
            base_denom: msg.base_denom,
            genesis_time: env.block.time.nanos() / 1_000_000_000,
            distribution_schedule: msg.distribution_schedule,
        },
    )?;

    store_total_weight(deps.storage, 0u32)?;
    store_last_distributed(deps.storage, env.block.time.nanos() / 1_000_000_000)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::PostInitialize {
            owner,
            mirror_token,
            mint_contract,
            oracle_contract,
            terraswap_factory,
            staking_contract,
            commission_collector,
        } => post_initialize(
            deps,
            env,
            owner,
            mirror_token,
            mint_contract,
            oracle_contract,
            terraswap_factory,
            staking_contract,
            commission_collector,
        ),
        ExecuteMsg::UpdateConfig {
            owner,
            token_code_id,
            distribution_schedule,
        } => update_config(deps, info, owner, token_code_id, distribution_schedule),
        ExecuteMsg::UpdateWeight {
            asset_token,
            weight,
        } => update_weight(deps, info, asset_token, weight),
        ExecuteMsg::Whitelist {
            name,
            symbol,
            oracle_feeder,
            params,
        } => whitelist(deps, info, name, symbol, oracle_feeder, params),
        ExecuteMsg::Distribute {} => distribute(deps, env),
        ExecuteMsg::PassCommand { contract_addr, msg } => {
            pass_command(deps, info, contract_addr, msg)
        }
        ExecuteMsg::RevokeAsset {
            asset_token,
            end_price,
        } => revoke_asset(deps, info, asset_token, end_price),
        ExecuteMsg::MigrateAsset {
            name,
            symbol,
            from_token,
            end_price,
        } => migrate_asset(deps, info, name, symbol, from_token, end_price),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn post_initialize(
    deps: DepsMut,
    env: Env,
    owner: String,
    mirror_token: String,
    mint_contract: String,
    oracle_contract: String,
    terraswap_factory: String,
    staking_contract: String,
    commission_collector: String,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;
    if config.owner != CanonicalAddr::from(vec![]) {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.owner = deps.api.addr_canonicalize(&owner)?;
    config.mirror_token = deps.api.addr_canonicalize(&mirror_token)?;
    config.mint_contract = deps.api.addr_canonicalize(&mint_contract)?;
    config.oracle_contract = deps.api.addr_canonicalize(&oracle_contract)?;
    config.terraswap_factory = deps.api.addr_canonicalize(&terraswap_factory)?;
    config.staking_contract = deps.api.addr_canonicalize(&staking_contract)?;
    config.commission_collector = deps.api.addr_canonicalize(&commission_collector)?;
    store_config(deps.storage, &config)?;

    // MIR Token and Pair are registered externally, update weights,
    // and register to staking contract
    store_weight(deps.storage, &config.mirror_token, MIRROR_TOKEN_WEIGHT)?;
    increase_total_weight(deps.storage, MIRROR_TOKEN_WEIGHT)?;

    let mir_addr = deps.api.addr_humanize(&config.mirror_token)?;

    terraswap_creation_hook(deps, env, mir_addr)
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    token_code_id: Option<u64>,
    distribution_schedule: Option<Vec<(u64, u64, Uint128)>>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;
    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(distribution_schedule) = distribution_schedule {
        config.distribution_schedule = distribution_schedule;
    }

    if let Some(token_code_id) = token_code_id {
        config.token_code_id = token_code_id;
    }

    store_config(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

pub fn update_weight(
    deps: DepsMut,
    info: MessageInfo,
    asset_token: String,
    weight: u32,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    let asset_token_raw = deps.api.addr_canonicalize(&asset_token)?;
    let origin_weight = read_weight(deps.storage, &asset_token_raw)?;
    store_weight(deps.storage, &asset_token_raw, weight)?;

    let origin_total_weight = read_total_weight(deps.storage)?;
    let updated_total_weight = origin_total_weight + weight - origin_weight;
    store_total_weight(deps.storage, updated_total_weight)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "update_weight"),
        attr("asset_token", asset_token),
        attr("weight", weight.to_string()),
    ]))
}

// just for by passing command to other contract like update config
pub fn pass_command(
    deps: DepsMut,
    info: MessageInfo,
    contract_addr: String,
    msg: Binary,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    Ok(
        Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds: vec![],
        })),
    )
}

/// Whitelisting process
/// 1. Create asset token contract with `config.token_code_id` with `minter` argument
/// 2. Call `TokenCreationHook`
///    2-1. Initialize distribution info
///    2-2. Register asset to mint contract
///    2-3. Register asset and oracle feeder to oracle contract
///    2-4. Create terraswap pair through terraswap factory
/// 3. Call `TerraswapCreationHook`
///    3-1. Register asset to staking contract
pub fn whitelist(
    deps: DepsMut,
    info: MessageInfo,
    name: String,
    symbol: String,
    oracle_feeder: String,
    params: Params,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    if read_params(deps.storage).is_ok() {
        return Err(StdError::generic_err("A whitelist process is in progress"));
    }

    store_params(deps.storage, &params)?;

    // store oracle in temp storage to use in reply callback
    let oracle = deps.api.addr_validate(&oracle_feeder)?;
    store_tmp_oracle(deps.storage, &oracle)?;

    Ok(Response::new()
        .add_submessage(SubMsg {
            // create asset token
            msg: WasmMsg::Instantiate {
                admin: None,
                code_id: config.token_code_id,
                funds: vec![],
                label: "".to_string(),
                msg: to_binary(&TokenInstantiateMsg {
                    name: name.clone(),
                    symbol: symbol.to_string(),
                    decimals: 6u8,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: deps.api.addr_humanize(&config.mint_contract)?.to_string(),
                        cap: None,
                    }),
                })?,
            }
            .into(),
            gas_limit: None,
            id: 1,
            reply_on: ReplyOn::Success,
        })
        .add_attributes(vec![
            attr("action", "whitelist"),
            attr("symbol", symbol),
            attr("name", name),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        1 => {
            // fetch saved oracle_feeder from temp state
            let oracle_feeder = read_tmp_oracle(deps.storage)?;

            // get new token's contract address
            let res: MsgInstantiateContractResponse = Message::parse_from_bytes(
                msg.result.unwrap().data.unwrap().as_slice(),
            )
            .map_err(|_| {
                StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
            })?;
            let asset_token = Addr::unchecked(res.get_contract_address());

            token_creation_hook(deps, env, asset_token, oracle_feeder)
        }
        2 => {
            // fetch saved asset_token from temp state
            let asset_token = read_tmp_asset(deps.storage)?;

            terraswap_creation_hook(deps, env, asset_token)
        }
        _ => Err(StdError::generic_err("reply id is invalid")),
    }
}

/// TokenCreationHook
/// 1. Initialize distribution info
/// 2. Register asset to mint contract
/// 3. Register asset and oracle feeder to oracle contract
/// 4. Create terraswap pair through terraswap factory with `TerraswapCreationHook`
pub fn token_creation_hook(
    deps: DepsMut,
    env: Env,
    asset_token: Addr,
    oracle_feeder: Addr,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;

    // If the param is not exists, it means there is no whitelist process in progress
    let params: Params = match read_params(deps.storage) {
        Ok(v) => v,
        Err(_) => return Err(StdError::generic_err("No whitelist process in progress")),
    };

    // let asset_token = env.message.sender;
    let asset_token_raw = deps.api.addr_canonicalize(asset_token.as_str())?;

    // If weight is given as params, we use that or just use default
    let weight = if let Some(weight) = params.weight {
        weight
    } else {
        NORMAL_TOKEN_WEIGHT
    };

    // Increase total weight
    store_weight(deps.storage, &asset_token_raw, weight)?;
    increase_total_weight(deps.storage, weight)?;

    // Remove params == clear flag
    remove_params(deps.storage);

    let mut attributes: Vec<Attribute> = vec![];

    // Check if all IPO params exist
    let ipo_params: Option<IPOParams> =
        if let (Some(mint_period), Some(min_collateral_ratio_after_ipo), Some(pre_ipo_price)) = (
            params.mint_period,
            params.min_collateral_ratio_after_ipo,
            params.pre_ipo_price,
        ) {
            let mint_end: u64 = env.block.time.plus_seconds(mint_period).nanos() / 1_000_000_000;
            attributes = vec![
                attr("is_pre_ipo", "true"),
                attr("mint_end", mint_end.to_string()),
                attr(
                    "min_collateral_ratio_after_ipo",
                    min_collateral_ratio_after_ipo.to_string(),
                ),
                attr("pre_ipo_price", pre_ipo_price.to_string()),
            ];
            Some(IPOParams {
                mint_end: mint_end,
                min_collateral_ratio_after_ipo,
                pre_ipo_price,
            })
        } else {
            attributes.push(attr("is_pre_ipo", "false"));
            None
        };

    // store asset_token in temp storage to use in reply callback
    store_tmp_asset(deps.storage, &asset_token)?;

    // Register asset to mint contract
    // Register asset to oracle contract
    // Create terraswap pair
    Ok(Response::new()
        .add_messages(vec![
            //ASSUMPTION: These don't need to be done before pair / lp token is made
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.mint_contract)?.to_string(),
                funds: vec![],
                msg: to_binary(&MintExecuteMsg::RegisterAsset {
                    asset_token: asset_token.to_string(),
                    auction_discount: params.auction_discount,
                    min_collateral_ratio: params.min_collateral_ratio,
                    ipo_params,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.oracle_contract)?.to_string(),
                funds: vec![],
                msg: to_binary(&OracleExecuteMsg::RegisterAsset {
                    asset_token: asset_token.to_string(),
                    feeder: oracle_feeder.to_string(),
                })?,
            }),
        ])
        .add_submessage(SubMsg {
            // create terraswap pair
            msg: WasmMsg::Execute {
                contract_addr: deps
                    .api
                    .addr_humanize(&config.terraswap_factory)?
                    .to_string(),
                funds: vec![],
                msg: to_binary(&TerraswapFactoryExecuteMsg::CreatePair {
                    asset_infos: [
                        AssetInfo::NativeToken {
                            denom: config.base_denom,
                        },
                        AssetInfo::Token {
                            contract_addr: asset_token.to_string(),
                        },
                    ],
                })?,
            }
            .into(),
            gas_limit: None,
            id: 2,
            reply_on: ReplyOn::Success,
        })
        .add_attributes(
            vec![
                vec![attr("asset_token_addr", asset_token.as_str())],
                attributes,
            ]
            .concat(),
        ))
}

/// TerraswapCreationHook
/// 1. Register asset and liquidity(LP) token to staking contract
pub fn terraswap_creation_hook(deps: DepsMut, _env: Env, asset_token: Addr) -> StdResult<Response> {
    // Now terraswap contract is already created,
    // and liquidity token also created
    let config: Config = read_config(deps.storage)?;

    let asset_infos = [
        AssetInfo::NativeToken {
            denom: config.base_denom,
        },
        AssetInfo::Token {
            contract_addr: asset_token.to_string(),
        },
    ];

    // Load terraswap pair info
    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        deps.api.addr_humanize(&config.terraswap_factory)?,
        &asset_infos,
    )?;

    // Execute staking contract to register staking token of newly created asset
    Ok(
        Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps
                .api
                .addr_humanize(&config.staking_contract)?
                .to_string(),
            funds: vec![],
            msg: to_binary(&StakingExecuteMsg::RegisterAsset {
                asset_token: asset_token.to_string(),
                staking_token: pair_info.liquidity_token.to_string(),
            })?,
        })),
    )
}

/// Distribute
/// Anyone can execute distribute operation to distribute
/// mirror inflation rewards on the staking pool
pub fn distribute(deps: DepsMut, env: Env) -> StdResult<Response> {
    let last_distributed = read_last_distributed(deps.storage)?;
    if last_distributed + DISTRIBUTION_INTERVAL > env.block.time.nanos() / 1_000_000_000 {
        return Err(StdError::generic_err(
            "Cannot distribute mirror token before interval",
        ));
    }

    let config: Config = read_config(deps.storage)?;
    let time_elapsed = env.block.time.nanos() / 1_000_000_000 - config.genesis_time;
    let last_time_elapsed = last_distributed - config.genesis_time;
    let mut target_distribution_amount: Uint128 = Uint128::zero();
    for s in config.distribution_schedule.iter() {
        if s.0 > time_elapsed || s.1 < last_time_elapsed {
            continue;
        }

        // min(s.1, time_elapsed) - max(s.0, last_time_elapsed)
        let time_duration =
            std::cmp::min(s.1, time_elapsed) - std::cmp::max(s.0, last_time_elapsed);

        let time_slot = s.1 - s.0;
        let distribution_amount_per_sec: Decimal = Decimal::from_ratio(s.2, time_slot);
        target_distribution_amount +=
            distribution_amount_per_sec * Uint128::from(time_duration as u128);
    }

    let staking_contract = deps.api.addr_humanize(&config.staking_contract)?;
    let mirror_token = deps.api.addr_humanize(&config.mirror_token)?;

    let total_weight: u32 = read_total_weight(deps.storage)?;
    let mut distribution_amount: Uint128 = Uint128::zero();
    let weights: Vec<(CanonicalAddr, u32)> = read_all_weight(deps.storage)?;

    let rewards: Vec<(String, Uint128)> = weights
        .iter()
        .map(|w| {
            let amount = Uint128::from(
                target_distribution_amount.u128() * (w.1 as u128) / (total_weight as u128),
            );

            if amount.is_zero() {
                return Err(StdError::generic_err("cannot distribute zero amount"));
            }

            distribution_amount += amount;
            Ok((deps.api.addr_humanize(&w.0)?.to_string(), amount))
        })
        .filter(|m| m.is_ok())
        .collect::<StdResult<Vec<(String, Uint128)>>>()?;

    // store last distributed
    store_last_distributed(deps.storage, env.block.time.nanos() / 1_000_000_000)?;

    const SPLIT_UNIT: usize = 10;
    let rewards_vec: Vec<Vec<(String, Uint128)>> =
        rewards.chunks(SPLIT_UNIT).map(|v| v.to_vec()).collect();

    println!("{:?}", rewards);

    // mint token to self and try send minted tokens to staking contract
    Ok(Response::new()
        .add_messages(
            rewards_vec
                .into_iter()
                .map(|rewards| {
                    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: mirror_token.to_string(),
                        msg: to_binary(&Cw20ExecuteMsg::Send {
                            contract: staking_contract.to_string(),
                            amount: rewards.iter().map(|v| v.1.u128()).sum::<u128>().into(),
                            msg: to_binary(&StakingCw20HookMsg::DepositReward { rewards })?,
                        })?,
                        funds: vec![],
                    }))
                })
                .collect::<StdResult<Vec<CosmosMsg>>>()?,
        )
        .add_attributes(vec![
            attr("action", "distribute"),
            attr("distribution_amount", distribution_amount.to_string()),
        ]))
}

pub fn revoke_asset(
    deps: DepsMut,
    info: MessageInfo,
    asset_token: String,
    end_price: Decimal,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let asset_token_raw: CanonicalAddr = deps.api.addr_canonicalize(&asset_token)?;
    let oracle_feeder: Addr = deps.api.addr_humanize(&load_oracle_feeder(
        deps.as_ref(),
        deps.api.addr_humanize(&config.oracle_contract)?,
        &asset_token_raw,
    )?)?;
    let sender_raw = deps.api.addr_canonicalize(info.sender.as_str())?;

    // revoke asset can only be executed by the feeder or the owner (gov contract)
    if oracle_feeder != info.sender && config.owner != sender_raw {
        return Err(StdError::generic_err("unauthorized"));
    }

    remove_weight(deps.storage, &asset_token_raw);
    decrease_total_weight(deps.storage, NORMAL_TOKEN_WEIGHT)?;

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.mint_contract)?.to_string(),
            funds: vec![],
            msg: to_binary(&MintExecuteMsg::RegisterMigration {
                asset_token: asset_token.clone(),
                end_price,
            })?,
        }))
        .add_attributes(vec![
            attr("action", "revoke_asset"),
            attr("end_price", end_price.to_string()),
            attr("asset_token", asset_token.to_string()),
        ]))
}

pub fn migrate_asset(
    deps: DepsMut,
    info: MessageInfo,
    name: String,
    symbol: String,
    asset_token: String,
    end_price: Decimal,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let asset_token_raw: CanonicalAddr = deps.api.addr_canonicalize(&asset_token)?;
    let oracle_feeder: Addr = deps.api.addr_humanize(&load_oracle_feeder(
        deps.as_ref(),
        deps.api.addr_humanize(&config.oracle_contract)?,
        &asset_token_raw,
    )?)?;

    if oracle_feeder != info.sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    let weight = read_weight(deps.storage, &asset_token_raw)?;
    remove_weight(deps.storage, &asset_token_raw);
    decrease_total_weight(deps.storage, NORMAL_TOKEN_WEIGHT)?;

    let mint_contract = deps.api.addr_humanize(&config.mint_contract)?;
    let mint_config: (Decimal, Decimal, _) =
        load_mint_asset_config(deps.as_ref(), mint_contract.clone(), &asset_token_raw)?;

    store_params(
        deps.storage,
        &Params {
            auction_discount: mint_config.0,
            min_collateral_ratio: mint_config.1,
            weight: Some(weight),
            mint_period: None,
            min_collateral_ratio_after_ipo: None,
            pre_ipo_price: None,
        },
    )?;

    // store oracle in temp storage to use in reply callback
    store_tmp_oracle(deps.storage, &oracle_feeder)?;

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: mint_contract.to_string(),
            funds: vec![],
            msg: to_binary(&MintExecuteMsg::RegisterMigration {
                asset_token: asset_token.clone(),
                end_price,
            })?,
        }))
        .add_submessage(SubMsg {
            // create asset token
            msg: WasmMsg::Instantiate {
                admin: None,
                code_id: config.token_code_id,
                funds: vec![],
                label: "".to_string(),
                msg: to_binary(&TokenInstantiateMsg {
                    name: name.clone(),
                    symbol: symbol.to_string(),
                    decimals: 6u8,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: deps.api.addr_humanize(&config.mint_contract)?.to_string(),
                        cap: None,
                    }),
                })?,
            }
            .into(),
            gas_limit: None,
            id: 1,
            reply_on: ReplyOn::Success,
        })
        .add_attributes(vec![
            attr("action", "migration"),
            attr("end_price", end_price.to_string()),
            attr("asset_token", asset_token.to_string()),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::DistributionInfo {} => to_binary(&query_distribution_info(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.addr_humanize(&state.owner)?.to_string(),
        mirror_token: deps.api.addr_humanize(&state.mirror_token)?.to_string(),
        mint_contract: deps.api.addr_humanize(&state.mint_contract)?.to_string(),
        oracle_contract: deps.api.addr_humanize(&state.oracle_contract)?.to_string(),
        terraswap_factory: deps
            .api
            .addr_humanize(&state.terraswap_factory)?
            .to_string(),
        staking_contract: deps.api.addr_humanize(&state.staking_contract)?.to_string(),
        commission_collector: deps
            .api
            .addr_humanize(&state.commission_collector)?
            .to_string(),
        token_code_id: state.token_code_id,
        base_denom: state.base_denom,
        genesis_time: state.genesis_time,
        distribution_schedule: state.distribution_schedule,
    };

    Ok(resp)
}

pub fn query_distribution_info(deps: Deps) -> StdResult<DistributionInfoResponse> {
    let weights: Vec<(CanonicalAddr, u32)> = read_all_weight(deps.storage)?;
    let last_distributed = read_last_distributed(deps.storage)?;
    let resp = DistributionInfoResponse {
        last_distributed,
        weights: weights
            .iter()
            .map(|w| Ok((deps.api.addr_humanize(&w.0)?.to_string(), w.1)))
            .collect::<StdResult<Vec<(String, u32)>>>()?,
    };

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
