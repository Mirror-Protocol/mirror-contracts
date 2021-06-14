use crate::state::{
    read_config, read_position_lock_info, remove_position_lock_info, store_config,
    store_position_lock_info, total_locked_funds_read, total_locked_funds_store, Config,
    PositionLockInfo,
};
use cosmwasm_std::{
    log, to_binary, Api, Binary, CanonicalAddr, Env, Extern, HandleResponse, HumanAddr,
    InitResponse, Querier, StdError, StdResult, Storage, Uint128,
};
use mirror_protocol::lock::{
    ConfigResponse, HandleMsg, InitMsg, PositionLockInfoResponse, QueryMsg,
};
use terraswap::{
    asset::{Asset, AssetInfo},
    querier::query_balance,
};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let config = Config {
        owner: deps.api.canonical_address(&msg.owner)?,
        mint_contract: deps.api.canonical_address(&msg.mint_contract)?,
        base_denom: msg.base_denom,
        lockup_period: msg.lockup_period,
    };

    store_config(&mut deps.storage, &config)?;
    total_locked_funds_store(&mut deps.storage).save(&Uint128::zero())?;
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::UpdateConfig {
            owner,
            mint_contract,
            base_denom,
            lockup_period,
        } => update_config(deps, env, owner, mint_contract, base_denom, lockup_period),
        HandleMsg::LockPositionFundsHook {
            position_idx,
            receiver,
        } => lock_position_funds_hook(deps, env, position_idx, receiver),
        HandleMsg::UnlockPositionFunds { positions_idx } => {
            unlock_positions_funds(deps, env, positions_idx)
        }
        HandleMsg::ReleasePositionFunds { position_idx } => {
            release_position_funds(deps, env, position_idx)
        }
    }
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    mint_contract: Option<HumanAddr>,
    base_denom: Option<String>,
    lockup_period: Option<u64>,
) -> StdResult<HandleResponse> {
    let mut config: Config = read_config(&deps.storage)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(mint_contract) = mint_contract {
        config.mint_contract = deps.api.canonical_address(&mint_contract)?;
    }

    if let Some(base_denom) = base_denom {
        config.base_denom = base_denom;
    }

    if let Some(lockup_period) = lockup_period {
        config.lockup_period = lockup_period;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

pub fn lock_position_funds_hook<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    position_idx: Uint128,
    receiver: HumanAddr,
) -> StdResult<HandleResponse> {
    let config: Config = read_config(&deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    if sender_addr_raw != config.mint_contract {
        return Err(StdError::unauthorized());
    }

    let current_balance: Uint128 =
        query_balance(deps, &env.contract.address, config.base_denom.clone())?;
    let locked_funds: Uint128 = total_locked_funds_read(&deps.storage).load()?;
    let position_locked_amount: Uint128 = (current_balance - locked_funds)?;

    if position_locked_amount.is_zero() {
        // nothing to lock
        return Err(StdError::generic_err("Nothing to lock"));
    }

    let unlock_time: u64 = env.block.time + config.lockup_period;
    let receiver_raw: CanonicalAddr = deps.api.canonical_address(&receiver)?;
    let lock_info: PositionLockInfo =
        if let Ok(mut lock_info) = read_position_lock_info(&deps.storage, position_idx) {
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
                unlock_time: unlock_time,
            }
        };

    store_position_lock_info(&mut deps.storage, &lock_info)?;
    total_locked_funds_store(&mut deps.storage).save(&current_balance)?;

    Ok(HandleResponse {
        log: vec![
            log("action", "lock_position_funds_hook"),
            log("position_idx", position_idx.to_string()),
            log(
                "locked_amount",
                position_locked_amount.to_string() + &config.base_denom,
            ),
            log(
                "total_locked_amount",
                lock_info.locked_amount.to_string() + &config.base_denom,
            ),
            log("unlock_time", unlock_time),
        ],
        messages: vec![],
        data: None,
    })
}

pub fn unlock_positions_funds<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    positions_idx: Vec<Uint128>,
) -> StdResult<HandleResponse> {
    let config: Config = read_config(&deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;

    let unlockable_positions: Vec<PositionLockInfo> = positions_idx
        .iter()
        .filter_map(|position_idx| read_position_lock_info(&deps.storage, *position_idx).ok())
        .filter(|lock_info| {
            lock_info.receiver == sender_addr_raw && env.block.time >= lock_info.unlock_time
        })
        .collect();

    let unlock_amount: u128 = unlockable_positions
        .iter()
        .map(|valid_lock_info| {
            // remove lock record
            remove_position_lock_info(&mut deps.storage, valid_lock_info.idx);
            valid_lock_info.locked_amount.u128()
        })
        .sum();

    let unlock_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: config.base_denom.clone(),
        },
        amount: Uint128(unlock_amount),
    };

    if unlock_asset.amount.is_zero() {
        return Err(StdError::generic_err(
            "There are no unlockable funds for the provided positions",
        ));
    }

    // decrease total locked amount
    total_locked_funds_store(&mut deps.storage).update(|current| {
        let new_total = (current - unlock_asset.amount)?;
        Ok(new_total)
    })?;

    let tax_amount: Uint128 = unlock_asset.compute_tax(&deps)?;

    Ok(HandleResponse {
        log: vec![
            log("action", "unlock_shorting_funds"),
            log("unlocked_amount", unlock_asset.to_string()),
            log("tax_amount", tax_amount.to_string() + &config.base_denom),
        ],
        messages: vec![unlock_asset.into_msg(&deps, env.contract.address, env.message.sender)?],
        data: None,
    })
}

pub fn release_position_funds<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    position_idx: Uint128,
) -> StdResult<HandleResponse> {
    let config: Config = read_config(&deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    // only mint contract can force claim all funds, without checking lock period
    if sender_addr_raw != config.mint_contract {
        return Err(StdError::unauthorized());
    }

    let lock_info: PositionLockInfo = match read_position_lock_info(&deps.storage, position_idx) {
        Ok(lock_info) => lock_info,
        Err(_) => {
            return Ok(HandleResponse::default()); // user previously unlocked funds, graceful return
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
    remove_position_lock_info(&mut deps.storage, position_idx);

    // decrease locked amount
    total_locked_funds_store(&mut deps.storage).update(|current| {
        let new_total = (current - unlock_amount)?;
        Ok(new_total)
    })?;

    let tax_amount: Uint128 = unlock_asset.compute_tax(&deps)?;

    Ok(HandleResponse {
        log: vec![
            log("action", "release_shorting_funds"),
            log("position_idx", position_idx.to_string()),
            log("unlocked_amount", unlock_asset.to_string()),
            log("tax_amount", tax_amount.to_string() + &config.base_denom),
        ],
        messages: vec![unlock_asset.into_msg(
            &deps,
            env.contract.address,
            deps.api.human_address(&lock_info.receiver)?,
        )?],
        data: None,
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PositionLockInfo { position_idx } => {
            to_binary(&query_position_lock_info(deps, position_idx)?)
        }
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        mint_contract: deps.api.human_address(&state.mint_contract)?,
        base_denom: state.base_denom,
        lockup_period: state.lockup_period,
    };

    Ok(resp)
}

pub fn query_position_lock_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    position_idx: Uint128,
) -> StdResult<PositionLockInfoResponse> {
    let lock_info: PositionLockInfo = read_position_lock_info(&deps.storage, position_idx)?;

    let resp = PositionLockInfoResponse {
        idx: lock_info.idx,
        receiver: deps.api.human_address(&lock_info.receiver)?,
        locked_amount: lock_info.locked_amount,
        unlock_time: lock_info.unlock_time,
    };

    Ok(resp)
}
