use cosmwasm_std::{
    log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, InitResponse, MigrateResponse, MigrateResult, Querier, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};

use crate::querier::{load_mint_asset_config, load_oracle_feeder};
use crate::state::{
    decrease_total_weight, increase_total_weight, read_all_weight, read_config,
    read_last_distributed, read_params, read_total_weight, remove_params, remove_weight,
    store_config, store_last_distributed, store_params, store_total_weight, store_weight, Config,
};

use mirror_protocol::factory::{
    ConfigResponse, DistributionInfoResponse, HandleMsg, InitMsg, MigrateMsg, Params, QueryMsg,
};
use mirror_protocol::mint::HandleMsg as MintHandleMsg;
use mirror_protocol::oracle::HandleMsg as OracleHandleMsg;
use mirror_protocol::staking::Cw20HookMsg as StakingCw20HookMsg;
use mirror_protocol::staking::HandleMsg as StakingHandleMsg;

use cw20::{Cw20HandleMsg, MinterResponse};

use terraswap::asset::{AssetInfo, PairInfo};
use terraswap::factory::HandleMsg as TerraswapFactoryHandleMsg;
use terraswap::hook::InitHook;
use terraswap::querier::query_pair_info;
use terraswap::token::InitMsg as TokenInitMsg;

const MIRROR_TOKEN_WEIGHT: u32 = 3u32;
const NORMAL_TOKEN_WEIGHT: u32 = 1u32;
const DISTRIBUTION_INTERVAL: u64 = 60u64;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: CanonicalAddr::default(),
            mirror_token: CanonicalAddr::default(),
            mint_contract: CanonicalAddr::default(),
            oracle_contract: CanonicalAddr::default(),
            terraswap_factory: CanonicalAddr::default(),
            staking_contract: CanonicalAddr::default(),
            commission_collector: CanonicalAddr::default(),
            token_code_id: msg.token_code_id,
            base_denom: msg.base_denom,
            genesis_time: env.block.time,
            distribution_schedule: msg.distribution_schedule,
        },
    )?;

    store_total_weight(&mut deps.storage, 0u32)?;
    store_last_distributed(&mut deps.storage, env.block.time)?;
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::PostInitialize {
            owner,
            mirror_token,
            mint_contract,
            oracle_contract,
            terraswap_factory,
            staking_contract,
            commission_collector,
        } => try_post_initialize(
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
        HandleMsg::UpdateConfig {
            owner,
            token_code_id,
            distribution_schedule,
        } => try_update_config(deps, env, owner, token_code_id, distribution_schedule),
        HandleMsg::Whitelist {
            name,
            symbol,
            oracle_feeder,
            params,
        } => try_whitelist(deps, env, name, symbol, oracle_feeder, params),
        HandleMsg::TokenCreationHook { oracle_feeder } => {
            token_creation_hook(deps, env, oracle_feeder)
        }
        HandleMsg::TerraswapCreationHook { asset_token } => {
            terraswap_creation_hook(deps, env, asset_token)
        }
        HandleMsg::Distribute {} => try_distribute(deps, env),
        HandleMsg::PassCommand { contract_addr, msg } => {
            try_pass_command(deps, env, contract_addr, msg)
        }
        HandleMsg::MigrateAsset {
            name,
            symbol,
            from_token,
            end_price,
        } => try_migrate_asset(deps, env, name, symbol, from_token, end_price),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn try_post_initialize<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    owner: HumanAddr,
    mirror_token: HumanAddr,
    mint_contract: HumanAddr,
    oracle_contract: HumanAddr,
    terraswap_factory: HumanAddr,
    staking_contract: HumanAddr,
    commission_collector: HumanAddr,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if config.owner != CanonicalAddr::default() {
        return Err(StdError::unauthorized());
    }

    config.owner = deps.api.canonical_address(&owner)?;
    config.mirror_token = deps.api.canonical_address(&mirror_token)?;
    config.mint_contract = deps.api.canonical_address(&mint_contract)?;
    config.oracle_contract = deps.api.canonical_address(&oracle_contract)?;
    config.terraswap_factory = deps.api.canonical_address(&terraswap_factory)?;
    config.staking_contract = deps.api.canonical_address(&staking_contract)?;
    config.commission_collector = deps.api.canonical_address(&commission_collector)?;
    store_config(&mut deps.storage, &config)?;

    Ok(HandleResponse::default())
}

pub fn try_update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    token_code_id: Option<u64>,
    distribution_schedule: Option<Vec<(u64, u64, Uint128)>>,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(distribution_schedule) = distribution_schedule {
        config.distribution_schedule = distribution_schedule;
    }

    if let Some(token_code_id) = token_code_id {
        config.token_code_id = token_code_id;
    }

    store_config(&mut deps.storage, &config)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

