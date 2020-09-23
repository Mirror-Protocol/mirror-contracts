use cosmwasm_std::{
    log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, InitResponse, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::msg::{
    ConfigResponse, DistributionInfoResponse, HandleMsg, InitMsg, QueryMsg, StakingCw20HookMsg,
};
use crate::register_msgs::*;
use crate::state::{
    read_config, read_distribution_info, read_params, remove_params, store_config,
    store_distribution_info, store_params, Config, DistributionInfo, Params,
};

use cw20::{Cw20HandleMsg, MinterResponse};
use uniswap::{load_liquidity_token, load_pair_contract, AssetInfo, InitHook, TokenInitMsg};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: CanonicalAddr::default(),
            mirror_token: CanonicalAddr::default(),
            mint_contract: CanonicalAddr::default(),
            oracle_contract: CanonicalAddr::default(),
            uniswap_factory: CanonicalAddr::default(),
            staking_contract: CanonicalAddr::default(),
            commission_collector: CanonicalAddr::default(),
            mint_per_block: msg.mint_per_block,
            token_code_id: msg.token_code_id,
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
        HandleMsg::PostInitialize {
            owner,
            mirror_token,
            mint_contract,
            oracle_contract,
            uniswap_factory,
            staking_contract,
            commission_collector,
        } => try_post_initialize(
            deps,
            env,
            owner,
            mirror_token,
            mint_contract,
            oracle_contract,
            uniswap_factory,
            staking_contract,
            commission_collector,
        ),
        HandleMsg::UpdateConfig {
            owner,
            mint_per_block,
            token_code_id,
        } => try_update_config(deps, env, owner, mint_per_block, token_code_id),
        HandleMsg::UpdateWeight {
            asset_token,
            weight,
        } => try_update_weight(deps, env, asset_token, weight),
        HandleMsg::Whitelist {
            name,
            symbol,
            oracle_feeder,
            params,
        } => try_whitelist(deps, env, name, symbol, oracle_feeder, params),
        HandleMsg::TokenCreationHook { oracle_feeder } => {
            token_creation_hook(deps, env, oracle_feeder)
        }
        HandleMsg::UniswapCreationHook { asset_token } => {
            uniswap_creation_hook(deps, env, asset_token)
        }
        HandleMsg::Mint { asset_token } => try_mint(deps, env, asset_token),
        HandleMsg::PassCommand { contract_addr, msg } => {
            try_pass_command(deps, env, contract_addr, msg)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn try_post_initialize<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: HumanAddr,
    mirror_token: HumanAddr,
    mint_contract: HumanAddr,
    oracle_contract: HumanAddr,
    uniswap_factory: HumanAddr,
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
    config.uniswap_factory = deps.api.canonical_address(&uniswap_factory)?;
    config.staking_contract = deps.api.canonical_address(&staking_contract)?;
    config.commission_collector = deps.api.canonical_address(&commission_collector)?;
    store_config(&mut deps.storage, &config)?;

    // for the mirror token, we skip token creation hook
    // just calling uniswap creation hook is enough
    // mirror staking pool rewards x2
    store_distribution_info(
        &mut deps.storage,
        &config.mirror_token,
        &DistributionInfo {
            weight: Decimal::percent(200),
            last_height: env.block.height,
        },
    )?;

    Ok(HandleResponse::default())
}

pub fn try_update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    mint_per_block: Option<Uint128>,
    token_code_id: Option<u64>,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(mint_per_block) = mint_per_block {
        config.mint_per_block = mint_per_block;
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

pub fn try_update_weight<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    weight: Decimal,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let mut distribution_info = read_distribution_info(&deps.storage, &asset_token_raw)?;
    distribution_info.weight = weight;

    store_distribution_info(&mut deps.storage, &asset_token_raw, &distribution_info)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "update_weight"),
            log("asset_token", asset_token.as_str()),
            log("weight", &weight.to_string()),
        ],
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
///    2-4. Create uniswap pair through uniswap factory
/// 3. Call `UniswapCreationHook`
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
/// 4. Create uniswap pair through uniswap factory with `UniswapCreationHook`
pub fn token_creation_hook<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    oracle_feeder: HumanAddr,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    // If the param is not exists, it means there is no whitelist process in progress
    let params: Params = match read_params(&deps.storage) {
        Ok(v) => v,
        Err(_) => {
            return Err(StdError::generic_err(
                "There is no whitelist process in progress",
            ))
        }
    };

    let asset_token = env.message.sender;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;

    store_distribution_info(
        &mut deps.storage,
        &asset_token_raw,
        &DistributionInfo {
            weight: params.weight,
            last_height: env.block.height,
        },
    )?;

    // Remove params == clear flag
    remove_params(&mut deps.storage);

    // Register asset to mint contract
    // Register asset to oracle contract
    // Create uniswap pair
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
                    asset_info: AssetInfo::Token {
                        contract_addr: asset_token.clone(),
                    },
                    feeder: oracle_feeder,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.uniswap_factory)?,
                send: vec![],
                msg: to_binary(&UniswapHandleMsg::CreatePair {
                    pair_owner: env.contract.address.clone(),
                    commission_collector: deps.api.human_address(&config.commission_collector)?,
                    lp_commission: params.lp_commission,
                    owner_commission: params.owner_commission,
                    asset_infos: [
                        AssetInfo::NativeToken {
                            denom: config.base_denom,
                        },
                        AssetInfo::Token {
                            contract_addr: asset_token.clone(),
                        },
                    ],
                    init_hook: Some(InitHook {
                        msg: to_binary(&HandleMsg::UniswapCreationHook {
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

/// UniswapCreationHook
/// 1. Register asset and liquidity(LP) token to staking contract
pub fn uniswap_creation_hook<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
) -> HandleResult {
    // Now uniswap contract is already created,
    // and liquidty token also created
    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let sender_raw = deps.api.canonical_address(&env.message.sender)?;

    if config.mirror_token != asset_token_raw && config.uniswap_factory != sender_raw {
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

    // Load uniswap pair contract
    let uniswap_contract: HumanAddr = load_pair_contract(
        &deps,
        &deps.api.human_address(&config.uniswap_factory)?,
        &asset_infos,
    )?;

    // Load uniswap pair LP token
    let liquidity_token: HumanAddr = load_liquidity_token(&deps, &uniswap_contract)?;

    // Execute staking contract to register staking token of newly created asset
    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.staking_contract)?,
            send: vec![],
            msg: to_binary(&StakingHandleMsg::RegisterAsset {
                asset_token,
                staking_token: liquidity_token,
            })?,
        })],
        log: vec![],
        data: None,
    })
}

