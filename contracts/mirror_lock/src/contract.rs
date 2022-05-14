use crate::state::{
    read_config, read_position_lock_info, remove_position_lock_info, store_config,
    store_position_lock_info, total_locked_funds_read, total_locked_funds_store, Config,
    PositionLockInfo,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Binary, CanonicalAddr, Deps, DepsMut, Empty, Env, MessageInfo, Response,
    StdError, StdResult, Uint128,
};
use mirror_protocol::lock::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, PositionLockInfoResponse, QueryMsg,
};
use terraswap::{
    asset::{Asset, AssetInfo},
    querier::query_balance,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        owner: deps.api.addr_canonicalize(&msg.owner)?,
        mint_contract: deps.api.addr_canonicalize(&msg.mint_contract)?,
        base_denom: msg.base_denom,
        lockup_period: msg.lockup_period,
    };

    store_config(deps.storage, &config)?;
    total_locked_funds_store(deps.storage).save(&Uint128::zero())?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig {
            owner,
            mint_contract,
            base_denom,
            lockup_period,
        } => update_config(deps, info, owner, mint_contract, base_denom, lockup_period),
        ExecuteMsg::LockPositionFundsHook {
            position_idx,
            receiver,
        } => lock_position_funds_hook(deps, env, info, position_idx, receiver),
        ExecuteMsg::UnlockPositionFunds { positions_idx } => {
            unlock_positions_funds(deps, env, info, positions_idx)
        }
        ExecuteMsg::ReleasePositionFunds { position_idx } => {
            release_position_funds(deps, env, info, position_idx)
        }
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    mint_contract: Option<String>,
    base_denom: Option<String>,
    lockup_period: Option<u64>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;

    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(mint_contract) = mint_contract {
        config.mint_contract = deps.api.addr_canonicalize(&mint_contract)?;
    }

    if let Some(base_denom) = base_denom {
        config.base_denom = base_denom;
    }

    if let Some(lockup_period) = lockup_period {
        config.lockup_period = lockup_period;
    }

    store_config(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

pub fn lock_position_funds_hook(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    position_idx: Uint128,
    receiver: String,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;
    if sender_addr_raw != config.mint_contract {
        return Err(StdError::generic_err("unauthorized"));
    }

    let current_balance: Uint128 = query_balance(
        &deps.querier,
        env.contract.address,
        config.base_denom.clone(),
    )?;
    let locked_funds: Uint128 = total_locked_funds_read(deps.storage).load()?;
    let position_locked_amount: Uint128 = current_balance.checked_sub(locked_funds)?;

    if position_locked_amount.is_zero() {
        // nothing to lock
        return Err(StdError::generic_err("Nothing to lock"));
    }

    let unlock_time: u64 = env.block.time.seconds() + config.lockup_period;
    let receiver_raw: CanonicalAddr = deps.api.addr_canonicalize(&receiver)?;
    let lock_info: PositionLockInfo =
        if let Ok(mut lock_info) = read_position_lock_info(deps.storage, position_idx) {
            // assert position receiver
            if receiver_raw != lock_info.receiver {
                // should never happen
                return Err(StdError::generic_err(
                    "Receiver address do not match with existing record",
                ));
            }
            // increase amount
            lock_info.locked_amount += position_locked_amount;
            lock_info.unlock_time = unlock_time;
            lock_info
        } else {
            PositionLockInfo {
                idx: position_idx,
                receiver: receiver_raw,
                locked_amount: position_locked_amount,
                unlock_time,
            }
        };

    store_position_lock_info(deps.storage, &lock_info)?;
    total_locked_funds_store(deps.storage).save(&current_balance)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "lock_position_funds_hook"),
        attr("position_idx", position_idx.to_string()),
        attr(
            "locked_amount",
            position_locked_amount.to_string() + &config.base_denom,
        ),
        attr(
            "total_locked_amount",
            lock_info.locked_amount.to_string() + &config.base_denom,
        ),
        attr("unlock_time", unlock_time.to_string()),
    ]))
}

