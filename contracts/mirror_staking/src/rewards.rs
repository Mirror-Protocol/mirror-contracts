use cosmwasm_std::{
    log, to_binary, Api, CanonicalAddr, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, Order, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::querier::compute_premium_rate;
use crate::state::{
    read_config, read_pool_info, rewards_read, rewards_store, store_pool_info, Config, PoolInfo,
    RewardInfo,
};
use mirror_protocol::staking::{RewardInfoResponse, RewardInfoResponseItem};

use cw20::Cw20HandleMsg;

pub fn adjust_premium<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_tokens: Vec<HumanAddr>,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let oracle_contract = deps.api.human_address(&config.oracle_contract)?;
    let terraswap_factory = deps.api.human_address(&config.terraswap_factory)?;
    for asset_token in asset_tokens.iter() {
        let premium_rate = compute_premium_rate(
            deps,
            &oracle_contract,
            &terraswap_factory,
            asset_token,
            config.base_denom.to_string(),
        )?;

        let asset_token_raw = deps.api.canonical_address(&asset_token)?;
        let pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw)?;
        if env.block.time < pool_info.premium_updated_time + config.premium_min_update_interval {
            return Err(StdError::generic_err(
                "cannot adjust premium before premium_min_update_interval passed",
            ));
        }

        store_pool_info(
            &mut deps.storage,
            &asset_token_raw,
            &PoolInfo {
                premium_rate,
                premium_updated_time: env.block.time,
                ..pool_info
            },
        )?;
    }

    Ok(HandleResponse {
        log: vec![log("action", "premium_adjustment")],
        ..HandleResponse::default()
    })
}

// deposit_reward must be from reward token contract
pub fn deposit_reward<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    rewards: Vec<(HumanAddr, Uint128)>,
    rewards_amount: Uint128,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    for (asset_token, amount) in rewards.iter() {
        let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;
        let mut pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw)?;

        // Depends on the last premium, apply different short_reward_weight
        let short_reward_weight = if pool_info.premium_rate > config.premium_tolerance {
            config.premium_short_reward_weight
        } else {
            config.short_reward_weight
        };

        let total_reward = *amount;
        let mut short_reward = total_reward * short_reward_weight;
        let mut normal_reward = (total_reward - short_reward).unwrap();

        if pool_info.total_bond_amount.is_zero() {
            pool_info.pending_reward += normal_reward;
        } else {
            normal_reward += pool_info.pending_reward;
            let normal_reward_per_bond =
                Decimal::from_ratio(normal_reward, pool_info.total_bond_amount);
            pool_info.reward_index = pool_info.reward_index + normal_reward_per_bond;
            pool_info.pending_reward = Uint128::zero();
        }

        if pool_info.total_short_amount.is_zero() {
            pool_info.short_pending_reward += short_reward;
        } else {
            short_reward += pool_info.short_pending_reward;
            let short_reward_per_bond =
                Decimal::from_ratio(short_reward, pool_info.total_short_amount);
            pool_info.short_reward_index = pool_info.short_reward_index + short_reward_per_bond;
            pool_info.short_pending_reward = Uint128::zero();
        }

        store_pool_info(&mut deps.storage, &asset_token_raw, &pool_info)?;
    }

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "deposit_reward"),
            log("rewards_amount", rewards_amount.to_string()),
        ],
        data: None,
    })
}

