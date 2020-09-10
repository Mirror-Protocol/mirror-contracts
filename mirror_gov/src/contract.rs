use crate::msg::{
    ConfigResponse, CreatePollResponse, Cw20HookMsg, ExecuteMsg, HandleMsg, InitMsg, PollResponse,
    QueryMsg, StakeResponse,
};
use crate::querier::load_token_balance;
use crate::state::{
    bank, bank_read, config, config_read, poll, poll_read, ExecuteData, Poll, PollStatus, State,
    Voter,
};
use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, InitResult, Querier, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};

pub const VOTING_TOKEN: &str = "voting_token";
pub const DEFAULT_END_HEIGHT_BLOCKS: &u64 = &100_800_u64;
const MIN_DESC_LENGTH: usize = 3;
const MAX_DESC_LENGTH: usize = 64;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> InitResult {
    let state = State {
        mirror_token: deps.api.canonical_address(&msg.mirror_token)?,
        owner: deps.api.canonical_address(&env.message.sender)?,
        contract_addr: deps.api.canonical_address(&env.contract.address)?,
        poll_count: 0,
        total_share: Uint128::zero(),
    };

    config(&mut deps.storage).save(&state)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Receive(msg) => receive_cw20(deps, env, msg),
        HandleMsg::WithdrawVotingTokens { amount } => withdraw_voting_tokens(deps, env, amount),
        HandleMsg::CastVote {
            poll_id,
            vote,
            share,
        } => cast_vote(deps, env, poll_id, vote, share),
        HandleMsg::EndPoll { poll_id } => end_poll(deps, env, poll_id),
        HandleMsg::CreatePoll {
            quorum_percentage,
            description,
            start_height,
            end_height,
            execute_msg,
        } => create_poll(
            deps,
            env,
            quorum_percentage,
            description,
            start_height,
            end_height,
            execute_msg,
        ),
    }
}

