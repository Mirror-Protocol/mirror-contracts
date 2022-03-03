#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use crate::migrate::migrate_config;
use crate::querier::load_token_balance;
use crate::staking::{
    deposit_reward, query_shares, query_staker, stake_voting_rewards, stake_voting_tokens,
    withdraw_voting_rewards, withdraw_voting_tokens,
};
use crate::state::{
    bank_read, bank_store, config_read, config_store, poll_additional_params_read,
    poll_additional_params_store, poll_indexer_store, poll_read, poll_store, poll_voter_read,
    poll_voter_store, read_poll_voters, read_polls, read_tmp_poll_id, state_read, state_store,
    store_tmp_poll_id, Config, ExecuteData, Poll, PollAdditionalParams, State,
};

use cosmwasm_std::{
    attr, from_binary, to_binary, Api, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Reply, ReplyOn, Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use mirror_protocol::common::OrderBy;
use mirror_protocol::gov::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PollAdminAction,
    PollConfig, PollExecuteMsg, PollResponse, PollStatus, PollsResponse, QueryMsg, StateResponse,
    VoteOption, VoterInfo, VotersResponse, VotersResponseItem,
};

const MIN_TITLE_LENGTH: usize = 4;
const MAX_TITLE_LENGTH: usize = 64;
const MIN_DESC_LENGTH: usize = 4;
const MAX_DESC_LENGTH: usize = 256;
const MIN_LINK_LENGTH: usize = 12;
const MAX_LINK_LENGTH: usize = 128;
const MAX_POLLS_IN_PROGRESS: usize = 50;