// just for by passing command to other contract like update config
pub fn try_pass_command<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    contract_addr: HumanAddr,
    msg: Binary,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            send: vec![],
        })],
        log: vec![],
        data: None,
    })
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
pub fn try_whitelist<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    name: String,
    symbol: String,
    oracle_feeder: HumanAddr,
    params: Params,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if read_params(&deps.storage).is_ok() {
        return Err(StdError::generic_err("A whitelist process is in progress"));
    }

    store_params(&mut deps.storage, &params)?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Instantiate {
            code_id: config.token_code_id,
            send: vec![],
            label: None,
            msg: to_binary(&TokenInitMsg {
                name: name.clone(),
                symbol: symbol.to_string(),
                decimals: 6u8,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: deps.api.human_address(&config.mint_contract)?,
                    cap: None,
                }),
                init_hook: Some(InitHook {
                    contract_addr: env.contract.address,
                    msg: to_binary(&HandleMsg::TokenCreationHook { oracle_feeder })?,
                }),
            })?,
        })],
        log: vec![
            log("action", "whitelist"),
            log("symbol", symbol),
            log("name", name),
        ],
        data: None,
    })
}

/// TokenCreationHook
/// 1. Initialize distribution info
/// 2. Register asset to mint contract
/// 3. Register asset and oracle feeder to oracle contract
/// 4. Create terraswap pair through terraswap factory with `TerraswapCreationHook`
pub fn token_creation_hook<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    oracle_feeder: HumanAddr,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;

    // If the param is not exists, it means there is no whitelist process in progress
    let params: Params = match read_params(&deps.storage) {
        Ok(v) => v,
        Err(_) => return Err(StdError::generic_err("No whitelist process in progress")),
    };

    let asset_token = env.message.sender;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;

    // Increase total weight
    store_weight(&mut deps.storage, &asset_token_raw, NORMAL_TOKEN_WEIGHT)?;
    increase_total_weight(&mut deps.storage, NORMAL_TOKEN_WEIGHT)?;

    // Remove params == clear flag
    remove_params(&mut deps.storage);

    // Register asset to mint contract
    // Register asset to oracle contract
    // Create terraswap pair
    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.mint_contract)?,
                send: vec![],
                msg: to_binary(&MintHandleMsg::RegisterAsset {
                    asset_token: asset_token.clone(),
                    auction_discount: params.auction_discount,
                    min_collateral_ratio: params.min_collateral_ratio,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.oracle_contract)?,
                send: vec![],
                msg: to_binary(&OracleHandleMsg::RegisterAsset {
                    asset_token: asset_token.clone(),
                    feeder: oracle_feeder,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.terraswap_factory)?,
                send: vec![],
                msg: to_binary(&TerraswapFactoryHandleMsg::CreatePair {
                    asset_infos: [
                        AssetInfo::NativeToken {
                            denom: config.base_denom,
                        },
                        AssetInfo::Token {
                            contract_addr: asset_token.clone(),
                        },
                    ],
                    init_hook: Some(InitHook {
                        msg: to_binary(&HandleMsg::TerraswapCreationHook {
                            asset_token: asset_token.clone(),
                        })?,
                        contract_addr: env.contract.address,
                    }),
                })?,
            }),
        ],
        log: vec![log("asset_token_addr", asset_token.as_str())],
        data: None,
    })
}

/// TerraswapCreationHook
/// 1. Register asset and liquidity(LP) token to staking contract
pub fn terraswap_creation_hook<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
) -> HandleResult {
    // Now terraswap contract is already created,
    // and liquidty token also created
    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let sender_raw = deps.api.canonical_address(&env.message.sender)?;

    if config.mirror_token == asset_token_raw {
        store_weight(&mut deps.storage, &asset_token_raw, MIRROR_TOKEN_WEIGHT)?;
        increase_total_weight(&mut deps.storage, MIRROR_TOKEN_WEIGHT)?;
    } else if config.terraswap_factory != sender_raw {
        return Err(StdError::unauthorized());
    }

    let asset_infos = [
        AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        AssetInfo::Token {
            contract_addr: asset_token.clone(),
        },
    ];

    // Load terraswap pair info
    let pair_info: PairInfo = query_pair_info(
        &deps,
        &deps.api.human_address(&config.terraswap_factory)?,
        &asset_infos,
    )?;

    // Execute staking contract to register staking token of newly created asset
    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.staking_contract)?,
            send: vec![],
            msg: to_binary(&StakingHandleMsg::RegisterAsset {
                asset_token,
                staking_token: pair_info.liquidity_token,
            })?,
        })],
        log: vec![],
        data: None,
    })
}

