use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, Order, Querier, StdError, StdResult,
    Storage, Uint128, WasmMsg,
};

use crate::msg::{
    ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, PoolInfoResponse, QueryMsg,
    RewardInfoResponse, RewardInfoResponseItem,
};

use crate::state::{
    read_config, read_pool_info, rewards_read, rewards_store, store_config, store_pool_info,
    Config, PoolInfo, RewardInfo,
};

use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};

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
        HandleMsg::UpdateConfig { owner } => try_update_config(deps, env, owner),
        HandleMsg::RegisterAsset {
            asset_token,
            staking_token,
        } => try_register_asset(deps, env, asset_token, staking_token),
        HandleMsg::Unbond {
            asset_token,
            amount,
        } => try_unbond(deps, env, asset_token, amount),
        HandleMsg::Withdraw { asset_token } => try_withdraw(deps, env, asset_token),
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

                try_bond(deps, env, cw20_msg.sender, asset_token, cw20_msg.amount)
            }
            Cw20HookMsg::DepositReward { asset_token } => {
                // only reward token contract can execute this message
                if config.mirror_token != deps.api.canonical_address(&env.message.sender)? {
                    return Err(StdError::unauthorized());
                }

                try_deposit_reward(deps, env, asset_token, cw20_msg.amount)
            }
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

pub fn try_update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
) -> StdResult<HandleResponse> {
    let mut config: Config = read_config(&deps.storage)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

pub fn try_register_asset<S: Storage, A: Api, Q: Querier>(
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
            reward_index: Decimal::zero(),
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

pub fn try_bond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    sender_addr: HumanAddr,
    asset_token: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let sender_addr_raw: CanonicalAddr = deps.api.canonical_address(&sender_addr)?;
    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;
    let mut pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw)?;
    let mut reward_info: RewardInfo = rewards_read(&deps.storage, &sender_addr_raw)
        .load(asset_token_raw.as_slice())
        .unwrap_or_else(|_| RewardInfo {
            index: Decimal::zero(),
            bond_amount: Uint128::zero(),
            pending_reward: Uint128::zero(),
        });

    // Withdraw reward to pending reward; before changing share
    before_share_change(&pool_info, &mut reward_info)?;

    // Increase bond_amount
    increase_bond_amount(&mut pool_info, &mut reward_info, amount);

    rewards_store(&mut deps.storage, &sender_addr_raw)
        .save(&asset_token_raw.as_slice(), &reward_info)?;
    store_pool_info(&mut deps.storage, &asset_token_raw, &pool_info)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "bond"),
            log("asset_token", asset_token.as_str()),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
}

pub fn try_unbond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let sender_addr_raw: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;

    let mut pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw)?;
    let mut reward_info: RewardInfo =
        rewards_read(&deps.storage, &sender_addr_raw).load(asset_token_raw.as_slice())?;

    if reward_info.bond_amount < amount {
        return Err(StdError::generic_err("Cannot unbond more than bond amount"));
    }

    // Distribute reward to pending reward; before changing share
    before_share_change(&pool_info, &mut reward_info)?;

    // Decrease bond_amount
    decrease_bond_amount(&mut pool_info, &mut reward_info, amount)?;

    // Update rewards info
    if reward_info.pending_reward.is_zero() && reward_info.bond_amount.is_zero() {
        rewards_store(&mut deps.storage, &sender_addr_raw).remove(asset_token_raw.as_slice());
    } else {
        rewards_store(&mut deps.storage, &sender_addr_raw)
            .save(asset_token_raw.as_slice(), &reward_info)?;
    }

    // Update pool info
    store_pool_info(&mut deps.storage, &asset_token_raw, &pool_info)?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&pool_info.staking_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: env.message.sender,
                amount,
            })?,
            send: vec![],
        })],
        log: vec![log("action", "unbond"), log("amount", amount.to_string())],
        data: None,
    })
}

// deposit reward must be from reward token contract
pub fn try_deposit_reward<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    asset_token: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;
    let mut pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw)?;
    let reward_per_bond = Decimal::from_ratio(amount, pool_info.total_bond_amount);
    pool_info.reward_index = pool_info.reward_index + reward_per_bond;
    store_pool_info(&mut deps.storage, &asset_token_raw, &pool_info)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "deposit_reward"),
            log("asset_token", asset_token.as_str()),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
}