// withdraw all rewards or single reward depending on asset_token
pub fn withdraw_reward<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: Option<HumanAddr>,
) -> HandleResult {
    let staker_addr = deps.api.canonical_address(&env.message.sender)?;
    let asset_token = asset_token.map(|a| deps.api.canonical_address(&a).unwrap());
    let normal_reward = _withdraw_reward(&mut deps.storage, &staker_addr, &asset_token, false)?;
    let short_reward = _withdraw_reward(&mut deps.storage, &staker_addr, &asset_token, true)?;

    let amount = normal_reward + short_reward;
    let config: Config = read_config(&deps.storage)?;
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

fn _withdraw_reward<S: Storage>(
    storage: &mut S,
    staker_addr: &CanonicalAddr,
    asset_token: &Option<CanonicalAddr>,
    is_short: bool,
) -> StdResult<Uint128> {
    let rewards_bucket = rewards_read(storage, &staker_addr, is_short);

    // single reward withdraw
    let reward_pairs: Vec<(CanonicalAddr, RewardInfo)>;
    if let Some(asset_token) = asset_token {
        let reward_info = rewards_bucket.may_load(asset_token.as_slice())?;
        reward_pairs = if let Some(reward_info) = reward_info {
            vec![(asset_token.clone(), reward_info)]
        } else {
            vec![]
        };
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
        let pool_info: PoolInfo = read_pool_info(storage, &asset_token_raw)?;

        // Withdraw reward to pending reward
        before_share_change(&pool_info, &mut reward_info, is_short)?;

        amount += reward_info.pending_reward;
        reward_info.pending_reward = Uint128::zero();

        // Update rewards info
        if reward_info.pending_reward.is_zero() && reward_info.bond_amount.is_zero() {
            rewards_store(storage, &staker_addr, is_short).remove(asset_token_raw.as_slice());
        } else {
            rewards_store(storage, &staker_addr, is_short)
                .save(asset_token_raw.as_slice(), &reward_info)?;
        }
    }

    Ok(amount)
}

// withdraw reward to pending reward
pub fn before_share_change(
    pool_info: &PoolInfo,
    reward_info: &mut RewardInfo,
    is_short: bool,
) -> StdResult<()> {
    let pool_index = if is_short {
        pool_info.short_reward_index
    } else {
        pool_info.reward_index
    };

    let pending_reward =
        (reward_info.bond_amount * pool_index - reward_info.bond_amount * reward_info.index)?;

    reward_info.index = pool_index;
    reward_info.pending_reward += pending_reward;
    Ok(())
}

pub fn query_reward_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    staker_addr: HumanAddr,
    asset_token: Option<HumanAddr>,
) -> StdResult<RewardInfoResponse> {
    let staker_addr_raw = deps.api.canonical_address(&staker_addr)?;

    let reward_infos: Vec<RewardInfoResponseItem> = vec![
        _read_reward_infos(
            &deps.api,
            &deps.storage,
            &staker_addr_raw,
            &asset_token,
            false,
        )?,
        _read_reward_infos(
            &deps.api,
            &deps.storage,
            &staker_addr_raw,
            &asset_token,
            true,
        )?,
    ]
    .concat();

    Ok(RewardInfoResponse {
        staker_addr,
        reward_infos,
    })
}

fn _read_reward_infos<S: Storage, A: Api>(
    api: &A,
    storage: &S,
    staker_addr: &CanonicalAddr,
    asset_token: &Option<HumanAddr>,
    is_short: bool,
) -> StdResult<Vec<RewardInfoResponseItem>> {
    let rewards_bucket = rewards_read(storage, &staker_addr, is_short);
    let reward_infos: Vec<RewardInfoResponseItem>;
    if let Some(asset_token) = asset_token {
        let asset_token_raw = api.canonical_address(&asset_token)?;

        reward_infos =
            if let Some(mut reward_info) = rewards_bucket.may_load(asset_token_raw.as_slice())? {
                let pool_info = read_pool_info(storage, &asset_token_raw)?;
                before_share_change(&pool_info, &mut reward_info, is_short)?;

                vec![RewardInfoResponseItem {
                    asset_token: asset_token.clone(),
                    bond_amount: reward_info.bond_amount,
                    pending_reward: reward_info.pending_reward,
                    is_short,
                }]
            } else {
                vec![]
            };
    } else {
        reward_infos = rewards_bucket
            .range(None, None, Order::Ascending)
            .map(|item| {
                let (k, v) = item?;
                let asset_token_raw = CanonicalAddr::from(k);
                let mut reward_info = v;
                let pool_info = read_pool_info(storage, &asset_token_raw)?;
                before_share_change(&pool_info, &mut reward_info, is_short)?;

                Ok(RewardInfoResponseItem {
                    asset_token: api.human_address(&asset_token_raw)?,
                    bond_amount: reward_info.bond_amount,
                    pending_reward: reward_info.pending_reward,
                    is_short,
                })
            })
            .collect::<StdResult<Vec<RewardInfoResponseItem>>>()?;
    }

    Ok(reward_infos)
}