/// Mint
/// Anyone can execute mint operation to distribute
/// mirror inflation rewards on the staking pool
pub fn try_mint<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
) -> HandleResult {
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;

    let config: Config = read_config(&deps.storage)?;
    let distribution_info: DistributionInfo =
        read_distribution_info(&deps.storage, &asset_token_raw)?;

    // mint_amount = weight * mint_per_block * (height - last_height)
    let mint_amount = (config.mint_per_block * distribution_info.weight)
        .multiply_ratio(env.block.height - distribution_info.last_height, 1u64);

    store_distribution_info(
        &mut deps.storage,
        &asset_token_raw,
        &DistributionInfo {
            last_height: env.block.height,
            ..distribution_info
        },
    )?;

    // mint token to self and try send minted tokens to staking contract
    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.mirror_token)?,
                msg: to_binary(&Cw20HandleMsg::Mint {
                    recipient: env.contract.address,
                    amount: mint_amount,
                })?,
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.mirror_token)?,
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: deps.api.human_address(&config.staking_contract)?,
                    amount: mint_amount,
                    msg: Some(to_binary(&StakingCw20HookMsg::DepositReward {
                        asset_token: asset_token.clone(),
                    })?),
                })?,
                send: vec![],
            }),
        ],
        log: vec![
            log("action", "mint"),
            log("asset_token", asset_token.as_str()),
            log("mint_amount", mint_amount.to_string()),
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
        QueryMsg::DistributionInfo { asset_token } => {
            to_binary(&query_distribution_info(deps, asset_token)?)
        }
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
        uniswap_factory: deps.api.human_address(&state.uniswap_factory)?,
        staking_contract: deps.api.human_address(&state.staking_contract)?,
        commission_collector: deps.api.human_address(&state.commission_collector)?,
        mint_per_block: state.mint_per_block,
        token_code_id: state.token_code_id,
        base_denom: state.base_denom,
    };

    Ok(resp)
}

pub fn query_distribution_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_token: HumanAddr,
) -> StdResult<DistributionInfoResponse> {
    let state = read_distribution_info(&deps.storage, &deps.api.canonical_address(&asset_token)?)?;
    let resp = DistributionInfoResponse {
        last_height: state.last_height,
        weight: state.weight,
    };

    Ok(resp)
}