pub fn unlock_positions_funds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    positions_idx: Vec<Uint128>,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    let unlockable_positions: Vec<PositionLockInfo> = positions_idx
        .iter()
        .filter_map(|position_idx| read_position_lock_info(deps.storage, *position_idx).ok())
        .filter(|lock_info| {
            lock_info.receiver == sender_addr_raw
                && env.block.time.seconds() >= lock_info.unlock_time
        })
        .collect();

    let mut unlocked_positions: Vec<Uint128> = vec![];
    let mut unlock_amount = Uint128::zero();
    for lock_info in unlockable_positions {
        if unlocked_positions.contains(&lock_info.idx) {
            return Err(StdError::generic_err("Duplicate position_idx"));
        }
        unlocked_positions.push(lock_info.idx);

        // remove lock record
        remove_position_lock_info(deps.storage, lock_info.idx);
        unlock_amount += lock_info.locked_amount
    }

    let unlock_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: config.base_denom.clone(),
        },
        amount: unlock_amount,
    };

    if unlock_asset.amount.is_zero() {
        return Err(StdError::generic_err(
            "There are no unlockable funds for the provided positions",
        ));
    }

    // decrease locked amount
    total_locked_funds_store(deps.storage).update(|current| {
        current
            .checked_sub(unlock_amount)
            .map_err(StdError::overflow)
    })?;

    let tax_amount: Uint128 = unlock_asset.compute_tax(&deps.querier)?;

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "unlock_shorting_funds"),
            attr("unlocked_amount", unlock_asset.to_string()),
            attr("tax_amount", tax_amount.to_string() + &config.base_denom),
        ])
        .add_message(unlock_asset.into_msg(&deps.querier, info.sender)?))
}

pub fn release_position_funds(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    position_idx: Uint128,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;
    // only mint contract can force claim all funds, without checking lock period
    if sender_addr_raw != config.mint_contract {
        return Err(StdError::generic_err("unauthorized"));
    }

    let lock_info: PositionLockInfo = match read_position_lock_info(deps.storage, position_idx) {
        Ok(lock_info) => lock_info,
        Err(_) => {
            return Ok(Response::default()); // user previously unlocked funds, graceful return
        }
    };

    // ingnore lock period, and unlock funds
    let unlock_amount: Uint128 = lock_info.locked_amount;
    let unlock_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: config.base_denom.clone(),
        },
        amount: unlock_amount,
    };

    // remove position info
    remove_position_lock_info(deps.storage, position_idx);

    // decrease locked amount
    total_locked_funds_store(deps.storage).update(|current| {
        current
            .checked_sub(unlock_amount)
            .map_err(StdError::overflow)
    })?;

    let tax_amount: Uint128 = unlock_asset.compute_tax(&deps.querier)?;

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "release_shorting_funds"),
            attr("position_idx", position_idx.to_string()),
            attr("unlocked_amount", unlock_asset.to_string()),
            attr("tax_amount", tax_amount.to_string() + &config.base_denom),
        ])
        .add_message(
            unlock_asset.into_msg(&deps.querier, deps.api.addr_humanize(&lock_info.receiver)?)?,
        ))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PositionLockInfo { position_idx } => {
            to_binary(&query_position_lock_info(deps, position_idx)?)
        }
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.addr_humanize(&state.owner)?.to_string(),
        mint_contract: deps.api.addr_humanize(&state.mint_contract)?.to_string(),
        base_denom: state.base_denom,
        lockup_period: state.lockup_period,
    };

    Ok(resp)
}

pub fn query_position_lock_info(
    deps: Deps,
    position_idx: Uint128,
) -> StdResult<PositionLockInfoResponse> {
    let lock_info: PositionLockInfo = read_position_lock_info(deps.storage, position_idx)?;

    let resp = PositionLockInfoResponse {
        idx: lock_info.idx,
        receiver: deps.api.addr_humanize(&lock_info.receiver)?.to_string(),
        locked_amount: lock_info.locked_amount,
        unlock_time: lock_info.unlock_time,
    };

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: Empty) -> StdResult<Response> {
    Ok(Response::default())
}
