use cosmwasm_std::{
    log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, InitResponse, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::msg::{
    ConfigResponse, DistributionInfoResponse, HandleMsg, InitMsg, QueryMsg, StakingCw20HookMsg,
    WhitelistInfoResponse,
};
use crate::state::{
    read_config, read_distribution_info, read_whitelist_info, store_config,
    store_distribution_info, store_whitelist_info, Config, DistributionInfo, WhitelistInfo,
};

use cw20::Cw20HandleMsg;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&env.message.sender)?,
            mirror_token: CanonicalAddr::default(),
            mint_per_block: msg.mint_per_block,
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
        HandleMsg::PostInitialize { mirror_token } => try_post_initialize(deps, env, mirror_token),
        HandleMsg::UpdateConfig {
            owner,
            mint_per_block,
        } => try_update_config(deps, env, owner, mint_per_block),
        HandleMsg::UpdateWeight { symbol, weight } => try_update_weight(deps, env, symbol, weight),
        HandleMsg::Whitelist {
            symbol,
            weight,
            token_contract,
            mint_contract,
            market_contract,
            oracle_contract,
            staking_contract,
        } => try_whitelist(
            deps,
            env,
            symbol,
            weight,
            token_contract,
            mint_contract,
            market_contract,
            oracle_contract,
            staking_contract,
        ),
        HandleMsg::Mint { symbol } => try_mint(deps, env, symbol),
    }
}

pub fn try_post_initialize<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    mirror_token: HumanAddr,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)?
        || config.mirror_token != CanonicalAddr::default()
    {
        return Err(StdError::unauthorized());
    }

    config.mirror_token = deps.api.canonical_address(&mirror_token)?;
    store_config(&mut deps.storage, &config)?;

    Ok(HandleResponse::default())
}

pub fn try_update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    mint_per_block: Option<Uint128>,
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

    store_config(&mut deps.storage, &config)?;

    Ok(HandleResponse::default())
}

pub fn try_update_weight<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    symbol: String,
    weight: Decimal,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    let mut distribution_info = read_distribution_info(&deps.storage, symbol.to_string())?;
    distribution_info.weight = weight;

    store_distribution_info(&mut deps.storage, symbol.to_string(), &distribution_info)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "update_weight"),
            log("symbol", &symbol),
            log("weight", &weight.to_string()),
        ],
        data: None,
    })
}

// only owner can exeucte whitelist
#[allow(clippy::too_many_arguments)]
pub fn try_whitelist<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    symbol: String,
    weight: Decimal,
    token_contract: HumanAddr,
    mint_contract: HumanAddr,
    market_contract: HumanAddr,
    oracle_contract: HumanAddr,
    staking_contract: HumanAddr,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if read_whitelist_info(&deps.storage, symbol.clone()).is_ok() {
        return Err(StdError::generic_err(format!(
            "whitelist {} already exists",
            symbol
        )));
    }

    store_whitelist_info(
        &mut deps.storage,
        symbol.clone(),
        &WhitelistInfo {
            token_contract: deps.api.canonical_address(&token_contract)?,
            mint_contract: deps.api.canonical_address(&mint_contract)?,
            market_contract: deps.api.canonical_address(&market_contract)?,
            oracle_contract: deps.api.canonical_address(&oracle_contract)?,
            staking_contract: deps.api.canonical_address(&staking_contract)?,
        },
    )?;

    store_distribution_info(
        &mut deps.storage,
        symbol.clone(),
        &DistributionInfo {
            weight,
            last_height: env.block.height,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "whitelist"),
            log("symbol", &symbol),
            log("weight", &weight.to_string()),
        ],
        data: None,
    })
}

// Anyone can execute mint function to receive rewards
pub fn try_mint<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    symbol: String,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let whitelist_info: WhitelistInfo = read_whitelist_info(&deps.storage, symbol.to_string())?;
    let distribution_info: DistributionInfo =
        read_distribution_info(&deps.storage, symbol.to_string())?;

    // mint_amount = weight * mint_per_block * (height - last_height)
    let mint_amount = (config.mint_per_block * distribution_info.weight)
        .multiply_ratio(env.block.height - distribution_info.last_height, 1u64);

    store_distribution_info(
        &mut deps.storage,
        symbol.to_string(),
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
                    contract: deps.api.human_address(&whitelist_info.staking_contract)?,
                    amount: mint_amount,
                    msg: Some(to_binary(&StakingCw20HookMsg::DepositReward {})?),
                })?,
                send: vec![],
            }),
        ],
        log: vec![
            log("action", "mint"),
            log("symbol", symbol),
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
        QueryMsg::WhitelistInfo { symbol } => to_binary(&query_whitelist_info(deps, symbol)?),
        QueryMsg::DistributionInfo { symbol } => to_binary(&query_distribution_info(deps, symbol)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        mirror_token: deps.api.human_address(&state.mirror_token)?,
        mint_per_block: state.mint_per_block,
    };

    Ok(resp)
}

pub fn query_whitelist_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    symbol: String,
) -> StdResult<WhitelistInfoResponse> {
    let state = read_whitelist_info(&deps.storage, symbol)?;
    let resp = WhitelistInfoResponse {
        mint_contract: deps.api.human_address(&state.mint_contract)?,
        market_contract: deps.api.human_address(&state.market_contract)?,
        staking_contract: deps.api.human_address(&state.staking_contract)?,
        token_contract: deps.api.human_address(&state.token_contract)?,
        oracle_contract: deps.api.human_address(&state.oracle_contract)?,
    };

    Ok(resp)
}

pub fn query_distribution_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    symbol: String,
) -> StdResult<DistributionInfoResponse> {
    let state = read_distribution_info(&deps.storage, symbol)?;
    let resp = DistributionInfoResponse {
        last_height: state.last_height,
        weight: state.weight,
    };

    Ok(resp)
}