const POLL_EXECUTE_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    validate_poll_config(&msg.default_poll_config)?;
    validate_poll_config(&msg.migration_poll_config)?;
    validate_poll_config(&msg.auth_admin_poll_config)?;
    validate_voter_weight(msg.voter_weight)?;

    let config = Config {
        mirror_token: deps.api.addr_canonicalize(&msg.mirror_token)?,
        owner: deps.api.addr_canonicalize(info.sender.as_str())?,
        effective_delay: msg.effective_delay,
        default_poll_config: msg.default_poll_config,
        migration_poll_config: msg.migration_poll_config,
        auth_admin_poll_config: msg.auth_admin_poll_config,
        voter_weight: msg.voter_weight,
        snapshot_period: msg.snapshot_period,
        admin_manager: deps.api.addr_canonicalize(&msg.admin_manager)?,
        poll_gas_limit: msg.poll_gas_limit,
    };

    let state = State {
        contract_addr: deps.api.addr_canonicalize(env.contract.address.as_str())?,
        poll_count: 0,
        total_share: Uint128::zero(),
        total_deposit: Uint128::zero(),
        pending_voting_rewards: Uint128::zero(),
    };

    config_store(deps.storage).save(&config)?;
    state_store(deps.storage).save(&state)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig {
            owner,
            effective_delay,
            default_poll_config,
            migration_poll_config,
            auth_admin_poll_config,
            voter_weight,
            snapshot_period,
            admin_manager,
            poll_gas_limit,
        } => update_config(
            deps,
            info,
            owner,
            effective_delay,
            default_poll_config,
            migration_poll_config,
            auth_admin_poll_config,
            voter_weight,
            snapshot_period,
            admin_manager,
            poll_gas_limit,
        ),
        ExecuteMsg::WithdrawVotingTokens { amount } => withdraw_voting_tokens(deps, info, amount),
        ExecuteMsg::WithdrawVotingRewards { poll_id } => {
            withdraw_voting_rewards(deps, info, poll_id)
        }
        ExecuteMsg::StakeVotingRewards { poll_id } => stake_voting_rewards(deps, info, poll_id),
        ExecuteMsg::CastVote {
            poll_id,
            vote,
            amount,
        } => cast_vote(deps, env, info, poll_id, vote, amount),
        ExecuteMsg::EndPoll { poll_id } => end_poll(deps, env, poll_id),
        ExecuteMsg::ExecutePoll { poll_id } => execute_poll(deps, env, poll_id),
        ExecuteMsg::SnapshotPoll { poll_id } => snapshot_poll(deps, env, poll_id),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    // only asset contract can execute this message
    let config: Config = config_read(deps.storage).load()?;
    if config.mirror_token != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::StakeVotingTokens {}) => {
            stake_voting_tokens(deps, cw20_msg.sender, cw20_msg.amount)
        }
        Ok(Cw20HookMsg::CreatePoll {
            title,
            description,
            link,
            execute_msg,
            admin_action,
        }) => create_poll(
            deps,
            env,
            cw20_msg.sender,
            cw20_msg.amount,
            title,
            description,
            link,
            execute_msg,
            admin_action,
        ),
        Ok(Cw20HookMsg::DepositReward {}) => deposit_reward(deps, cw20_msg.amount),
        Err(_) => Err(StdError::generic_err("invalid cw20 hook message")),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        POLL_EXECUTE_REPLY_ID => {
            let poll_id: u64 = read_tmp_poll_id(deps.storage)?;
            failed_poll(deps, poll_id)
        }
        _ => Err(StdError::generic_err("reply id is invalid")),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    effective_delay: Option<u64>,
    default_poll_config: Option<PollConfig>,
    migration_poll_config: Option<PollConfig>,
    auth_admin_poll_config: Option<PollConfig>,
    voter_weight: Option<Decimal>,
    snapshot_period: Option<u64>,
    admin_manager: Option<String>,
    poll_gas_limit: Option<u64>,
) -> StdResult<Response> {
    let api = deps.api;
    config_store(deps.storage).update(|mut config| {
        if config.owner != api.addr_canonicalize(info.sender.as_str())? {
            return Err(StdError::generic_err("unauthorized"));
        }

        if let Some(owner) = owner {
            config.owner = api.addr_canonicalize(&owner)?;
        }

        if let Some(default_poll_config) = default_poll_config {
            validate_poll_config(&default_poll_config)?;
            config.default_poll_config = default_poll_config;
        }

        if let Some(migration_poll_config) = migration_poll_config {
            validate_poll_config(&migration_poll_config)?;
            config.migration_poll_config = migration_poll_config;
        }

        if let Some(auth_admin_poll_config) = auth_admin_poll_config {
            validate_poll_config(&auth_admin_poll_config)?;
            config.auth_admin_poll_config = auth_admin_poll_config;
        }

        if let Some(effective_delay) = effective_delay {
            config.effective_delay = effective_delay;
        }

        if let Some(voter_weight) = voter_weight {
            validate_voter_weight(voter_weight)?;
            config.voter_weight = voter_weight;
        }

        if let Some(snapshot_period) = snapshot_period {
            config.snapshot_period = snapshot_period;
        }

        if let Some(admin_manager) = admin_manager {
            config.admin_manager = api.addr_canonicalize(&admin_manager)?;
        }

        if let Some(poll_gas_limit) = poll_gas_limit {
            config.poll_gas_limit = poll_gas_limit;
        }

        Ok(config)
    })?;
    Ok(Response::default())
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

fn validate_poll_config(poll_config: &PollConfig) -> StdResult<()> {
    validate_quorum(poll_config.quorum)?;
    validate_threshold(poll_config.threshold)?;

    Ok(())
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

pub fn validate_voter_weight(voter_weight: Decimal) -> StdResult<()> {
    if voter_weight >= Decimal::one() {
        Err(StdError::generic_err("voter_weight must be smaller than 1"))
    } else {
        Ok(())
    }
}

pub fn validate_migrations(api: &dyn Api, migrations: &[(String, u64, Binary)]) -> StdResult<()> {
    for (addr, _, _) in migrations.iter() {
        api.addr_validate(addr)?;
    }
    Ok(())
}

/*
 * Creates a new poll
 */
#[allow(clippy::too_many_arguments)]
pub fn create_poll(
    deps: DepsMut,
    env: Env,
    proposer: String,
    deposit_amount: Uint128,
    title: String,
    description: String,
    link: Option<String>,
    poll_execute_msg: Option<PollExecuteMsg>,
    poll_admin_action: Option<PollAdminAction>,
) -> StdResult<Response> {
    validate_title(&title)?;
    validate_description(&description)?;
    validate_link(&link)?;

    let config: Config = config_store(deps.storage).load()?;
    let current_seconds = env.block.time.seconds();
    let (proposal_deposit, end_time, max_polls_in_progress) = match poll_admin_action.clone() {
        None => (
            config.default_poll_config.proposal_deposit,
            current_seconds + config.default_poll_config.voting_period,
            MAX_POLLS_IN_PROGRESS,
        ),
        Some(PollAdminAction::ExecuteMigrations { migrations }) => {
            // check that contract addresses are valid
            validate_migrations(deps.api, &migrations)?;

            (
                config.migration_poll_config.proposal_deposit,
                current_seconds + config.migration_poll_config.voting_period,
                MAX_POLLS_IN_PROGRESS + 10usize, // increase maximum to prevent mailcious users from authorizing migrations
            )
        }
        // all other admin actions have the most restrictive parameters
        _ => (
            config.auth_admin_poll_config.proposal_deposit,
            current_seconds + config.auth_admin_poll_config.voting_period,
            MAX_POLLS_IN_PROGRESS + 10usize,
        ),
    };

    if deposit_amount < proposal_deposit {
        return Err(StdError::generic_err(format!(
            "Must deposit more than {} token",
            proposal_deposit
        )));
    }

    let polls_in_progress: usize = read_polls(
        deps.storage,
        Some(PollStatus::InProgress),
        None,
        None,
        None,
        Some(true),
    )?
    .len();
    if polls_in_progress.gt(&max_polls_in_progress) {
        return Err(StdError::generic_err("Too many polls in progress"));
    }

    let mut state: State = state_store(deps.storage).load()?;
    let poll_id = state.poll_count + 1;

    // Increase poll count & total deposit amount
    state.poll_count += 1;
    state.total_deposit += deposit_amount;

    let poll_execute_data = if let Some(poll_execute_msg) = poll_execute_msg {
        if poll_admin_action.is_some() {
            return Err(StdError::generic_err(
                "Can not make a poll with normal action and admin action",
            ));
        }
        let target_contract = deps.api.addr_canonicalize(&poll_execute_msg.contract)?;
        let contract_raw = deps.api.addr_canonicalize(env.contract.address.as_str())?;
        if target_contract.eq(&config.admin_manager) || target_contract.eq(&contract_raw) {
            return Err(StdError::generic_err(
                "Can not make a normal pool targeting the admin_manager or gov contract",
            ));
        }
        Some(ExecuteData {
            contract: target_contract,
            msg: poll_execute_msg.msg,
        })
    } else {
        None
    };

    let sender_address_raw = deps.api.addr_canonicalize(&proposer)?;
    let new_poll = Poll {
        id: poll_id,
        creator: sender_address_raw,
        status: PollStatus::InProgress,
        yes_votes: Uint128::zero(),
        no_votes: Uint128::zero(),
        abstain_votes: Uint128::zero(),
        end_time,
        title,
        description,
        link,
        execute_data: poll_execute_data,
        deposit_amount,
        total_balance_at_end_poll: None,
        voters_reward: Uint128::zero(),
        staked_amount: None,
    };

    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &new_poll)?;
    poll_indexer_store(deps.storage, &PollStatus::InProgress)
        .save(&poll_id.to_be_bytes(), &true)?;

    if let Some(poll_admin_action) = poll_admin_action {
        poll_additional_params_store(deps.storage).save(
            &poll_id.to_be_bytes(),
            &PollAdditionalParams {
                admin_action: poll_admin_action,
            },
        )?;
    }

    state_store(deps.storage).save(&state)?;

    let r = Response::new().add_attributes(vec![
        attr("action", "create_poll"),
        attr(
            "creator",
            deps.api.addr_humanize(&new_poll.creator)?.as_str(),
        ),
        attr("poll_id", &poll_id.to_string()),
        attr("end_time", new_poll.end_time.to_string()),
    ]);
    Ok(r)
}

/*
 * Ends a poll.
 */
pub fn end_poll(deps: DepsMut, env: Env, poll_id: u64) -> StdResult<Response> {
    let config: Config = config_store(deps.storage).load()?;
    let mut a_poll: Poll = poll_store(deps.storage).load(&poll_id.to_be_bytes())?;
    let (target_quorum, target_threshold, is_fast_track) =
        match poll_additional_params_read(deps.storage).load(&poll_id.to_be_bytes()) {
            Ok(params) => match params.admin_action {
                PollAdminAction::ExecuteMigrations { .. } => (
                    config.migration_poll_config.quorum,
                    config.migration_poll_config.threshold,
                    true,
                ),
                _ => (
                    config.auth_admin_poll_config.quorum,
                    config.auth_admin_poll_config.threshold,
                    false,
                ),
            },
            _ => (
                config.default_poll_config.quorum,
                config.default_poll_config.threshold,
                false,
            ),
        };

    if a_poll.status != PollStatus::InProgress {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    let current_seconds = env.block.time.seconds();
    if a_poll.end_time > current_seconds && !is_fast_track {
        return Err(StdError::generic_err("Voting period has not expired"));
    }

    let no = a_poll.no_votes.u128();
    let yes = a_poll.yes_votes.u128();
    let abstain = a_poll.abstain_votes.u128();

    let tallied_weight = yes + no + abstain;

    let mut poll_status = PollStatus::Rejected;
    let mut rejected_reason = "";
    let mut passed = false;

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut state: State = state_read(deps.storage).load()?;

    let (quorum, staked_weight) = if state.total_share.u128() == 0 {
        (Decimal::zero(), Uint128::zero())
    } else if let Some(staked_amount) = a_poll.staked_amount {
        (
            Decimal::from_ratio(tallied_weight, staked_amount),
            staked_amount,
        )
    } else {
        let total_locked_balance = state.total_deposit + state.pending_voting_rewards;
        let staked_weight = load_token_balance(
            &deps.querier,
            deps.api.addr_humanize(&config.mirror_token)?.to_string(),
            &state.contract_addr,
        )?
        .checked_sub(total_locked_balance)?;
        (
            Decimal::from_ratio(tallied_weight, staked_weight),
            staked_weight,
        )
    };

    if tallied_weight == 0 || quorum < target_quorum {
        // Quorum: More than quorum of the total staked tokens at the end of the voting
        // period need to have participated in the vote.
        rejected_reason = "Quorum not reached";
    } else {
        if yes != 0u128 && Decimal::from_ratio(yes, yes + no) > target_threshold {
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
                contract_addr: deps.api.addr_humanize(&config.mirror_token)?.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: deps.api.addr_humanize(&a_poll.creator)?.to_string(),
                    amount: a_poll.deposit_amount,
                })?,
            }))
        }
    }

    // if the poll is fast track and is rejected, we return error instead of updating state
    // the poll can still pass until the poll end_time
    if poll_status.eq(&PollStatus::Rejected) && is_fast_track && a_poll.end_time > current_seconds {
        return Err(StdError::generic_err(
            "Fastrack poll has not reached the target quorum or threshold",
        ));
    }

    // Decrease total deposit amount
    state.total_deposit = state.total_deposit.checked_sub(a_poll.deposit_amount)?;
    state_store(deps.storage).save(&state)?;

    // Update poll indexer
    poll_indexer_store(deps.storage, &PollStatus::InProgress).remove(&a_poll.id.to_be_bytes());
    poll_indexer_store(deps.storage, &poll_status).save(&a_poll.id.to_be_bytes(), &true)?;

    // Update poll status
    a_poll.status = poll_status;
    a_poll.total_balance_at_end_poll = Some(staked_weight);
    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "end_poll"),
        attr("poll_id", &poll_id.to_string()),
        attr("rejected_reason", rejected_reason),
        attr("passed", &passed.to_string()),
    ]))
}

