use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, Decimal, Env, Extern, HandleResponse, HandleResult,
    HumanAddr, InitResponse, MigrateResponse, MigrateResult, Querier, StdError, StdResult, Storage,
    Uint128,
};

use mirror_protocol::staking::{
    ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, MigrateMsg, PoolInfoResponse, QueryMsg,
};

use crate::migration::{migrate_config, migrate_pool_infos};
use crate::rewards::{adjust_premium, deposit_reward, query_reward_info, withdraw_reward};
use crate::staking::{bond, decrease_short_token, increase_short_token, unbond};
use crate::state::{read_config, read_pool_info, store_config, store_pool_info, Config, PoolInfo};

use cw20::Cw20ReceiveMsg;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&msg.owner)?,
            mirror_token: deps.api.canonical_address(&msg.mirror_token)?,
            mint_contract: deps.api.canonical_address(&msg.mint_contract)?,
            oracle_contract: deps.api.canonical_address(&msg.oracle_contract)?,
            terraswap_factory: deps.api.canonical_address(&msg.terraswap_factory)?,
            base_denom: msg.base_denom,
            premium_tolerance: msg.premium_tolerance,
            short_reward_weight: msg.short_reward_weight,
            premium_short_reward_weight: msg.premium_short_reward_weight,
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
        HandleMsg::UpdateConfig {
            owner,
            premium_tolerance,
            short_reward_weight,
            premium_short_reward_weight,
        } => update_config(
            deps,
            env,
            owner,
            premium_tolerance,
            short_reward_weight,
            premium_short_reward_weight,
        ),
        HandleMsg::RegisterAsset {
            asset_token,
            staking_token,
        } => register_asset(deps, env, asset_token, staking_token),
        HandleMsg::Unbond {
            asset_token,
            amount,
        } => unbond(deps, env.message.sender, asset_token, amount),
        HandleMsg::Withdraw { asset_token } => withdraw_reward(deps, env, asset_token),
        HandleMsg::AdjustPremium { asset_tokens } => adjust_premium(deps, asset_tokens),
        HandleMsg::IncreaseShortToken {
            staker_addr,
            asset_token,
            amount,
        } => increase_short_token(deps, env, staker_addr, asset_token, amount),
        HandleMsg::DecreaseShortToken {
            staker_addr,
            asset_token,
            amount,
        } => decrease_short_token(deps, env, staker_addr, asset_token, amount),
    }
}

pub fn receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> HandleResult {
    if let Some(msg) = cw20_msg.msg {
        let config: Config = read_config(&deps.storage)?;

        match from_binary(&msg)? {
            Cw20HookMsg::Bond { asset_token } => {
                let pool_info: PoolInfo =
                    read_pool_info(&deps.storage, &deps.api.canonical_address(&asset_token)?)?;

                // only staking token contract can execute this message
                if pool_info.staking_token != deps.api.canonical_address(&env.message.sender)? {
                    return Err(StdError::unauthorized());
                }

                bond(deps, env, cw20_msg.sender, asset_token, cw20_msg.amount)
            }
            Cw20HookMsg::DepositReward { rewards } => {
                // only reward token contract can execute this message
                if config.mirror_token != deps.api.canonical_address(&env.message.sender)? {
                    return Err(StdError::unauthorized());
                }

                let mut rewards_amount = Uint128::zero();
                for (_, amount) in rewards.iter() {
                    rewards_amount += *amount;
                }

                if rewards_amount != cw20_msg.amount {
                    return Err(StdError::generic_err("rewards amount miss matched"));
                }

                deposit_reward(deps, rewards, rewards_amount)
            }
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    premium_tolerance: Option<Decimal>,
    short_reward_weight: Option<Decimal>,
    premium_short_reward_weight: Option<Decimal>,
) -> StdResult<HandleResponse> {
    let mut config: Config = read_config(&deps.storage)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(premium_tolerance) = premium_tolerance {
        config.premium_tolerance = premium_tolerance;
    }

    if let Some(short_reward_weight) = short_reward_weight {
        config.short_reward_weight = short_reward_weight;
    }

    if let Some(premium_short_reward_weight) = premium_short_reward_weight {
        config.premium_short_reward_weight = premium_short_reward_weight;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

fn register_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    staking_token: HumanAddr,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;

    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if read_pool_info(&deps.storage, &asset_token_raw).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    store_pool_info(
        &mut deps.storage,
        &asset_token_raw,
        &PoolInfo {
            staking_token: deps.api.canonical_address(&staking_token)?,
            total_bond_amount: Uint128::zero(),
            total_short_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::zero(),
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "register_asset"),
            log("asset_token", asset_token.as_str()),
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
        QueryMsg::PoolInfo { asset_token } => to_binary(&query_pool_info(deps, asset_token)?),
        QueryMsg::RewardInfo {
            staker_addr,
            asset_token,
        } => to_binary(&query_reward_info(deps, staker_addr, asset_token)?),
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
        base_denom: state.base_denom,
        premium_tolerance: state.premium_tolerance,
        short_reward_weight: state.short_reward_weight,
        premium_short_reward_weight: state.premium_short_reward_weight,
    };

    Ok(resp)
}

pub fn query_pool_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_token: HumanAddr,
) -> StdResult<PoolInfoResponse> {
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw)?;
    Ok(PoolInfoResponse {
        asset_token,
        staking_token: deps.api.human_address(&pool_info.staking_token)?,
        total_bond_amount: pool_info.total_bond_amount,
        total_short_amount: pool_info.total_short_amount,
        reward_index: pool_info.reward_index,
        short_reward_index: pool_info.short_reward_index,
        pending_reward: pool_info.pending_reward,
        short_pending_reward: pool_info.short_pending_reward,
        premium_rate: pool_info.premium_rate,
    })
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: MigrateMsg,
) -> MigrateResult {
    migrate_config(
        &mut deps.storage,
        deps.api.canonical_address(&msg.mint_contract)?,
        deps.api.canonical_address(&msg.oracle_contract)?,
        deps.api.canonical_address(&msg.terraswap_factory)?,
        msg.base_denom,
        msg.premium_tolerance,
        msg.short_reward_weight,
        msg.premium_short_reward_weight,
    )?;

    migrate_pool_infos(&mut deps.storage)?;

    Ok(MigrateResponse::default())
}