/// Distribute
/// Anyone can execute distribute operation to distribute
/// mirror inflation rewards on the staking pool
pub fn try_distribute<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let last_distributed = read_last_distributed(&deps.storage)?;
    if last_distributed + DISTRIBUTION_INTERVAL > env.block.time {
        return Err(StdError::generic_err(
            "Cannot distribute mirror token before interval",
        ));
    }

    let config: Config = read_config(&deps.storage)?;
    let time_elapsed = env.block.time - config.genesis_time;
    let last_time_elapsed = last_distributed - config.genesis_time;
    let mut distributed_amount: Uint128 = Uint128::zero();
    for s in config.distribution_schedule.iter() {
        if s.0 > time_elapsed || s.1 < last_time_elapsed {
            continue;
        }

        // min(s.1, time_elpased) - max(s.0, last_time_elapsed)
        let time_duration =
            std::cmp::min(s.1, time_elapsed) - std::cmp::max(s.0, last_time_elapsed);

        let time_slot = s.1 - s.0;
        let distribution_amount_per_sec: Decimal = Decimal::from_ratio(s.2, time_slot);
        distributed_amount += distribution_amount_per_sec * Uint128(time_duration as u128);
    }

    let staking_contract = deps.api.human_address(&config.staking_contract)?;
    let mirror_token = deps.api.human_address(&config.mirror_token)?;

    let total_weight: u32 = read_total_weight(&deps.storage)?;
    let weights: Vec<(CanonicalAddr, u32)> = read_all_weight(&deps.storage)?;
    let messages: Vec<CosmosMsg> = weights
        .iter()
        .map(|w| {
            let amount =
                distributed_amount * Decimal::from_ratio(w.1 as u128, total_weight as u128);
            if amount.is_zero() {
                return Err(StdError::generic_err("cannot distribute zero amount"));
            }

            Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: mirror_token.clone(),
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: staking_contract.clone(),
                    amount,
                    msg: Some(to_binary(&StakingCw20HookMsg::DepositReward {
                        asset_token: deps.api.human_address(&w.0)?,
                    })?),
                })?,
                send: vec![],
            }))
        })
        .filter(|m| m.is_ok())
        .collect::<StdResult<Vec<CosmosMsg>>>()?;

    // store last distributed
    store_last_distributed(&mut deps.storage, env.block.time)?;

    // mint token to self and try send minted tokens to staking contract
    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "distribute"),
            log("distributed_amount", distributed_amount.to_string()),
        ],
        data: None,
    })
}

pub fn try_migrate_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    name: String,
    symbol: String,
    asset_token: HumanAddr,
    end_price: Decimal,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;
    let oracle_feeder: HumanAddr = deps.api.human_address(&load_oracle_feeder(
        &deps,
        &deps.api.human_address(&config.oracle_contract)?,
        &asset_token_raw,
    )?)?;

    if oracle_feeder != env.message.sender {
        return Err(StdError::unauthorized());
    }

    remove_weight(&mut deps.storage, &asset_token_raw);
    decrease_total_weight(&mut deps.storage, NORMAL_TOKEN_WEIGHT)?;

    let mint_contract = deps.api.human_address(&config.mint_contract)?;
    let mint_config: (Decimal, Decimal) =
        load_mint_asset_config(&deps, &mint_contract, &asset_token_raw)?;

    store_params(
        &mut deps.storage,
        &Params {
            auction_discount: mint_config.0,
            min_collateral_ratio: mint_config.1,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: mint_contract,
                send: vec![],
                msg: to_binary(&MintHandleMsg::RegisterMigration {
                    asset_token: asset_token.clone(),
                    end_price,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Instantiate {
                code_id: config.token_code_id,
                send: vec![],
                label: None,
                msg: to_binary(&TokenInitMsg {
                    name,
                    symbol,
                    decimals: 6u8,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: deps.api.human_address(&config.mint_contract)?,
                        cap: None,
                    }),
                    init_hook: Some(InitHook {
                        contract_addr: env.contract.address,
                        msg: to_binary(&HandleMsg::TokenCreationHook { oracle_feeder })?,
                    }),
                })?,
            }),
        ],
        log: vec![
            log("action", "migration"),
            log("end_price", end_price.to_string()),
            log("asset_token", asset_token.to_string()),
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
        QueryMsg::DistributionInfo {} => to_binary(&query_distribution_info(deps)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        mirror_token: deps.api.human_address(&state.mirror_token)?,
        mint_contract: deps.api.human_address(&state.mint_contract)?,
        oracle_contract: deps.api.human_address(&state.oracle_contract)?,
        terraswap_factory: deps.api.human_address(&state.terraswap_factory)?,
        staking_contract: deps.api.human_address(&state.staking_contract)?,
        commission_collector: deps.api.human_address(&state.commission_collector)?,
        token_code_id: state.token_code_id,
        base_denom: state.base_denom,
        genesis_time: state.genesis_time,
        distribution_schedule: state.distribution_schedule,
    };

    Ok(resp)
}

pub fn query_distribution_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<DistributionInfoResponse> {
    let weights: Vec<(CanonicalAddr, u32)> = read_all_weight(&deps.storage)?;
    let last_distributed = read_last_distributed(&deps.storage)?;
    let resp = DistributionInfoResponse {
        last_distributed,
        weights: weights
            .iter()
            .map(|w| Ok((deps.api.human_address(&w.0)?, w.1)))
            .collect::<StdResult<Vec<(HumanAddr, u32)>>>()?,
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