/*
 * Execute a msg of passed poll.
 */
pub fn execute_poll(deps: DepsMut, env: Env, poll_id: u64) -> StdResult<Response> {
    let config: Config = config_read(deps.storage).load()?;
    let mut a_poll: Poll = poll_store(deps.storage).load(&poll_id.to_be_bytes())?;

    let (is_fast_track, admin_msg) =
        match poll_additional_params_read(deps.storage).load(&poll_id.to_be_bytes()) {
            Ok(params) => match params.admin_action {
                PollAdminAction::ExecuteMigrations { .. } => (true, Some(params.admin_action)),
                _ => (false, Some(params.admin_action)),
            },
            _ => (false, None),
        };

    if a_poll.status != PollStatus::Passed {
        return Err(StdError::generic_err("Poll is not in passed status"));
    }

    let current_seconds = env.block.time.seconds();
    if !is_fast_track && a_poll.end_time + config.effective_delay > current_seconds {
        return Err(StdError::generic_err("Effective delay has not expired"));
    }

    poll_indexer_store(deps.storage, &PollStatus::Passed).remove(&poll_id.to_be_bytes());
    poll_indexer_store(deps.storage, &PollStatus::Executed).save(&poll_id.to_be_bytes(), &true)?;

    a_poll.status = PollStatus::Executed;
    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    // if is not possible to create a poll with both admin_msg and execute_data, only one per poll
    let execute_msg: CosmosMsg = if let Some(execute_data) = a_poll.execute_data {
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&execute_data.contract)?.to_string(),
            msg: execute_data.msg,
            funds: vec![],
        })
    } else if let Some(admin_msg) = admin_msg {
        match admin_msg {
            PollAdminAction::UpdateConfig { .. } => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&admin_msg)?,
                funds: vec![],
            }),
            _ => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.admin_manager)?.to_string(),
                msg: to_binary(&admin_msg)?,
                funds: vec![],
            }),
        }
    } else {
        return Err(StdError::generic_err("The poll does not have execute_data"));
    };

    // the execution will reply in case of failure, to mark the poll as failed
    let execute_submsg = SubMsg {
        msg: execute_msg,
        gas_limit: Some(config.poll_gas_limit),
        id: POLL_EXECUTE_REPLY_ID,
        reply_on: ReplyOn::Error,
    };

    store_tmp_poll_id(deps.storage, a_poll.id)?;

    Ok(Response::new()
        .add_submessage(execute_submsg)
        .add_attributes(vec![
            attr("action", "execute_poll"),
            attr("poll_id", poll_id.to_string()),
        ]))
}

