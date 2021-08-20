use crate::querier::load_token_balance;
use crate::state::{
    bank_read, bank_store, config_read, config_store, poll_read, poll_store, poll_voter_read,
    poll_voter_store, read_bank_stakers, read_polls, state_read, state_store, Config, Poll, State,
    TokenManager,
};

use cosmwasm_std::{
    attr, to_binary, CanonicalAddr, CosmosMsg, Deps, DepsMut, MessageInfo, Response, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use mirror_protocol::common::OrderBy;
use mirror_protocol::gov::{
    PollStatus, SharesResponse, SharesResponseItem, StakerResponse, VoterInfo,
};

pub fn stake_voting_tokens(deps: DepsMut, sender: String, amount: Uint128) -> StdResult<Response> {
    if amount.is_zero() {
        return Err(StdError::generic_err("Insufficient funds sent"));
    }

    let sender_address_raw = deps.api.addr_canonicalize(&sender)?;
    let key = &sender_address_raw.as_slice();

    let mut token_manager = bank_read(deps.storage).may_load(key)?.unwrap_or_default();
    let config: Config = config_store(deps.storage).load()?;
    let mut state: State = state_store(deps.storage).load()?;

    // balance already increased, so subtract deposit amount
    let total_locked_balance = state.total_deposit + state.pending_voting_rewards;

    let total_balance = load_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.mirror_token)?.to_string(),
        &state.contract_addr,
    )?
    .checked_sub(total_locked_balance + amount)?;

    let share = if total_balance.is_zero() || state.total_share.is_zero() {
        amount
    } else {
        amount.multiply_ratio(state.total_share, total_balance)
    };

    token_manager.share += share;
    state.total_share += share;

    state_store(deps.storage).save(&state)?;
    bank_store(deps.storage).save(key, &token_manager)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "staking"),
        attr("sender", sender.as_str()),
        attr("share", share.to_string()),
        attr("amount", amount.to_string()),
    ]))
}

