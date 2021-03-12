use crate::querier::load_token_balance;
use crate::state::{
    bank_read, bank_store, config_read, config_store, poll_all_voters, poll_indexer_store,
    poll_read, poll_store, poll_voter_read, poll_voter_store, read_poll_voters, read_polls,
    state_read, state_store, Config, ExecuteData, Poll, State,
};
use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, InitResult, MigrateResponse,
    MigrateResult, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};

use mirror_protocol::common::OrderBy;
use mirror_protocol::gov::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, HandleMsg, InitMsg, MigrateMsg, PollResponse,
    PollStatus, PollsResponse, QueryMsg, StakerResponse, StateResponse, VoteOption, VoterInfo,
    VotersResponse, VotersResponseItem,
};

const MIN_TITLE_LENGTH: usize = 4;
const MAX_TITLE_LENGTH: usize = 64;
const MIN_DESC_LENGTH: usize = 4;
const MAX_DESC_LENGTH: usize = 256;
const MIN_LINK_LENGTH: usize = 12;
const MAX_LINK_LENGTH: usize = 128;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> InitResult {
    validate_quorum(msg.quorum)?;
    validate_threshold(msg.threshold)?;

    let config = Config {
        mirror_token: deps.api.canonical_address(&msg.mirror_token)?,
        owner: deps.api.canonical_address(&env.message.sender)?,
        quorum: msg.quorum,
        threshold: msg.threshold,
        voting_period: msg.voting_period,
        effective_delay: msg.effective_delay,
        expiration_period: msg.expiration_period,
        proposal_deposit: msg.proposal_deposit,
    };

    let state = State {
        contract_addr: deps.api.canonical_address(&env.contract.address)?,
        poll_count: 0,
        total_share: Uint128::zero(),
        total_deposit: Uint128::zero(),
    };

    config_store(&mut deps.storage).save(&config)?;
    state_store(&mut deps.storage).save(&state)?;

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
            quorum,
            threshold,
            voting_period,
            effective_delay,
            expiration_period,
            proposal_deposit,
        } => update_config(
            deps,
            env,
            owner,
            quorum,
            threshold,
            voting_period,
            effective_delay,
            expiration_period,
            proposal_deposit,
        ),
        HandleMsg::WithdrawVotingTokens { amount } => withdraw_voting_tokens(deps, env, amount),
        HandleMsg::CastVote {
            poll_id,
            vote,
            amount,
        } => cast_vote(deps, env, poll_id, vote, amount),
        HandleMsg::EndPoll { poll_id } => end_poll(deps, env, poll_id),
        HandleMsg::ExecutePoll { poll_id } => execute_poll(deps, env, poll_id),
        HandleMsg::ExpirePoll { poll_id } => expire_poll(deps, env, poll_id),
    }
}