pub fn receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> HandleResult {
    // only asset contract can execute this message
    let config: State = config_read(&deps.storage).load()?;
    if config.mirror_token != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if let Some(msg) = cw20_msg.msg {
        match from_binary(&msg)? {
            Cw20HookMsg::StakeVotingTokens {} => {
                stake_voting_tokens(deps, env, cw20_msg.sender, cw20_msg.amount)
            }
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

pub fn stake_voting_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    sender: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    if amount.is_zero() {
        return Err(StdError::generic_err("Insufficient funds sent"));
    }

    let sender_address_raw = deps.api.canonical_address(&sender)?;
    let key = &sender_address_raw.as_slice();

    let mut token_manager = bank_read(&deps.storage).may_load(key)?.unwrap_or_default();
    let mut state: State = config(&mut deps.storage).load()?;

    // balance already increased, so subtract depoist amount
    let total_balance = (load_token_balance(
        &deps,
        &deps.api.human_address(&state.mirror_token)?,
        &state.contract_addr,
    )? - amount)?;

    let share = if total_balance.is_zero() || state.total_share.is_zero() {
        amount
    } else {
        amount.multiply_ratio(state.total_share, total_balance)
    };

    token_manager.share += share;
    state.total_share += share;

    config(&mut deps.storage).save(&state)?;
    bank(&mut deps.storage).save(key, &token_manager)?;

    Ok(HandleResponse {
        messages: vec![],
        data: None,
        log: vec![
            log("action", "staking"),
            log("sender", sender.as_str()),
            log("share", share.to_string()),
            log("amount", amount.to_string()),
        ],
    })
}

// Withdraw amount if not staked. By default all funds will be withdrawn.
pub fn withdraw_voting_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Option<Uint128>,
) -> HandleResult {
    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let key = sender_address_raw.as_slice();

    if let Some(mut token_manager) = bank_read(&deps.storage).may_load(key)? {
        let mut state: State = config(&mut deps.storage).load()?;
        let total_share = state.total_share;
        let total_balance = load_token_balance(
            &deps,
            &deps.api.human_address(&state.mirror_token)?,
            &state.contract_addr,
        )?;

        let locked_share = locked_share(&sender_address_raw, deps);
        let withdraw_share = match amount {
            Some(amount) => Some(
                // balance to share
                amount.multiply_ratio(total_share, total_balance).u128(),
            ),
            None => Some(token_manager.share.u128()),
        }
        .unwrap();

        if locked_share + withdraw_share > token_manager.share.u128() {
            Err(StdError::generic_err(
                "User is trying to withdraw too many tokens.",
            ))
        } else {
            let share = token_manager.share.u128() - withdraw_share;
            token_manager.share = Uint128::from(share);

            bank(&mut deps.storage).save(key, &token_manager)?;

            state.total_share = Uint128::from(total_share.u128() - withdraw_share);
            config(&mut deps.storage).save(&state)?;

            send_tokens(
                &deps.api,
                &state.mirror_token,
                &sender_address_raw,
                // share to balance
                Uint128(withdraw_share)
                    .multiply_ratio(total_balance, total_share)
                    .u128(),
                "withdraw",
            )
        }
    } else {
        Err(StdError::generic_err("Nothing staked"))
    }
}

/// validate_description returns an error if the description is invalid
fn validate_description(description: &str) -> StdResult<()> {
    if description.len() < MIN_DESC_LENGTH {
        Err(StdError::generic_err("Description too short"))
    } else if description.len() > MAX_DESC_LENGTH {
        Err(StdError::generic_err("Description too long"))
    } else {
        Ok(())
    }
}

/// validate_quorum_percentage returns an error if the quorum_percentage is invalid
/// (we require 0-100)
fn validate_quorum_percentage(quorum_percentage: Option<u8>) -> StdResult<()> {
    if quorum_percentage.is_some() && quorum_percentage.unwrap() > 100 {
        Err(StdError::generic_err("quorum_percentage must be 0 to 100"))
    } else {
        Ok(())
    }
}

/// validate_end_height returns an error if the poll ends in the past
fn validate_end_height(end_height: Option<u64>, env: Env) -> StdResult<()> {
    if end_height.is_some() && env.block.height >= end_height.unwrap() {
        Err(StdError::generic_err("Poll cannot end in the past"))
    } else {
        Ok(())
    }
}

/// create a new poll
pub fn create_poll<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    quorum_percentage: Option<u8>,
    description: String,
    start_height: Option<u64>,
    end_height: Option<u64>,
    execute_msg: Option<ExecuteMsg>,
) -> StdResult<HandleResponse> {
    validate_quorum_percentage(quorum_percentage)?;
    validate_end_height(end_height, env.clone())?;
    validate_description(&description)?;

    let mut state = config(&mut deps.storage).load()?;
    let poll_count = state.poll_count;
    let poll_id = poll_count + 1;
    state.poll_count = poll_id;

    let execute_data = if let Some(execute_msg) = execute_msg {
        Some(ExecuteData {
            contract: deps.api.canonical_address(&execute_msg.contract)?,
            msg: execute_msg.msg,
        })
    } else {
        None
    };

    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let new_poll = Poll {
        creator: sender_address_raw,
        status: PollStatus::InProgress,
        quorum_percentage,
        yes_votes: Uint128::zero(),
        no_votes: Uint128::zero(),
        voters: vec![],
        voter_info: vec![],
        end_height: end_height.unwrap_or(env.block.height + DEFAULT_END_HEIGHT_BLOCKS),
        start_height,
        description,
        execute_data,
    };

    let key = state.poll_count.to_string();
    poll(&mut deps.storage).save(key.as_bytes(), &new_poll)?;

    config(&mut deps.storage).save(&state)?;

    let r = HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "create_poll"),
            log(
                "creator",
                deps.api.human_address(&new_poll.creator)?.as_str(),
            ),
            log("poll_id", &poll_id.to_string()),
            log("quorum_percentage", quorum_percentage.unwrap_or(0)),
            log("end_height", new_poll.end_height),
            log("start_height", start_height.unwrap_or(0)),
        ],
        data: Some(to_binary(&CreatePollResponse { poll_id })?),
    };
    Ok(r)
}