/*
 * If the executed message of a passed poll fails, it is marked as failed
 */
pub fn failed_poll(deps: DepsMut, poll_id: u64) -> StdResult<Response> {
    let mut a_poll: Poll = poll_store(deps.storage).load(&poll_id.to_be_bytes())?;

    poll_indexer_store(deps.storage, &PollStatus::Executed).remove(&poll_id.to_be_bytes());
    poll_indexer_store(deps.storage, &PollStatus::Failed).save(&poll_id.to_be_bytes(), &true)?;

    a_poll.status = PollStatus::Failed;
    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    Ok(Response::new().add_attribute("action", "failed_poll"))
}

/*
 * User casts a vote on the provided poll id
 */
pub fn cast_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    poll_id: u64,
    vote: VoteOption,
    amount: Uint128,
) -> StdResult<Response> {
    let sender_address_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let config = config_read(deps.storage).load()?;
    let state = state_read(deps.storage).load()?;
    if poll_id == 0 || state.poll_count < poll_id {
        return Err(StdError::generic_err("Poll does not exist"));
    }

    let mut a_poll: Poll = poll_store(deps.storage).load(&poll_id.to_be_bytes())?;
    let current_seconds = env.block.time.seconds();
    if a_poll.status != PollStatus::InProgress || current_seconds > a_poll.end_time {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    // Check the voter already has a vote on the poll
    if poll_voter_read(deps.storage, poll_id)
        .load(sender_address_raw.as_slice())
        .is_ok()
    {
        return Err(StdError::generic_err("User has already voted."));
    }

    let key = &sender_address_raw.as_slice();
    let mut token_manager = bank_read(deps.storage).may_load(key)?.unwrap_or_default();

    // convert share to amount
    let total_share = state.total_share;
    let total_locked_balance = state.total_deposit + state.pending_voting_rewards;
    let total_balance = load_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.mirror_token)?.to_string(),
        &state.contract_addr,
    )?
    .checked_sub(total_locked_balance)?;

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
    match vote {
        VoteOption::Yes => a_poll.yes_votes += amount,
        VoteOption::No => a_poll.no_votes += amount,
        VoteOption::Abstain => a_poll.abstain_votes += amount,
    }

    let vote_info = VoterInfo {
        vote,
        balance: amount,
    };
    token_manager
        .locked_balance
        .push((poll_id, vote_info.clone()));
    token_manager.participated_polls = vec![];
    bank_store(deps.storage).save(key, &token_manager)?;

    // store poll voter && and update poll data
    poll_voter_store(deps.storage, poll_id).save(sender_address_raw.as_slice(), &vote_info)?;

    // processing snapshot
    let time_to_end = a_poll.end_time - current_seconds;
    if time_to_end < config.snapshot_period && a_poll.staked_amount.is_none() {
        a_poll.staked_amount = Some(total_balance);
    }

    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "cast_vote"),
        attr("poll_id", &poll_id.to_string()),
        attr("amount", &amount.to_string()),
        attr("voter", &info.sender.to_string()),
        attr("vote_option", vote_info.vote.to_string()),
    ]))
}