// Withdraw amount if not staked. By default all funds will be withdrawn.
pub fn withdraw_voting_tokens(
    deps: DepsMut,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> StdResult<Response> {
    let sender_address_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let key = sender_address_raw.as_slice();

    if let Some(mut token_manager) = bank_read(deps.storage).may_load(key)? {
        let config: Config = config_store(deps.storage).load()?;
        let mut state: State = state_store(deps.storage).load()?;

        // Load total share & total balance except proposal deposit amount
        let total_share = state.total_share.u128();
        let total_locked_balance = state.total_deposit + state.pending_voting_rewards;
        let total_balance = (load_token_balance(
            &deps.querier,
            deps.api.addr_humanize(&config.mirror_token)?.to_string(),
            &state.contract_addr,
        )?
        .checked_sub(total_locked_balance))?
        .u128();

        let user_locked_balance =
            compute_locked_balance(deps.storage, &mut token_manager, &sender_address_raw)?;
        let user_locked_share = user_locked_balance * total_share / total_balance;
        let user_share = token_manager.share.u128();

        let withdraw_share = amount
            .map(|v| std::cmp::max(v.multiply_ratio(total_share, total_balance).u128(), 1u128))
            .unwrap_or_else(|| user_share - user_locked_share);
        let withdraw_amount = amount
            .map(|v| v.u128())
            .unwrap_or_else(|| withdraw_share * total_balance / total_share);

        if user_locked_share + withdraw_share > user_share {
            Err(StdError::generic_err(
                "User is trying to withdraw too many tokens.",
            ))
        } else {
            let share = user_share - withdraw_share;
            token_manager.share = Uint128::from(share);

            bank_store(deps.storage).save(key, &token_manager)?;

            state.total_share = Uint128::from(total_share - withdraw_share);
            state_store(deps.storage).save(&state)?;

            send_tokens(
                deps,
                &config.mirror_token,
                &sender_address_raw,
                withdraw_amount,
                "withdraw",
            )
        }
    } else {
        Err(StdError::generic_err("Nothing staked"))
    }
}

// returns the largest locked amount in participated polls.
fn compute_locked_balance(
    storage: &mut dyn Storage,
    token_manager: &mut TokenManager,
    voter: &CanonicalAddr,
) -> StdResult<u128> {
    // filter out not in-progress polls and get max locked
    let mut lock_entries_to_remove: Vec<u64> = vec![];
    let max_locked = token_manager
        .locked_balance
        .iter()
        .filter(|(poll_id, _)| {
            let poll: Poll = poll_read(storage).load(&poll_id.to_be_bytes()).unwrap();

            // cleanup not needed information, voting info in polls with no rewards
            if poll.status != PollStatus::InProgress && poll.voters_reward.is_zero() {
                poll_voter_store(storage, *poll_id).remove(voter.as_slice());
                lock_entries_to_remove.push(*poll_id);
            }

            poll.status == PollStatus::InProgress
        })
        .map(|(_, v)| v.balance.u128())
        .max()
        .unwrap_or_default();

    // cleanup, check if there was any voter info removed
    token_manager
        .locked_balance
        .retain(|(poll_id, _)| !lock_entries_to_remove.contains(poll_id));

    Ok(max_locked)
}

pub fn deposit_reward(deps: DepsMut, amount: Uint128) -> StdResult<Response> {
    let config = config_read(deps.storage).load()?;

    let mut polls_in_progress = read_polls(
        deps.storage,
        Some(PollStatus::InProgress),
        None,
        None,
        None,
        Some(true), // remove hard cap to get all polls
    )?;

    if config.voter_weight.is_zero() || polls_in_progress.is_empty() {
        return Ok(Response::new().add_attributes(vec![
            attr("action", "deposit_reward"),
            attr("amount", amount.to_string()),
        ]));
    }

    let voter_rewards = amount * config.voter_weight;
    let rewards_per_poll =
        voter_rewards.multiply_ratio(Uint128::new(1), polls_in_progress.len() as u128);
    if rewards_per_poll.is_zero() {
        return Err(StdError::generic_err("Reward deposited is too small"));
    }
    for poll in polls_in_progress.iter_mut() {
        poll.voters_reward += rewards_per_poll;
        poll_store(deps.storage)
            .save(&poll.id.to_be_bytes(), poll)
            .unwrap()
    }

    state_store(deps.storage).update(|mut state| -> StdResult<_> {
        state.pending_voting_rewards += voter_rewards;
        Ok(state)
    })?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "deposit_reward"),
        attr("amount", amount.to_string()),
    ]))
}

pub fn withdraw_voting_rewards(
    deps: DepsMut,
    info: MessageInfo,
    poll_id: Option<u64>,
) -> StdResult<Response> {
    let config: Config = config_store(deps.storage).load()?;
    let sender_address_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let key = sender_address_raw.as_slice();

    let mut token_manager = bank_read(deps.storage)
        .load(key)
        .map_err(|_| StdError::generic_err("Nothing staked"))?;

    let (user_reward_amount, w_polls) =
        withdraw_user_voting_rewards(deps.storage, &sender_address_raw, &token_manager, poll_id)?;
    if user_reward_amount.eq(&0u128) {
        return Err(StdError::generic_err("Nothing to withdraw"));
    }

    // cleanup, remove from locked_balance the polls from which we withdrew the rewards
    token_manager
        .locked_balance
        .retain(|(poll_id, _)| !w_polls.contains(poll_id));
    bank_store(deps.storage).save(key, &token_manager)?;

    state_store(deps.storage).update(|mut state| -> StdResult<_> {
        state.pending_voting_rewards = state
            .pending_voting_rewards
            .checked_sub(Uint128::new(user_reward_amount))?;
        Ok(state)
    })?;

    send_tokens(
        deps,
        &config.mirror_token,
        &sender_address_raw,
        user_reward_amount,
        "withdraw_voting_rewards",
    )
}