pub fn receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> HandleResult {
    // only asset contract can execute this message
    let config: Config = config_read(&deps.storage).load()?;
    if config.mirror_token != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    if let Some(msg) = cw20_msg.msg {
        match from_binary(&msg)? {
            Cw20HookMsg::StakeVotingTokens {} => {
                stake_voting_tokens(deps, env, cw20_msg.sender, cw20_msg.amount)
            }
            Cw20HookMsg::CreatePoll {
                title,
                description,
                link,
                execute_msg,
            } => create_poll(
                deps,
                env,
                cw20_msg.sender,
                cw20_msg.amount,
                title,
                description,
                link,
                execute_msg,
            ),
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
    let config: Config = config_store(&mut deps.storage).load()?;
    let mut state: State = state_store(&mut deps.storage).load()?;

    // balance already increased, so subtract depoist amount
    let total_balance = (load_token_balance(
        &deps,
        &deps.api.human_address(&config.mirror_token)?,
        &state.contract_addr,
    )? - (state.total_deposit + amount))?;

    let share = if total_balance.is_zero() || state.total_share.is_zero() {
        amount
    } else {
        amount.multiply_ratio(state.total_share, total_balance)
    };

    token_manager.share += share;
    state.total_share += share;

    state_store(&mut deps.storage).save(&state)?;
    bank_store(&mut deps.storage).save(key, &token_manager)?;

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

#[allow(clippy::too_many_arguments)]
pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    quorum: Option<Decimal>,
    threshold: Option<Decimal>,
    voting_period: Option<u64>,
    effective_delay: Option<u64>,
    expiration_period: Option<u64>,
    proposal_deposit: Option<Uint128>,
) -> HandleResult {
    let api = deps.api;
    config_store(&mut deps.storage).update(|mut config| {
        if config.owner != api.canonical_address(&env.message.sender)? {
            return Err(StdError::unauthorized());
        }

        if let Some(owner) = owner {
            config.owner = api.canonical_address(&owner)?;
        }

        if let Some(quorum) = quorum {
            config.quorum = quorum;
        }

        if let Some(threshold) = threshold {
            config.threshold = threshold;
        }

        if let Some(voting_period) = voting_period {
            config.voting_period = voting_period;
        }

        if let Some(effective_delay) = effective_delay {
            config.effective_delay = effective_delay;
        }

        if let Some(expiration_period) = expiration_period {
            config.expiration_period = expiration_period;
        }

        if let Some(proposal_deposit) = proposal_deposit {
            config.proposal_deposit = proposal_deposit;
        }

        Ok(config)
    })?;
    Ok(HandleResponse::default())
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
        let config: Config = config_store(&mut deps.storage).load()?;
        let mut state: State = state_store(&mut deps.storage).load()?;

        // Load total share & total balance except proposal deposit amount
        let total_share = state.total_share;
        let total_balance = (load_token_balance(
            &deps,
            &deps.api.human_address(&config.mirror_token)?,
            &state.contract_addr,
        )? - state.total_deposit)?;

        let locked_balance = locked_balance(&sender_address_raw, deps);
        let locked_share = locked_balance * total_share.u128() / total_balance.u128();
        let withdraw_share = amount
            .map(|v| std::cmp::max(v.multiply_ratio(total_share, total_balance).u128(), 1u128))
            .unwrap_or_else(|| token_manager.share.u128());

        if locked_share + withdraw_share > token_manager.share.u128() {
            Err(StdError::generic_err(
                "User is trying to withdraw too many tokens.",
            ))
        } else {
            let share = token_manager.share.u128() - withdraw_share;
            token_manager.share = Uint128::from(share);

            bank_store(&mut deps.storage).save(key, &token_manager)?;

            state.total_share = Uint128::from(total_share.u128() - withdraw_share);
            state_store(&mut deps.storage).save(&state)?;

            send_tokens(
                &deps.api,
                &config.mirror_token,
                &sender_address_raw,
                amount.unwrap().u128(),
                "withdraw",
            )
        }
    } else {
        Err(StdError::generic_err("Nothing staked"))
    }
}

/// validate_title returns an error if the title is invalid
fn validate_title(title: &str) -> StdResult<()> {
    if title.len() < MIN_TITLE_LENGTH {
        Err(StdError::generic_err("Title too short"))
    } else if title.len() > MAX_TITLE_LENGTH {
        Err(StdError::generic_err("Title too long"))
    } else {
        Ok(())
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

/// validate_link returns an error if the link is invalid
fn validate_link(link: &Option<String>) -> StdResult<()> {
    if let Some(link) = link {
        if link.len() < MIN_LINK_LENGTH {
            Err(StdError::generic_err("Link too short"))
        } else if link.len() > MAX_LINK_LENGTH {
            Err(StdError::generic_err("Link too long"))
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

/// validate_quorum returns an error if the quorum is invalid
/// (we require 0-1)
fn validate_quorum(quorum: Decimal) -> StdResult<()> {
    if quorum > Decimal::one() {
        Err(StdError::generic_err("quorum must be 0 to 1"))
    } else {
        Ok(())
    }
}

/// validate_threshold returns an error if the threshold is invalid
/// (we require 0-1)
fn validate_threshold(threshold: Decimal) -> StdResult<()> {
    if threshold > Decimal::one() {
        Err(StdError::generic_err("threshold must be 0 to 1"))
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
/// create a new poll
pub fn create_poll<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    proposer: HumanAddr,
    deposit_amount: Uint128,
    title: String,
    description: String,
    link: Option<String>,
    execute_msg: Option<ExecuteMsg>,
) -> StdResult<HandleResponse> {
    validate_title(&title)?;
    validate_description(&description)?;
    validate_link(&link)?;

    let config: Config = config_store(&mut deps.storage).load()?;
    if deposit_amount < config.proposal_deposit {
        return Err(StdError::generic_err(format!(
            "Must deposit more than {} token",
            config.proposal_deposit
        )));
    }

    let mut state: State = state_store(&mut deps.storage).load()?;
    let poll_id = state.poll_count + 1;

    // Increase poll count & total deposit amount
    state.poll_count += 1;
    state.total_deposit += deposit_amount;

    let execute_data = if let Some(execute_msg) = execute_msg {
        Some(ExecuteData {
            contract: deps.api.canonical_address(&execute_msg.contract)?,
            msg: execute_msg.msg,
        })
    } else {
        None
    };

    let sender_address_raw = deps.api.canonical_address(&proposer)?;
    let new_poll = Poll {
        id: poll_id,
        creator: sender_address_raw,
        status: PollStatus::InProgress,
        yes_votes: Uint128::zero(),
        no_votes: Uint128::zero(),
        end_height: env.block.height + config.voting_period,
        title,
        description,
        link,
        execute_data,
        deposit_amount,
        total_balance_at_end_poll: None,
    };

    poll_store(&mut deps.storage).save(&poll_id.to_be_bytes(), &new_poll)?;
    poll_indexer_store(&mut deps.storage, &PollStatus::InProgress)
        .save(&poll_id.to_be_bytes(), &true)?;

    state_store(&mut deps.storage).save(&state)?;

    let r = HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "create_poll"),
            log(
                "creator",
                deps.api.human_address(&new_poll.creator)?.as_str(),
            ),
            log("poll_id", &poll_id.to_string()),
            log("end_height", new_poll.end_height),
        ],
        data: None,
    };
    Ok(r)
}

/*
 * Ends a poll.
 */
pub fn end_poll<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    poll_id: u64,
) -> HandleResult {
    let mut a_poll: Poll = poll_store(&mut deps.storage).load(&poll_id.to_be_bytes())?;

    if a_poll.status != PollStatus::InProgress {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    if a_poll.end_height > env.block.height {
        return Err(StdError::generic_err("Voting period has not expired"));
    }

    let mut poll_status = PollStatus::Rejected;
    let mut rejected_reason = "";
    let mut passed = false;

    let mut messages: Vec<CosmosMsg> = vec![];
    let config: Config = config_read(&deps.storage).load()?;
    let mut state: State = state_read(&deps.storage).load()?;
    if state.total_share.u128() == 0 {
        return Err(StdError::generic_err("Nothing staked"));
    }

    let no = a_poll.no_votes.u128();
    let yes = a_poll.yes_votes.u128();

    let tallied_weight = yes + no;
    let staked_weight = (load_token_balance(
        &deps,
        &deps.api.human_address(&config.mirror_token)?,
        &state.contract_addr,
    )? - state.total_deposit)?;

    let quorum = Decimal::from_ratio(tallied_weight, staked_weight);
    if tallied_weight == 0 || quorum < config.quorum {
        // Quorum: More than quorum of the total staked tokens at the end of the voting
        // period need to have participated in the vote.
        rejected_reason = "Quorum not reached";
    } else {
        if Decimal::from_ratio(yes, tallied_weight) > config.threshold {
            //Threshold: More than 50% of the tokens that participated in the vote
            // (after excluding “Abstain” votes) need to have voted in favor of the proposal (“Yes”).
            poll_status = PollStatus::Passed;
            passed = true;
        } else {
            rejected_reason = "Threshold not reached";
        }

        // Refunds deposit only when quorum is reached
        if !a_poll.deposit_amount.is_zero() {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.mirror_token)?,
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: deps.api.human_address(&a_poll.creator)?,
                    amount: a_poll.deposit_amount,
                })?,
            }))
        }
    }

    // Decrease total deposit amount
    state.total_deposit = (state.total_deposit - a_poll.deposit_amount)?;
    state_store(&mut deps.storage).save(&state)?;

    // Update poll indexer
    poll_indexer_store(&mut deps.storage, &PollStatus::InProgress).remove(&a_poll.id.to_be_bytes());
    poll_indexer_store(&mut deps.storage, &poll_status).save(&a_poll.id.to_be_bytes(), &true)?;

    // Update poll status
    a_poll.status = poll_status;
    a_poll.total_balance_at_end_poll = Some(staked_weight);
    poll_store(&mut deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    // Unlock all voter tokens
    let voters = poll_all_voters(&deps.storage, poll_id)?;
    for voter in &voters {
        unlock_tokens(deps, voter, poll_id)?;
    }

    // remove all voters info
    let mut voter_store = poll_voter_store(&mut deps.storage, poll_id);
    for voter in &voters {
        voter_store.remove(&voter.as_slice());
    }

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "end_poll"),
            log("poll_id", &poll_id.to_string()),
            log("rejected_reason", rejected_reason),
            log("passed", &passed.to_string()),
        ],
        data: None,
    })
}