// withdraw all rewards or single reward depending on asset_token
pub fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: Option<HumanAddr>,
) -> HandleResult {
    let sender_addr_raw = deps.api.canonical_address(&env.message.sender)?;

    let config: Config = read_config(&deps.storage)?;
    let rewards_bucket = rewards_read(&deps.storage, &sender_addr_raw);

    // single reward withdraw
    let reward_pairs: Vec<(CanonicalAddr, RewardInfo)>;
    if let Some(asset_token) = asset_token {
        let asset_token_raw = deps.api.canonical_address(&asset_token)?;
        let reward_info = rewards_bucket.load(asset_token_raw.as_slice())?;
        reward_pairs = vec![(asset_token_raw, reward_info)];
    } else {
        reward_pairs = rewards_bucket
            .range(None, None, Order::Ascending)
            .map(|item| {
                let (k, v) = item?;
                Ok((CanonicalAddr::from(k), v))
            })
            .collect::<StdResult<Vec<(CanonicalAddr, RewardInfo)>>>()?;
    }

    let mut amount: Uint128 = Uint128::zero();
    for reward_pair in reward_pairs {
        let (asset_token_raw, mut reward_info) = reward_pair;
        let pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw)?;

        // Withdraw reward to pending reward
        before_share_change(&pool_info, &mut reward_info)?;

        amount += reward_info.pending_reward;
        reward_info.pending_reward = Uint128::zero();

        // Update rewards info
        if reward_info.pending_reward.is_zero() && reward_info.bond_amount.is_zero() {
            rewards_store(&mut deps.storage, &sender_addr_raw).remove(asset_token_raw.as_slice());
        } else {
            rewards_store(&mut deps.storage, &sender_addr_raw)
                .save(asset_token_raw.as_slice(), &reward_info)?;
        }
    }

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.mirror_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: env.message.sender,
                amount,
            })?,
            send: vec![],
        })],
        log: vec![log("action", "withdraw"), log("amount", amount.to_string())],
        data: None,
    })
}

fn increase_bond_amount(pool_info: &mut PoolInfo, reward_info: &mut RewardInfo, amount: Uint128) {
    pool_info.total_bond_amount += amount;
    reward_info.bond_amount += amount;
}

fn decrease_bond_amount(
    pool_info: &mut PoolInfo,
    reward_info: &mut RewardInfo,
    amount: Uint128,
) -> StdResult<()> {
    pool_info.total_bond_amount = (pool_info.total_bond_amount - amount)?;
    reward_info.bond_amount = (reward_info.bond_amount - amount)?;
    Ok(())
}

// withdraw reward to pending reward
fn before_share_change(pool_info: &PoolInfo, reward_info: &mut RewardInfo) -> StdResult<()> {
    let pending_reward = (reward_info.bond_amount * pool_info.reward_index
        - reward_info.bond_amount * reward_info.index)?;

    reward_info.index = pool_info.reward_index;
    reward_info.pending_reward += pending_reward;
    Ok(())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PoolInfo { asset_token } => to_binary(&query_pool_info(deps, asset_token)?),
        QueryMsg::RewardInfo {
            asset_token,
            staker,
        } => to_binary(&query_reward_info(deps, asset_token, staker)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        mirror_token: deps.api.human_address(&state.mirror_token)?,
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
        reward_index: pool_info.reward_index,
    })
}

pub fn query_reward_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_token: Option<HumanAddr>,
    staker: HumanAddr,
) -> StdResult<RewardInfoResponse> {
    let staker_raw = deps.api.canonical_address(&staker)?;

    let rewards_bucket = rewards_read(&deps.storage, &staker_raw);
    let reward_infos: Vec<RewardInfoResponseItem>;
    if let Some(asset_token) = asset_token {
        let asset_token_raw = deps.api.canonical_address(&asset_token)?;
        let reward_info = rewards_bucket.load(asset_token_raw.as_slice())?;
        reward_infos = vec![RewardInfoResponseItem {
            asset_token,
            index: reward_info.index,
            bond_amount: reward_info.bond_amount,
            pending_reward: reward_info.pending_reward,
        }];
    } else {
        reward_infos = rewards_bucket
            .range(None, None, Order::Ascending)
            .map(|item| {
                let (k, v) = item?;

                Ok(RewardInfoResponseItem {
                    asset_token: deps.api.human_address(&CanonicalAddr::from(k))?,
                    index: v.index,
                    bond_amount: v.bond_amount,
                    pending_reward: v.pending_reward,
                })
            })
            .collect::<StdResult<Vec<RewardInfoResponseItem>>>()?;
    }

    Ok(RewardInfoResponse {
        staker,
        reward_infos,
    })
}