pub fn stake_voting_rewards(
    deps: DepsMut,
    info: MessageInfo,
    poll_id: Option<u64>,
) -> StdResult<Response> {
    let config: Config = config_store(deps.storage).load()?;
    let mut state: State = state_store(deps.storage).load()?;
    let sender_address_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let key = sender_address_raw.as_slice();

    let mut token_manager = bank_read(deps.storage)
        .load(key)
        .map_err(|_| StdError::generic_err("Nothing staked"))?;

    let (user_reward_amount, w_polls) =
        withdraw_user_voting_rewards(deps.storage, &sender_address_raw, &token_manager, poll_id)?;
    if user_reward_amount.eq(&0u128) {
        return Err(StdError::generic_err("Nothing to withdraw"));
    }

    // add the withdrawn rewards to stake pool and calculate share
    let total_locked_balance = state.total_deposit + state.pending_voting_rewards;
    let total_balance = load_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.mirror_token)?.to_string(),
        &state.contract_addr,
    )?
    .checked_sub(total_locked_balance)?;

    state.pending_voting_rewards = state
        .pending_voting_rewards
        .checked_sub(Uint128::new(user_reward_amount))?;

    let share: Uint128 = if total_balance.is_zero() || state.total_share.is_zero() {
        Uint128::new(user_reward_amount)
    } else {
        Uint128::new(user_reward_amount).multiply_ratio(state.total_share, total_balance)
    };

    token_manager.share += share;
    state.total_share += share;

    // cleanup, remove from locked_balance the polls from which we withdrew the rewards
    token_manager
        .locked_balance
        .retain(|(poll_id, _)| !w_polls.contains(poll_id));

    state_store(deps.storage).save(&state)?;
    bank_store(deps.storage).save(key, &token_manager)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "stake_voting_rewards"),
        attr("staker", info.sender.as_str()),
        attr("share", share.to_string()),
        attr("amount", user_reward_amount.to_string()),
    ]))
}

fn withdraw_user_voting_rewards(
    storage: &mut dyn Storage,
    user_address: &CanonicalAddr,
    token_manager: &TokenManager,
    poll_id: Option<u64>,
) -> StdResult<(u128, Vec<u64>)> {
    let w_polls: Vec<(Poll, VoterInfo)> = match poll_id {
        Some(poll_id) => {
            let poll: Poll = poll_read(storage).load(&poll_id.to_be_bytes())?;
            let voter_info = poll_voter_read(storage, poll_id).load(user_address.as_slice())?;
            if poll.status == PollStatus::InProgress {
                return Err(StdError::generic_err("This poll is still in progress"));
            }
            if poll.voters_reward.is_zero() {
                return Err(StdError::generic_err("This poll has no voting rewards"));
            }
            vec![(poll, voter_info)]
        }
        None => get_withdrawable_polls(storage, token_manager, user_address),
    };
    let user_reward_amount: u128 = w_polls
        .iter()
        .map(|(poll, voting_info)| {
            // remove voter info from the poll
            poll_voter_store(storage, poll.id).remove(user_address.as_slice());

            // calculate reward share
            let total_votes =
                poll.no_votes.u128() + poll.yes_votes.u128() + poll.abstain_votes.u128();
            let poll_voting_reward = poll
                .voters_reward
                .multiply_ratio(voting_info.balance, total_votes);
            poll_voting_reward.u128()
        })
        .sum();
    Ok((
        user_reward_amount,
        w_polls.iter().map(|(poll, _)| poll.id).collect(),
    ))
}