/*
 * Execute a msg of passed poll.
 */
pub fn execute_poll<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    poll_id: u64,
) -> HandleResult {
    let config: Config = config_read(&deps.storage).load()?;
    let mut a_poll: Poll = poll_store(&mut deps.storage).load(&poll_id.to_be_bytes())?;

    if a_poll.status != PollStatus::Passed {
        return Err(StdError::generic_err("Poll is not in passed status"));
    }

    if a_poll.end_height + config.effective_delay > env.block.height {
        return Err(StdError::generic_err("Effective delay has not expired"));
    }

    poll_indexer_store(&mut deps.storage, &PollStatus::Passed).remove(&poll_id.to_be_bytes());
    poll_indexer_store(&mut deps.storage, &PollStatus::Executed)
        .save(&poll_id.to_be_bytes(), &true)?;

    a_poll.status = PollStatus::Executed;
    poll_store(&mut deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    let mut messages: Vec<CosmosMsg> = vec![];
    if let Some(execute_data) = a_poll.execute_data {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&execute_data.contract)?,
            msg: execute_data.msg,
            send: vec![],
        }))
    } else {
        return Err(StdError::generic_err("The poll does not have execute_data"));
    }

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "execute_poll"),
            log("poll_id", poll_id.to_string()),
        ],
        data: None,
    })
}