/*
 * Ends a poll. Only the creator of a given poll can end that poll.
 */
pub fn end_poll<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    poll_id: u64,
) -> HandleResult {
    let key = &poll_id.to_string();
    let mut a_poll: Poll = poll(&mut deps.storage).load(key.as_bytes())?;

    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    if a_poll.creator != sender_address_raw {
        return Err(StdError::generic_err(
            "User is not the creator of the poll.",
        ));
    }

    if a_poll.status != PollStatus::InProgress {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    if a_poll.start_height.is_some() && a_poll.start_height.unwrap() > env.block.height {
        return Err(StdError::generic_err("Voting period has not started."));
    }

    if a_poll.end_height > env.block.height {
        return Err(StdError::generic_err("Voting period has not expired."));
    }

    let mut no = 0u128;
    let mut yes = 0u128;

    for voter in &a_poll.voter_info {
        if voter.vote == "yes" {
            yes += voter.share.u128();
        } else {
            no += voter.share.u128();
        }
    }
    let tallied_weight = yes + no;

    let poll_status = PollStatus::Rejected;
    let mut rejected_reason = "";
    let mut passed = false;

    if tallied_weight > 0 {
        let state: State = config_read(&deps.storage).load()?;

        let staked_weight = state.total_share.u128();
        if staked_weight == 0 {
            return Err(StdError::generic_err("Nothing staked"));
        }

        let quorum =
            (Decimal::from_ratio(tallied_weight, staked_weight) * Uint128(100u128)).u128() as u8;
        if a_poll.quorum_percentage.is_some() && quorum < a_poll.quorum_percentage.unwrap() {
            // Quorum: More than quorum_percentage of the total staked tokens at the end of the voting
            // period need to have participated in the vote.
            rejected_reason = "Quorum not reached";
        } else if yes > tallied_weight / 2 {
            //Threshold: More than 50% of the tokens that participated in the vote
            // (after excluding “Abstain” votes) need to have voted in favor of the proposal (“Yes”).
            a_poll.status = PollStatus::Passed;
            passed = true;
        } else {
            rejected_reason = "Threshold not reached";
        }
    } else {
        rejected_reason = "Quorum not reached";
    }
    a_poll.status = poll_status;
    poll(&mut deps.storage).save(key.as_bytes(), &a_poll)?;

    for voter in &a_poll.voters {
        unlock_tokens(deps, voter, poll_id)?;
    }

    let log = vec![
        log("action", "end_poll"),
        log("poll_id", &poll_id.to_string()),
        log("rejected_reason", rejected_reason),
        log("passed", &passed.to_string()),
    ];

    let mut messages: Vec<CosmosMsg> = vec![];
    if passed {
        if let Some(execute_data) = a_poll.execute_data {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&execute_data.contract)?,
                msg: execute_data.msg,
                send: vec![],
            }))
        }
    }

    let r = HandleResponse {
        messages,
        log,
        data: None,
    };
    Ok(r)
}

// unlock voter's tokens in a given poll
fn unlock_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    voter: &CanonicalAddr,
    poll_id: u64,
) -> HandleResult {
    let voter_key = &voter.as_slice();
    let mut token_manager = bank_read(&deps.storage).load(voter_key).unwrap();

    // unlock entails removing the mapped poll_id, retaining the rest
    token_manager.locked_share.retain(|(k, _)| k != &poll_id);
    bank(&mut deps.storage).save(voter_key, &token_manager)?;
    Ok(HandleResponse::default())
}

// finds the largest locked amount in participated polls.
fn locked_share<S: Storage, A: Api, Q: Querier>(
    voter: &CanonicalAddr,
    deps: &mut Extern<S, A, Q>,
) -> u128 {
    let voter_key = &voter.as_slice();
    let token_manager = bank_read(&deps.storage).load(voter_key).unwrap();
    token_manager
        .locked_share
        .iter()
        .map(|(_, v)| v.u128())
        .max()
        .unwrap_or_default()
}