/*
 * SnapshotPoll is used to take a snapshot of the staked amount for quorum calculation
 */
pub fn snapshot_poll(deps: DepsMut, env: Env, poll_id: u64) -> StdResult<Response> {
    let config: Config = config_read(deps.storage).load()?;
    let mut a_poll: Poll = poll_store(deps.storage).load(&poll_id.to_be_bytes())?;

    if a_poll.status != PollStatus::InProgress {
        return Err(StdError::generic_err("Poll is not in progress"));
    }

    let current_seconds = env.block.time.seconds();
    let time_to_end = a_poll.end_time - current_seconds;

    if time_to_end > config.snapshot_period {
        return Err(StdError::generic_err("Cannot snapshot at this time"));
    }

    if a_poll.staked_amount.is_some() {
        return Err(StdError::generic_err("Snapshot has already occurred"));
    }

    // store the current staked amount for quorum calculation
    let state: State = state_store(deps.storage).load()?;

    let total_locked_balance = state.total_deposit + state.pending_voting_rewards;
    let staked_amount = load_token_balance(
        &deps.querier,
        deps.api.addr_humanize(&config.mirror_token)?.to_string(),
        &state.contract_addr,
    )?
    .checked_sub(total_locked_balance)?;

    a_poll.staked_amount = Some(staked_amount);

    poll_store(deps.storage).save(&poll_id.to_be_bytes(), &a_poll)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "snapshot_poll"),
        attr("poll_id", poll_id.to_string()),
        attr("staked_amount", staked_amount),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Staker { address } => to_binary(&query_staker(deps, address)?),
        QueryMsg::Poll { poll_id } => to_binary(&query_poll(deps, poll_id)?),
        QueryMsg::Polls {
            filter,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_polls(deps, filter, start_after, limit, order_by)?),
        QueryMsg::Voter { poll_id, address } => to_binary(&query_voter(deps, poll_id, address)?),
        QueryMsg::Voters {
            poll_id,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_voters(deps, poll_id, start_after, limit, order_by)?),
        QueryMsg::Shares {
            start_after,
            limit,
            order_by,
        } => to_binary(&query_shares(deps, start_after, limit, order_by)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = config_read(deps.storage).load()?;
    Ok(ConfigResponse {
        owner: deps.api.addr_humanize(&config.owner)?.to_string(),
        mirror_token: deps.api.addr_humanize(&config.mirror_token)?.to_string(),
        effective_delay: config.effective_delay,
        default_poll_config: config.default_poll_config,
        migration_poll_config: config.migration_poll_config,
        auth_admin_poll_config: config.auth_admin_poll_config,
        voter_weight: config.voter_weight,
        snapshot_period: config.snapshot_period,
        admin_manager: deps.api.addr_humanize(&config.admin_manager)?.to_string(),
        poll_gas_limit: config.poll_gas_limit,
    })
}

fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state: State = state_read(deps.storage).load()?;
    Ok(StateResponse {
        poll_count: state.poll_count,
        total_share: state.total_share,
        total_deposit: state.total_deposit,
        pending_voting_rewards: state.pending_voting_rewards,
    })
}

fn query_poll(deps: Deps, poll_id: u64) -> StdResult<PollResponse> {
    let poll = match poll_read(deps.storage).may_load(&poll_id.to_be_bytes())? {
        Some(poll) => poll,
        None => return Err(StdError::generic_err("Poll does not exist")),
    };
    let admin_action = poll_additional_params_read(deps.storage)
        .load(&poll_id.to_be_bytes())
        .map(|params| Some(params.admin_action))
        .unwrap_or_default();

    Ok(PollResponse {
        id: poll.id,
        creator: deps.api.addr_humanize(&poll.creator).unwrap().to_string(),
        status: poll.status,
        end_time: poll.end_time,
        title: poll.title,
        description: poll.description,
        link: poll.link,
        deposit_amount: poll.deposit_amount,
        execute_data: if let Some(execute_data) = poll.execute_data {
            Some(PollExecuteMsg {
                contract: deps.api.addr_humanize(&execute_data.contract)?.to_string(),
                msg: execute_data.msg,
            })
        } else {
            None
        },
        yes_votes: poll.yes_votes,
        no_votes: poll.no_votes,
        abstain_votes: poll.abstain_votes,
        total_balance_at_end_poll: poll.total_balance_at_end_poll,
        voters_reward: poll.voters_reward,
        staked_amount: poll.staked_amount,
        admin_action,
    })
}