/// ExpirePoll is used to make the poll as expired state for querying purpose
pub fn expire_poll<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    poll_id: u64,
) -> HandleResult {
    let config: Config = config_read(&deps.storage).load()?;
    let mut a_poll: Poll = poll_store(&mut deps.storage).load(&poll_id.to_be_bytes())?;

    if a_poll.status != PollStatus::Passed {
        return Err(StdError::generic_err("Poll is not in passed status"));
    }

    if a_poll.execute_data.is_none() {
        return Err(StdError::generic_err(
            "Cannot make a text proposal to expired state",
        ));
    }

    if a_poll.end_height + config.expiration_period > env.block.height {
        return Err(StdError::generic_err("Expire height has not been reached"));
    }

    poll_indexer_store(&mut deps.storage, &PollStatus::Passed).remove(&poll_id.to_be_bytes());
    poll_indexer_store(&mut deps.storage, &PollStatus::Expired)
        .save(&poll_id.to_be_bytes(), &true)?;

    a_poll.status = PollStatus::Expired;
    poll_store(&mut deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "expire_poll"),
            log("poll_id", poll_id.to_string()),
        ],
        data: None,
    })
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
    token_manager.locked_balance.retain(|(k, _)| k != &poll_id);
    bank_store(&mut deps.storage).save(voter_key, &token_manager)?;
    Ok(HandleResponse::default())
}