fn has_voted(voter: &CanonicalAddr, a_poll: &Poll) -> bool {
    a_poll.voters.iter().any(|i| i == voter)
}

pub fn cast_vote<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    poll_id: u64,
    vote: String,
    share: Uint128,
) -> HandleResult {
    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let poll_key = &poll_id.to_string();
    let state = config_read(&deps.storage).load()?;
    if poll_id == 0 || state.poll_count > poll_id {
        return Err(StdError::generic_err("Poll does not exist"));
    }

    let mut a_poll = poll(&mut deps.storage).load(poll_key.as_bytes())?;

    if a_poll.status != PollStatus::InProgress {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    if has_voted(&sender_address_raw, &a_poll) {
        return Err(StdError::generic_err("User has already voted."));
    }

    let key = &sender_address_raw.as_slice();
    let mut token_manager = bank_read(&deps.storage).may_load(key)?.unwrap_or_default();

    if token_manager.share < share {
        return Err(StdError::generic_err(
            "User does not have enough staked tokens.",
        ));
    }
    token_manager.participated_polls.push(poll_id);
    token_manager.locked_share.push((poll_id, share));
    bank(&mut deps.storage).save(key, &token_manager)?;

    a_poll.voters.push(sender_address_raw.clone());

    let voter_info = Voter { vote, share };

    a_poll.voter_info.push(voter_info);
    poll(&mut deps.storage).save(poll_key.as_bytes(), &a_poll)?;

    let log = vec![
        log("action", "vote_casted"),
        log("poll_id", &poll_id.to_string()),
        log("share", &share.to_string()),
        log("voter", &env.message.sender.as_str()),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn send_tokens<A: Api>(
    api: &A,
    asset_token: &CanonicalAddr,
    recipient: &CanonicalAddr,
    amount: u128,
    action: &str,
) -> HandleResult {
    let contract_human = api.human_address(asset_token)?;
    let recipient_human = api.human_address(recipient)?;
    let log = vec![
        log("action", action),
        log("recipient", recipient_human.as_str()),
        log("amount", &amount.to_string()),
    ];

    let r = HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_human,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: recipient_human,
                amount: Uint128::from(amount),
            })?,
            send: vec![],
        })],
        log,
        data: None,
    };
    Ok(r)
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(&deps)?),
        QueryMsg::Stake { address } => to_binary(&query_stake(deps, address)?),
        QueryMsg::Poll { poll_id } => to_binary(&query_poll(deps, poll_id)?),
    }
}

fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let config: State = config_read(&deps.storage).load()?;
    Ok(ConfigResponse {
        owner: deps.api.human_address(&config.owner)?,
        mirror_token: deps.api.human_address(&config.mirror_token)?,
        poll_count: config.poll_count,
        total_share: config.total_share,
    })
}

fn query_poll<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    poll_id: u64,
) -> StdResult<PollResponse> {
    let key = &poll_id.to_string();

    let poll = match poll_read(&deps.storage).may_load(key.as_bytes())? {
        Some(poll) => Some(poll),
        None => return Err(StdError::generic_err("Poll does not exist")),
    }
    .unwrap();

    Ok(PollResponse {
        creator: deps.api.human_address(&poll.creator).unwrap(),
        status: poll.status,
        quorum_percentage: poll.quorum_percentage,
        end_height: Some(poll.end_height),
        start_height: poll.start_height,
        description: poll.description,
    })
}

fn query_stake<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> StdResult<StakeResponse> {
    let addr_raw = deps.api.canonical_address(&address).unwrap();
    let state: State = config_read(&deps.storage).load()?;
    let token_manager = bank_read(&deps.storage)
        .may_load(addr_raw.as_slice())?
        .unwrap_or_default();

    let total_balance = load_token_balance(
        &deps,
        &deps.api.human_address(&state.mirror_token)?,
        &state.contract_addr,
    )?;

    Ok(StakeResponse {
        balance: token_manager
            .share
            .multiply_ratio(total_balance, state.total_share),
        share: token_manager.share,
    })
}