fn query_polls(
    deps: Deps,
    filter: Option<PollStatus>,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<PollsResponse> {
    let polls = read_polls(deps.storage, filter, start_after, limit, order_by, None)?;
    let poll_responses: StdResult<Vec<PollResponse>> = polls
        .iter()
        .map(|poll| {
            let admin_action = poll_additional_params_read(deps.storage)
                .load(&poll.id.to_be_bytes())
                .map(|params| Some(params.admin_action))
                .unwrap_or_default();
            Ok(PollResponse {
                id: poll.id,
                creator: deps.api.addr_humanize(&poll.creator).unwrap().to_string(),
                status: poll.status.clone(),
                end_time: poll.end_time,
                title: poll.title.to_string(),
                description: poll.description.to_string(),
                link: poll.link.clone(),
                deposit_amount: poll.deposit_amount,
                execute_data: if let Some(execute_data) = poll.execute_data.clone() {
                    Some(PollExecuteMsg {
                        contract: deps.api.addr_humanize(&execute_data.contract)?.to_string(),
                        msg: execute_data.msg,
                    })
                } else {
                    None
                },
                yes_votes: poll.yes_votes,
                no_votes: poll.no_votes,
                abstain_votes: poll.abstain_votes,
                total_balance_at_end_poll: poll.total_balance_at_end_poll,
                voters_reward: poll.voters_reward,
                staked_amount: poll.staked_amount,
                admin_action,
            })
        })
        .collect();

    Ok(PollsResponse {
        polls: poll_responses?,
    })
}

fn query_voter(deps: Deps, poll_id: u64, address: String) -> StdResult<VotersResponseItem> {
    let voter: VoterInfo = poll_voter_read(deps.storage, poll_id)
        .load(deps.api.addr_canonicalize(&address)?.as_slice())?;
    Ok(VotersResponseItem {
        voter: address,
        vote: voter.vote,
        balance: voter.balance,
    })
}

fn query_voters(
    deps: Deps,
    poll_id: u64,
    start_after: Option<String>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<VotersResponse> {
    let voters = if let Some(start_after) = start_after {
        read_poll_voters(
            deps.storage,
            poll_id,
            Some(deps.api.addr_canonicalize(&start_after)?),
            limit,
            order_by,
        )?
    } else {
        read_poll_voters(deps.storage, poll_id, None, limit, order_by)?
    };

    let voters_response: StdResult<Vec<VotersResponseItem>> = voters
        .iter()
        .map(|voter_info| {
            Ok(VotersResponseItem {
                voter: deps.api.addr_humanize(&voter_info.0)?.to_string(),
                vote: voter_info.1.vote.clone(),
                balance: voter_info.1.balance,
            })
        })
        .collect();

    Ok(VotersResponse {
        voters: voters_response?,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> StdResult<Response> {
    migrate_config(
        deps,
        msg.migration_poll_config,
        msg.auth_admin_poll_config,
        msg.admin_manager,
        msg.poll_gas_limit,
    )?;

    Ok(Response::default())
}