// finds the largest locked amount in participated polls.
fn locked_balance<S: Storage, A: Api, Q: Querier>(
    voter: &CanonicalAddr,
    deps: &mut Extern<S, A, Q>,
) -> u128 {
    let voter_key = &voter.as_slice();
    let token_manager = bank_read(&deps.storage).load(voter_key).unwrap();
    token_manager
        .locked_balance
        .iter()
        .map(|(_, v)| v.balance.u128())
        .max()
        .unwrap_or_default()
}

pub fn cast_vote<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    poll_id: u64,
    vote: VoteOption,
    amount: Uint128,
) -> HandleResult {
    let sender_address_raw = deps.api.canonical_address(&env.message.sender)?;
    let config = config_read(&deps.storage).load()?;
    let state = state_read(&deps.storage).load()?;
    if poll_id == 0 || state.poll_count < poll_id {
        return Err(StdError::generic_err("Poll does not exist"));
    }

    let mut a_poll: Poll = poll_store(&mut deps.storage).load(&poll_id.to_be_bytes())?;
    if a_poll.status != PollStatus::InProgress || env.block.height > a_poll.end_height {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    // Check the voter arleady has a vote on the poll
    if poll_voter_read(&deps.storage, poll_id)
        .load(&sender_address_raw.as_slice())
        .is_ok()
    {
        return Err(StdError::generic_err("User has already voted."));
    }

    let key = &sender_address_raw.as_slice();
    let mut token_manager = bank_read(&deps.storage).may_load(key)?.unwrap_or_default();

    // convert share to amount
    let total_share = state.total_share;
    let total_balance = (load_token_balance(
        &deps,
        &deps.api.human_address(&config.mirror_token)?,
        &state.contract_addr,
    )? - state.total_deposit)?;

    if token_manager
        .share
        .multiply_ratio(total_balance, total_share)
        < amount
    {
        return Err(StdError::generic_err(
            "User does not have enough staked tokens.",
        ));
    }

    // update tally info
    if VoteOption::Yes == vote {
        a_poll.yes_votes += amount;
    } else {
        a_poll.no_votes += amount;
    }

    let vote_info = VoterInfo {
        vote,
        balance: amount,
    };
    token_manager.participated_polls.push(poll_id);
    token_manager
        .locked_balance
        .push((poll_id, vote_info.clone()));
    bank_store(&mut deps.storage).save(key, &token_manager)?;

    // store poll voter && and update poll data
    poll_voter_store(&mut deps.storage, poll_id)
        .save(&sender_address_raw.as_slice(), &vote_info)?;
    poll_store(&mut deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    let log = vec![
        log("action", "cast_vote"),
        log("poll_id", &poll_id.to_string()),
        log("amount", &amount.to_string()),
        log("voter", &env.message.sender.as_str()),
        log("vote_option", vote_info.vote),
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
        QueryMsg::State {} => to_binary(&query_state(&deps)?),
        QueryMsg::Staker { address } => to_binary(&query_staker(deps, address)?),
        QueryMsg::Poll { poll_id } => to_binary(&query_poll(deps, poll_id)?),
        QueryMsg::Polls {
            filter,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_polls(deps, filter, start_after, limit, order_by)?),
        QueryMsg::Voters {
            poll_id,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_voters(deps, poll_id, start_after, limit, order_by)?),
    }
}

fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let config: Config = config_read(&deps.storage).load()?;
    Ok(ConfigResponse {
        owner: deps.api.human_address(&config.owner)?,
        mirror_token: deps.api.human_address(&config.mirror_token)?,
        quorum: config.quorum,
        threshold: config.threshold,
        voting_period: config.voting_period,
        effective_delay: config.effective_delay,
        expiration_period: config.expiration_period,
        proposal_deposit: config.proposal_deposit,
    })
}

fn query_state<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> StdResult<StateResponse> {
    let state: State = state_read(&deps.storage).load()?;
    Ok(StateResponse {
        poll_count: state.poll_count,
        total_share: state.total_share,
        total_deposit: state.total_deposit,
    })
}

fn query_poll<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    poll_id: u64,
) -> StdResult<PollResponse> {
    let poll = match poll_read(&deps.storage).may_load(&poll_id.to_be_bytes())? {
        Some(poll) => Some(poll),
        None => return Err(StdError::generic_err("Poll does not exist")),
    }
    .unwrap();

    Ok(PollResponse {
        id: poll.id,
        creator: deps.api.human_address(&poll.creator).unwrap(),
        status: poll.status,
        end_height: poll.end_height,
        title: poll.title,
        description: poll.description,
        link: poll.link,
        deposit_amount: poll.deposit_amount,
        execute_data: if let Some(execute_data) = poll.execute_data {
            Some(ExecuteMsg {
                contract: deps.api.human_address(&execute_data.contract)?,
                msg: execute_data.msg,
            })
        } else {
            None
        },
        yes_votes: poll.yes_votes,
        no_votes: poll.no_votes,
        total_balance_at_end_poll: poll.total_balance_at_end_poll,
    })
}