fn get_withdrawable_polls(
    storage: &dyn Storage,
    token_manager: &TokenManager,
    user_address: &CanonicalAddr,
) -> Vec<(Poll, VoterInfo)> {
    let w_polls: Vec<(Poll, VoterInfo)> = token_manager
        .locked_balance
        .iter()
        .map(|(poll_id, _)| {
            let poll: Poll = poll_read(storage).load(&poll_id.to_be_bytes()).unwrap();
            let voter_info_res: StdResult<VoterInfo> =
                poll_voter_read(storage, *poll_id).load(user_address.as_slice());
            (poll, voter_info_res)
        })
        .filter(|(poll, voter_info_res)| {
            poll.status != PollStatus::InProgress
                && voter_info_res.is_ok()
                && !poll.voters_reward.is_zero()
        })
        .map(|(poll, voter_info_res)| (poll, voter_info_res.unwrap()))
        .collect();
    w_polls
}

fn send_tokens(
    deps: DepsMut,
    asset_token: &CanonicalAddr,
    recipient: &CanonicalAddr,
    amount: u128,
    action: &str,
) -> StdResult<Response> {
    let contract_human = deps.api.addr_humanize(asset_token)?.to_string();
    let recipient_human = deps.api.addr_humanize(recipient)?.to_string();
    let attributes = vec![
        attr("action", action),
        attr("recipient", recipient_human.as_str()),
        attr("amount", &amount.to_string()),
    ];

    let r = Response::new()
        .add_attributes(attributes)
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_human,
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient_human,
                amount: Uint128::from(amount),
            })?,
            funds: vec![],
        }));
    Ok(r)
}

pub fn query_staker(deps: Deps, address: String) -> StdResult<StakerResponse> {
    let addr_raw = deps.api.addr_canonicalize(&address).unwrap();
    let config: Config = config_read(deps.storage).load()?;
    let state: State = state_read(deps.storage).load()?;
    let mut token_manager = bank_read(deps.storage)
        .may_load(addr_raw.as_slice())?
        .unwrap_or_default();

    // calculate pending voting rewards
    let w_polls: Vec<(Poll, VoterInfo)> =
        get_withdrawable_polls(deps.storage, &token_manager, &addr_raw);

    let mut user_reward_amount = Uint128::zero();
    let w_polls_res: Vec<(u64, Uint128)> = w_polls
        .iter()
        .map(|(poll, voting_info)| {
            // calculate reward share
            let total_votes = poll.no_votes + poll.yes_votes + poll.abstain_votes;
            let poll_voting_reward = poll
                .voters_reward
                .multiply_ratio(voting_info.balance, total_votes);
            user_reward_amount += poll_voting_reward;

            (poll.id, poll_voting_reward)
        })
        .collect();

    // filter out not in-progress polls
    token_manager.locked_balance.retain(|(poll_id, _)| {
        let poll: Poll = poll_read(deps.storage)
            .load(&poll_id.to_be_bytes())
            .unwrap();

        poll.status == PollStatus::InProgress
    });

    let total_locked_balance = state.total_deposit + state.pending_voting_rewards;
    let total_balance = load_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.mirror_token)?.to_string(),
        &state.contract_addr,
    )?
    .checked_sub(total_locked_balance)?;

    Ok(StakerResponse {
        balance: if !state.total_share.is_zero() {
            token_manager
                .share
                .multiply_ratio(total_balance, state.total_share)
        } else {
            Uint128::zero()
        },
        share: token_manager.share,
        locked_balance: token_manager.locked_balance,
        pending_voting_rewards: user_reward_amount,
        withdrawable_polls: w_polls_res,
    })
}

pub fn query_shares(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<SharesResponse> {
    let stakers: Vec<(CanonicalAddr, TokenManager)> = if let Some(start_after) = start_after {
        read_bank_stakers(
            deps.storage,
            Some(deps.api.addr_canonicalize(&start_after)?),
            limit,
            order_by,
        )?
    } else {
        read_bank_stakers(deps.storage, None, limit, order_by)?
    };

    let stakers_shares: Vec<SharesResponseItem> = stakers
        .iter()
        .map(|item| {
            let (k, v) = item;
            SharesResponseItem {
                staker: deps.api.addr_humanize(k).unwrap().to_string(),
                share: v.share,
            }
        })
        .collect();

    Ok(SharesResponse {
        stakers: stakers_shares,
    })
}
