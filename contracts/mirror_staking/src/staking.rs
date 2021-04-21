use cosmwasm_std::{
    log, to_binary, Api, CanonicalAddr, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::rewards::before_share_change;
use crate::state::{
    read_config, read_pool_info, rewards_read, rewards_store, store_pool_info, Config, PoolInfo,
    RewardInfo,
};

use cw20::Cw20HandleMsg;

pub fn bond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    staker_addr: HumanAddr,
    asset_token: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let staker_addr_raw: CanonicalAddr = deps.api.canonical_address(&staker_addr)?;
    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;
    _increase_bond_amount(
        &mut deps.storage,
        &staker_addr_raw,
        &asset_token_raw,
        amount,
        false,
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "bond"),
            log("staker_addr", staker_addr.as_str()),
            log("asset_token", asset_token.as_str()),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
}

pub fn unbond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    staker_addr: HumanAddr,
    asset_token: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let staker_addr_raw: CanonicalAddr = deps.api.canonical_address(&staker_addr)?;
    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;
    let staking_token: CanonicalAddr = _decrease_bond_amount(
        &mut deps.storage,
        &staker_addr_raw,
        &asset_token_raw,
        amount,
        false,
    )?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&staking_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: staker_addr.clone(),
                amount,
            })?,
            send: vec![],
        })],
        log: vec![
            log("action", "unbond"),
            log("staker_addr", staker_addr.as_str()),
            log("asset_token", asset_token.as_str()),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
}

// only mint contract can execute the operation
pub fn increase_short_token<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    staker_addr: HumanAddr,
    asset_token: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if deps.api.canonical_address(&env.message.sender)? != config.mint_contract {
        return Err(StdError::unauthorized());
    }

    let staker_addr_raw: CanonicalAddr = deps.api.canonical_address(&staker_addr)?;
    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;

    _increase_bond_amount(
        &mut deps.storage,
        &staker_addr_raw,
        &asset_token_raw,
        amount,
        true,
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "increase_short_token"),
            log("staker_addr", staker_addr.as_str()),
            log("asset_token", asset_token.as_str()),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
}

// only mint contract can execute the operation
pub fn decrease_short_token<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    staker_addr: HumanAddr,
    asset_token: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if deps.api.canonical_address(&env.message.sender)? != config.mint_contract {
        return Err(StdError::unauthorized());
    }

    let staker_addr_raw: CanonicalAddr = deps.api.canonical_address(&staker_addr)?;
    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;

    // not used
    let _ = _decrease_bond_amount(
        &mut deps.storage,
        &staker_addr_raw,
        &asset_token_raw,
        amount,
        true,
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "decrease_short_token"),
            log("staker_addr", staker_addr.as_str()),
            log("asset_token", asset_token.as_str()),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
}

fn _increase_bond_amount<S: Storage>(
    storage: &mut S,
    staker_addr: &CanonicalAddr,
    asset_token: &CanonicalAddr,
    amount: Uint128,
    is_short: bool,
) -> StdResult<()> {
    let mut pool_info: PoolInfo = read_pool_info(storage, &asset_token)?;
    let mut reward_info: RewardInfo = rewards_read(storage, &staker_addr, is_short)
        .load(asset_token.as_slice())
        .unwrap_or_else(|_| RewardInfo {
            index: Decimal::zero(),
            bond_amount: Uint128::zero(),
            pending_reward: Uint128::zero(),
        });

    // Withdraw reward to pending reward; before changing share
    before_share_change(&pool_info, &mut reward_info, is_short)?;

    // Increase total short or bond amount
    if is_short {
        pool_info.total_short_amount += amount;
    } else {
        pool_info.total_bond_amount += amount;
    }

    reward_info.bond_amount += amount;
    rewards_store(storage, &staker_addr, is_short).save(&asset_token.as_slice(), &reward_info)?;
    store_pool_info(storage, &asset_token, &pool_info)?;

    Ok(())
}

fn _decrease_bond_amount<S: Storage>(
    storage: &mut S,
    staker_addr: &CanonicalAddr,
    asset_token: &CanonicalAddr,
    amount: Uint128,
    is_short: bool,
) -> StdResult<CanonicalAddr> {
    let mut pool_info: PoolInfo = read_pool_info(storage, &asset_token)?;
    let mut reward_info: RewardInfo =
        rewards_read(storage, &staker_addr, is_short).load(asset_token.as_slice())?;

    if reward_info.bond_amount < amount {
        return Err(StdError::generic_err("Cannot unbond more than bond amount"));
    }

    // Distribute reward to pending reward; before changing share
    before_share_change(&pool_info, &mut reward_info, is_short)?;

    // Decrease total short or bond amount
    if is_short {
        pool_info.total_short_amount = (pool_info.total_short_amount - amount)?;
    } else {
        pool_info.total_bond_amount = (pool_info.total_bond_amount - amount)?;
    }

    reward_info.bond_amount = (reward_info.bond_amount - amount)?;

    // Update rewards info
    if reward_info.pending_reward.is_zero() && reward_info.bond_amount.is_zero() {
        rewards_store(storage, &staker_addr, is_short).remove(asset_token.as_slice());
    } else {
        rewards_store(storage, &staker_addr, is_short)
            .save(asset_token.as_slice(), &reward_info)?;
    }

    // Update pool info
    store_pool_info(storage, &asset_token, &pool_info)?;

    Ok(pool_info.staking_token)
}