fn query_polls<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    filter: Option<PollStatus>,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<PollsResponse> {
    let polls = read_polls(&deps.storage, filter, start_after, limit, order_by)?;
    let poll_responses: StdResult<Vec<PollResponse>> = polls
        .iter()
        .map(|poll| {
            Ok(PollResponse {
                id: poll.id,
                creator: deps.api.human_address(&poll.creator).unwrap(),
                status: poll.status.clone(),
                end_height: poll.end_height,
                title: poll.title.to_string(),
                description: poll.description.to_string(),
                link: poll.link.clone(),
                deposit_amount: poll.deposit_amount,
                execute_data: if let Some(execute_data) = poll.execute_data.clone() {
                    Some(ExecuteMsg {
                        contract: deps.api.human_address(&execute_data.contract)?,
                        msg: execute_data.msg,
                    })
                } else {
                    None
                },
                yes_votes: poll.yes_votes,
                no_votes: poll.no_votes,
                total_balance_at_end_poll: poll.total_balance_at_end_poll,
            })
        })
        .collect();

    Ok(PollsResponse {
        polls: poll_responses?,
    })
}

fn query_voters<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    poll_id: u64,
    start_after: Option<HumanAddr>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<VotersResponse> {
    let voters = if let Some(start_after) = start_after {
        read_poll_voters(
            &deps.storage,
            poll_id,
            Some(deps.api.canonical_address(&start_after)?),
            limit,
            order_by,
        )?
    } else {
        read_poll_voters(&deps.storage, poll_id, None, limit, order_by)?
    };

    let voters_response: StdResult<Vec<VotersResponseItem>> = voters
        .iter()
        .map(|voter_info| {
            Ok(VotersResponseItem {
                voter: deps.api.human_address(&voter_info.0)?,
                vote: voter_info.1.vote.clone(),
                balance: voter_info.1.balance,
            })
        })
        .collect();

    Ok(VotersResponse {
        voters: voters_response?,
    })
}

fn query_staker<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> StdResult<StakerResponse> {
    let addr_raw = deps.api.canonical_address(&address).unwrap();
    let config: Config = config_read(&deps.storage).load()?;
    let state: State = state_read(&deps.storage).load()?;
    let token_manager = bank_read(&deps.storage)
        .may_load(addr_raw.as_slice())?
        .unwrap_or_default();

    let total_balance = (load_token_balance(
        &deps,
        &deps.api.human_address(&config.mirror_token)?,
        &state.contract_addr,
    )? - state.total_deposit)?;

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
    })
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> MigrateResult {
    Ok(MigrateResponse::default())
}
