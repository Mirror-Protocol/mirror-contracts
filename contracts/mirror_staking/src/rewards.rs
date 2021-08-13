use cosmwasm_std::{
    attr, to_binary, Addr, Api, CanonicalAddr, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Order, Response, StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::querier::{compute_premium_rate, compute_short_reward_weight};
use crate::state::{
    read_config, read_pool_info, rewards_read, rewards_store, store_pool_info, Config, PoolInfo,
    RewardInfo,
};
use mirror_protocol::staking::{RewardInfoResponse, RewardInfoResponseItem};

use cw20::Cw20ExecuteMsg;

pub fn adjust_premium(deps: DepsMut, env: Env, asset_tokens: Vec<String>) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let oracle_contract = deps.api.addr_humanize(&config.oracle_contract)?;
    let terraswap_factory = deps.api.addr_humanize(&config.terraswap_factory)?;
    let short_reward_contract = deps.api.addr_humanize(&config.short_reward_contract)?;
    for asset_token in asset_tokens.iter() {
        let asset_token_raw = deps.api.addr_canonicalize(asset_token)?;
        let pool_info: PoolInfo = read_pool_info(deps.storage, &asset_token_raw)?;
        if env.block.time.nanos() / 1_000_000_000
            < pool_info.premium_updated_time + config.premium_min_update_interval
        {
            return Err(StdError::generic_err(
                "cannot adjust premium before premium_min_update_interval passed",
            ));
        }

        let asset_token_addr = deps.api.addr_validate(asset_token)?;

        let (premium_rate, no_price_feed) = compute_premium_rate(
            deps.as_ref(),
            oracle_contract.clone(),
            terraswap_factory.clone(),
            asset_token_addr,
            config.base_denom.to_string(),
        )?;

        // if asset does not have price feed, set short reward weight directly to zero
        let short_reward_weight = if no_price_feed {
            Decimal::zero()
        } else {
            compute_short_reward_weight(&deps.querier, short_reward_contract.clone(), premium_rate)?
        };

        store_pool_info(
            deps.storage,
            &asset_token_raw,
            &PoolInfo {
                premium_rate,
                short_reward_weight,
                premium_updated_time: env.block.time.nanos() / 1_000_000_000,
                ..pool_info
            },
        )?;
    }

    Ok(Response::new().add_attributes(vec![attr("action", "premium_adjustment")]))
}

// deposit_reward must be from reward token contract
pub fn deposit_reward(
    deps: DepsMut,
    rewards: Vec<(String, Uint128)>,
    rewards_amount: Uint128,
) -> StdResult<Response> {
    for (asset_token, amount) in rewards.iter() {
        let asset_token_raw: CanonicalAddr = deps.api.addr_canonicalize(asset_token)?;
        let mut pool_info: PoolInfo = read_pool_info(deps.storage, &asset_token_raw)?;

        // Decimal::from_ratio(1, 5).mul()
        // erf(pool_info.premium_rate.0)
        // 3.0f64
        let total_reward = *amount;
        let mut short_reward = total_reward * pool_info.short_reward_weight;
        let mut normal_reward = total_reward.checked_sub(short_reward).unwrap();

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

        store_pool_info(deps.storage, &asset_token_raw, &pool_info)?;
    }

    Ok(Response::new().add_attributes(vec![
        attr("action", "deposit_reward"),
        attr("rewards_amount", rewards_amount.to_string()),
    ]))
}

// withdraw all rewards or single reward depending on asset_token
pub fn withdraw_reward(
    deps: DepsMut,
    info: MessageInfo,
    asset_token: Option<Addr>,
) -> StdResult<Response> {
    let staker_addr = deps.api.addr_canonicalize(info.sender.as_str())?;
    let asset_token = asset_token.map(|a| deps.api.addr_canonicalize(a.as_str()).unwrap());
    let normal_reward = _withdraw_reward(deps.storage, &staker_addr, &asset_token, false)?;
    let short_reward = _withdraw_reward(deps.storage, &staker_addr, &asset_token, true)?;

    let amount = normal_reward + short_reward;
    let config: Config = read_config(deps.storage)?;
    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.mirror_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }))
        .add_attributes(vec![
            attr("action", "withdraw"),
            attr("amount", amount.to_string()),
        ]))
}

fn _withdraw_reward(
    storage: &mut dyn Storage,
    staker_addr: &CanonicalAddr,
    asset_token: &Option<CanonicalAddr>,
    is_short: bool,
) -> StdResult<Uint128> {
    let rewards_bucket = rewards_read(storage, staker_addr, is_short);

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
        if reward_info.bond_amount.is_zero() {
            rewards_store(storage, staker_addr, is_short).remove(asset_token_raw.as_slice());
        } else {
            rewards_store(storage, staker_addr, is_short)
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

    let pending_reward = (reward_info.bond_amount * pool_index)
        .checked_sub(reward_info.bond_amount * reward_info.index)?;

    reward_info.index = pool_index;
    reward_info.pending_reward += pending_reward;
    Ok(())
}

pub fn query_reward_info(
    deps: Deps,
    staker_addr: String,
    asset_token: Option<String>,
) -> StdResult<RewardInfoResponse> {
    let staker_addr_raw = deps.api.addr_canonicalize(staker_addr.as_str())?;

    let reward_infos: Vec<RewardInfoResponseItem> = vec![
        _read_reward_infos(
            deps.api,
            deps.storage,
            &staker_addr_raw,
            &asset_token,
            false,
        )?,
        _read_reward_infos(deps.api, deps.storage, &staker_addr_raw, &asset_token, true)?,
    ]
    .concat();

    Ok(RewardInfoResponse {
        staker_addr,
        reward_infos,
    })
}

fn _read_reward_infos(
    api: &dyn Api,
    storage: &dyn Storage,
    staker_addr: &CanonicalAddr,
    asset_token: &Option<String>,
    is_short: bool,
) -> StdResult<Vec<RewardInfoResponseItem>> {
    let rewards_bucket = rewards_read(storage, staker_addr, is_short);
    let reward_infos: Vec<RewardInfoResponseItem>;
    if let Some(asset_token) = asset_token {
        let asset_token_raw = api.addr_canonicalize(asset_token.as_str())?;

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
                    asset_token: api.addr_humanize(&asset_token_raw)?.to_string(),
                    bond_amount: reward_info.bond_amount,
                    pending_reward: reward_info.pending_reward,
                    is_short,
                })
            })
            .collect::<StdResult<Vec<RewardInfoResponseItem>>>()?;
    }

    Ok(reward_infos)
}
