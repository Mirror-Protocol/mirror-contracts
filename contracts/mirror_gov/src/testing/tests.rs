use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies;
use crate::querier::load_token_balance;
use crate::state::{
    bank_read, bank_store, config_read, poll_indexer_store, poll_store, poll_voter_read,
    poll_voter_store, state_read, Config, Poll, State, TokenManager,
};

use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, coins, from_binary, to_binary, Addr, Api, CanonicalAddr, CosmosMsg, Decimal, Deps,
    DepsMut, Env, Response, StdError, SubMsg, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use mirror_protocol::common::OrderBy;
use mirror_protocol::gov::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PollExecuteMsg, PollResponse,
    PollStatus, PollsResponse, QueryMsg, SharesResponse, SharesResponseItem, StakerResponse,
    StateResponse, VoteOption, VoterInfo, VotersResponse, VotersResponseItem,
};

const VOTING_TOKEN: &str = "voting_token";
const TEST_CREATOR: &str = "creator";
const TEST_VOTER: &str = "voter1";
const TEST_VOTER_2: &str = "voter2";
const TEST_VOTER_3: &str = "voter3";
const TEST_COLLECTOR: &str = "collector";
const DEFAULT_QUORUM: u64 = 30u64;
const DEFAULT_THRESHOLD: u64 = 50u64;
const DEFAULT_VOTING_PERIOD: u64 = 10000u64;
const DEFAULT_EFFECTIVE_DELAY: u64 = 10000u64;
const DEFAULT_EXPIRATION_PERIOD: u64 = 20000u64;
const DEFAULT_PROPOSAL_DEPOSIT: u128 = 10000000000u128;
const DEFAULT_VOTER_WEIGHT: Decimal = Decimal::zero();
const DEFAULT_SNAPSHOT_PERIOD: u64 = 10u64;

fn mock_instantiate(deps: DepsMut) {
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: DEFAULT_VOTER_WEIGHT,
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };

    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps, mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");
}

fn mock_env_height(height: u64, time: u64) -> Env {
    let mut env = mock_env();
    env.block.height = height;
    env.block.time = Timestamp::from_seconds(time);
    env
}

fn init_msg() -> InstantiateMsg {
    InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: DEFAULT_VOTER_WEIGHT,
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    }
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = init_msg();
    let info = mock_info(TEST_CREATOR, &coins(2, VOTING_TOKEN));
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    let config: Config = config_read(&deps.storage).load().unwrap();
    assert_eq!(
        config,
        Config {
            mirror_token: deps.api.addr_canonicalize(VOTING_TOKEN).unwrap(),
            owner: deps.api.addr_canonicalize(TEST_CREATOR).unwrap(),
            quorum: Decimal::percent(DEFAULT_QUORUM),
            threshold: Decimal::percent(DEFAULT_THRESHOLD),
            voting_period: DEFAULT_VOTING_PERIOD,
            effective_delay: DEFAULT_EFFECTIVE_DELAY,
            expiration_period: DEFAULT_EXPIRATION_PERIOD,
            proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            voter_weight: DEFAULT_VOTER_WEIGHT,
            snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
        }
    );

    let state: State = state_read(&deps.storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::zero(),
            total_deposit: Uint128::zero(),
            pending_voting_rewards: Uint128::zero(),
        }
    );
}

#[test]
fn poll_not_found() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 });

    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Poll does not exist"),
        Err(e) => panic!("Unexpected error: {:?}", e),
        _ => panic!("Must return error"),
    }
}

#[test]
fn fails_create_poll_invalid_quorum() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("voter", &coins(11, VOTING_TOKEN));
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(101),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: DEFAULT_VOTER_WEIGHT,
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };

    let res = instantiate(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "quorum must be 0 to 1"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_create_poll_invalid_threshold() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("voter", &coins(11, VOTING_TOKEN));
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(101),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: DEFAULT_VOTER_WEIGHT,
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };

    let res = instantiate(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "threshold must be 0 to 1"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_create_poll_invalid_title() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let msg = create_poll_msg("a".to_string(), "test".to_string(), None, None);
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info.clone(), msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Title too short"),
        Err(_) => panic!("Unknown error"),
    }

    let msg = create_poll_msg(
            "0123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234".to_string(),
            "test".to_string(),
            None,
            None,
        );

    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Title too long"),
        Err(_) => panic!("Unknown error"),
    }
}

#[test]
fn fails_create_poll_invalid_description() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "a".to_string(), None, None);
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info.clone(), msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Description too short"),
        Err(_) => panic!("Unknown error"),
    }

    let msg = create_poll_msg(
            "test".to_string(),
            "0123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234".to_string(),
            None,
            None,
        );

    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Description too long"),
        Err(_) => panic!("Unknown error"),
    }
}

#[test]
fn fails_create_poll_invalid_link() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        Some("http://hih".to_string()),
        None,
    );
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info.clone(), msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Link too short"),
        Err(_) => panic!("Unknown error"),
    }

    let msg = create_poll_msg(
            "test".to_string(),
            "test".to_string(),
            Some("0123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234012345678901234567890123456789012345678901234567890123456789012340123456789012345678901234567890123456789012345678901234567890123401234567890123456789012345678901234567890123456789012345678901234".to_string()),
            None,
        );

    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Link too long"),
        Err(_) => panic!("Unknown error"),
    }
}

#[test]
fn fails_create_poll_invalid_deposit() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_CREATOR.to_string(),
        amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT - 1),
        msg: to_binary(&Cw20HookMsg::CreatePoll {
            title: "TESTTEST".to_string(),
            description: "TESTTEST".to_string(),
            link: None,
            execute_msg: None,
        })
        .unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    match execute(deps.as_mut(), mock_env(), info, msg) {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(
            msg,
            format!("Must deposit more than {} token", DEFAULT_PROPOSAL_DEPOSIT)
        ),
        Err(_) => panic!("Unknown error"),
    }
}

fn create_poll_msg(
    title: String,
    description: String,
    link: Option<String>,
    execute_msg: Option<PollExecuteMsg>,
) -> ExecuteMsg {
    ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_CREATOR.to_string(),
        amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        msg: to_binary(&Cw20HookMsg::CreatePoll {
            title,
            description,
            link,
            execute_msg,
        })
        .unwrap(),
    })
}

#[test]
fn happy_days_create_poll() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_create_poll_result(
        1,
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );
}

#[test]
fn query_polls() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        Some("http://google.com".to_string()),
        None,
    );
    let _execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    let msg = create_poll_msg("test2".to_string(), "test2".to_string(), None, None);
    let _execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![
            PollResponse {
                id: 1u64,
                creator: TEST_CREATOR.to_string(),
                status: PollStatus::InProgress,
                end_time: 10000u64,
                title: "test".to_string(),
                description: "test".to_string(),
                link: Some("http://google.com".to_string()),
                deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
                execute_data: None,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                total_balance_at_end_poll: None,
                voters_reward: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                staked_amount: None,
            },
            PollResponse {
                id: 2u64,
                creator: TEST_CREATOR.to_string(),
                status: PollStatus::InProgress,
                end_time: 10000u64,
                title: "test2".to_string(),
                description: "test2".to_string(),
                link: None,
                deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
                execute_data: None,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                total_balance_at_end_poll: None,
                voters_reward: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                staked_amount: None,
            },
        ]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: Some(1u64),
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![PollResponse {
            id: 2u64,
            creator: TEST_CREATOR.to_string(),
            status: PollStatus::InProgress,
            end_time: 10000u64,
            title: "test2".to_string(),
            description: "test2".to_string(),
            link: None,
            deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            execute_data: None,
            yes_votes: Uint128::zero(),
            no_votes: Uint128::zero(),
            total_balance_at_end_poll: None,
            voters_reward: Uint128::zero(),
            abstain_votes: Uint128::zero(),
            staked_amount: None,
        },]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: Some(2u64),
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![PollResponse {
            id: 1u64,
            creator: TEST_CREATOR.to_string(),
            status: PollStatus::InProgress,
            end_time: 10000u64,
            title: "test".to_string(),
            description: "test".to_string(),
            link: Some("http://google.com".to_string()),
            deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            execute_data: None,
            yes_votes: Uint128::zero(),
            no_votes: Uint128::zero(),
            total_balance_at_end_poll: None,
            voters_reward: Uint128::zero(),
            abstain_votes: Uint128::zero(),
            staked_amount: None,
        }]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::InProgress),
            start_after: Some(1u64),
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![PollResponse {
            id: 2u64,
            creator: TEST_CREATOR.to_string(),
            status: PollStatus::InProgress,
            end_time: 10000u64,
            title: "test2".to_string(),
            description: "test2".to_string(),
            link: None,
            deposit_amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            execute_data: None,
            yes_votes: Uint128::zero(),
            no_votes: Uint128::zero(),
            total_balance_at_end_poll: None,
            voters_reward: Uint128::zero(),
            abstain_votes: Uint128::zero(),
            staked_amount: None,
        },]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Passed),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls, vec![]);
}

#[test]
fn create_poll_no_quorum() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );
}

#[test]
fn fails_end_poll_before_end_time() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(DEFAULT_VOTING_PERIOD, value.end_time);

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let env = mock_env_height(0, 0);
    let info = mock_info(TEST_CREATOR, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg);

    match execute_res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Voting period has not expired"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_end_poll() {
    const POLL_START_TIME: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    let mut creator_env = mock_env_height(0, POLL_START_TIME);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();
    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz.clone(),
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .nanos()
            / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, POLL_START_TIME);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    // not in passed status
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap_err();
    match execute_res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "Poll is not in passed status"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD);

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // End poll will withdraw deposit balance
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(stake_amount as u128),
        )],
    )]);

    // effective delay has not expired
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap_err();
    match execute_res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "Effective delay has not expired"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_EFFECTIVE_DELAY);
    let msg = ExecuteMsg::ExecutePoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
            funds: vec![],
        })),]
    );
    assert_eq!(
        execute_res.attributes,
        vec![attr("action", "execute_poll"), attr("poll_id", "1"),]
    );

    // Query executed polls
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Passed),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::InProgress),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Executed),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 1);

    // staker locked token must disappeared
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let response: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        StakerResponse {
            balance: Uint128::new(stake_amount),
            share: Uint128::new(stake_amount),
            locked_balance: vec![],
            pending_voting_rewards: Uint128::zero(),
            withdrawable_polls: vec![],
        }
    );
}

#[test]
fn expire_poll() {
    const POLL_START_TIME: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    let mut creator_env = mock_env_height(0, POLL_START_TIME);
    let creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();
    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .nanos()
            / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, POLL_START_TIME);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    // Poll is not in passed status
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_EFFECTIVE_DELAY);
    let msg = ExecuteMsg::ExpirePoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    );
    match execute_res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Poll is not in passed status"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // Expiration period has not been passed
    let msg = ExecuteMsg::ExpirePoll { poll_id: 1 };
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    );
    match execute_res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Expiration time has not been reached")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    creator_env.block.time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_EXPIRATION_PERIOD);
    let msg = ExecuteMsg::ExpirePoll { poll_id: 1 };
    let _execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let poll_res: PollResponse = from_binary(&res).unwrap();
    assert_eq!(poll_res.status, PollStatus::Expired);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Expired),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let polls_res: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(polls_res.polls[0], poll_res);
}

#[test]
fn end_poll_zero_quorum() {
    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());
    let mut creator_env = mock_env_height(1000, 10000);
    let mut creator_info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Burn {
                amount: Uint128::new(123),
            })
            .unwrap(),
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .nanos()
            / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );
    let stake_amount = 100;
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(100u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD);

    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );

    assert_eq!(execute_res.messages.len(), 0usize);

    // Query rejected polls
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Rejected),
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 1);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::InProgress),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: Some(PollStatus::Passed),
            start_after: None,
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(response.polls.len(), 0);
}

#[test]
fn end_poll_quorum_rejected() {
    let mut deps = mock_dependencies(&coins(100, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let mut creator_env = mock_env();
    let mut creator_info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    let end_time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD)
        .nanos()
        / 1_000_000_000u64;
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_time", end_time.to_string()),
        ]
    );

    let stake_amount = 100;
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(100u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(10u128),
    };
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", "1"),
            attr("amount", "10"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD);

    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn end_poll_quorum_rejected_noting_staked() {
    let mut deps = mock_dependencies(&coins(100, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let creator_env = mock_env();
    let mut creator_info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    let end_time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD)
        .nanos()
        / 1_000_000_000u64;
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_time", end_time.to_string()),
        ]
    );

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    let env = mock_env_height(0, end_time);

    let execute_res = execute(deps.as_mut(), env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn end_poll_nay_rejected() {
    let voter1_stake = 100;
    let voter2_stake = 1000;
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    let mut creator_env = mock_env();
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    let end_time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD)
        .nanos()
        / 1_000_000_000u64;
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_time", end_time.to_string()),
        ]
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((voter1_stake + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(voter1_stake as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        voter1_stake,
        DEFAULT_PROPOSAL_DEPOSIT,
        voter1_stake,
        1,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((voter1_stake + voter2_stake + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(voter2_stake as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        voter1_stake + voter2_stake,
        DEFAULT_PROPOSAL_DEPOSIT,
        voter2_stake,
        1,
        execute_res,
        deps.as_ref(),
    );

    let info = mock_info(TEST_VOTER_2, &[]);
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::No,
        amount: Uint128::from(voter2_stake),
    };
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_2, voter2_stake, 1, VoteOption::No, execute_res);

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD);
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Threshold not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn fails_cast_vote_not_enough_staked() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());
    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(10u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(10u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        10,
        DEFAULT_PROPOSAL_DEPOSIT,
        10,
        1,
        execute_res,
        deps.as_ref(),
    );

    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(11u128),
    };

    let res = execute(deps.as_mut(), env, info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "User does not have enough staked tokens.")
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_cast_vote() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(11u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        11,
        DEFAULT_PROPOSAL_DEPOSIT,
        11,
        1,
        execute_res,
        deps.as_ref(),
    );

    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let amount = 10u128;
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, amount, 1, VoteOption::Yes, execute_res);

    // balance be double
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(22u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    // Query staker
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let response: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        StakerResponse {
            balance: Uint128::new(22u128),
            share: Uint128::new(11u128),
            locked_balance: vec![(
                1u64,
                VoterInfo {
                    vote: VoteOption::Yes,
                    balance: Uint128::from(amount),
                }
            )],
            pending_voting_rewards: Uint128::zero(),
            withdrawable_polls: vec![],
        }
    );

    // Query voters
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.voters,
        vec![VotersResponseItem {
            voter: TEST_VOTER.to_string(),
            vote: VoteOption::Yes,
            balance: Uint128::from(amount),
        }]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Voters {
            poll_id: 1u64,
            start_after: Some(TEST_VOTER.to_string()),
            limit: None,
            order_by: None,
        },
    )
    .unwrap();
    let response: VotersResponse = from_binary(&res).unwrap();
    assert_eq!(response.voters.len(), 0);
}

#[test]
fn happy_days_withdraw_voting_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, 0, 11, 0, execute_res, deps.as_ref());

    let state: State = state_read(&deps.storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::from(11u128),
            total_deposit: Uint128::zero(),
            pending_voting_rewards: Uint128::zero(),
        }
    );

    // double the balance, only half will be withdrawn
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(22u128))],
    )]);

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = execute_res.messages.get(0).expect("no message");

    assert_eq!(
        msg,
        &SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(11u128),
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    let state: State = state_read(&deps.storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::from(6u128),
            total_deposit: Uint128::zero(),
            pending_voting_rewards: Uint128::zero(),
        }
    );
}

#[test]
fn happy_days_withdraw_voting_tokens_all() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, 0, 11, 0, execute_res, deps.as_ref());

    let state: State = state_read(&deps.storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::from(11u128),
            total_deposit: Uint128::zero(),
            pending_voting_rewards: Uint128::zero(),
        }
    );

    // double the balance, all balance withdrawn
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(22u128))],
    )]);

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens { amount: None };

    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = execute_res.messages.get(0).expect("no message");

    assert_eq!(
        msg,
        &SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(22u128),
            })
            .unwrap(),
            funds: vec![],
        }))
    );

    let state: State = state_read(&deps.storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 0,
            total_share: Uint128::zero(),
            total_deposit: Uint128::zero(),
            pending_voting_rewards: Uint128::zero(),
        }
    );
}

#[test]
fn withdraw_voting_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, 0, 11, 0, execute_res, deps.as_ref());

    // make fake polls; one in progress & one in passed
    poll_store(&mut deps.storage)
        .save(
            &1u64.to_be_bytes(),
            &Poll {
                id: 1u64,
                creator: CanonicalAddr::from(vec![]),
                status: PollStatus::InProgress,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                end_time: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                total_balance_at_end_poll: None,
                voters_reward: Uint128::zero(),
                staked_amount: None,
            },
        )
        .unwrap();

    poll_store(&mut deps.storage)
        .save(
            &2u64.to_be_bytes(),
            &Poll {
                id: 1u64,
                creator: CanonicalAddr::from(vec![]),
                status: PollStatus::Passed,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                end_time: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                total_balance_at_end_poll: None,
                voters_reward: Uint128::zero(),
                staked_amount: None,
            },
        )
        .unwrap();

    let voter_addr_raw = deps.api.addr_canonicalize(TEST_VOTER).unwrap();
    poll_voter_store(&mut deps.storage, 1u64)
        .save(
            voter_addr_raw.as_slice(),
            &VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::new(5u128),
            },
        )
        .unwrap();
    poll_voter_store(&mut deps.storage, 2u64)
        .save(
            voter_addr_raw.as_slice(),
            &VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::new(5u128),
            },
        )
        .unwrap();
    bank_store(&mut deps.storage)
        .save(
            voter_addr_raw.as_slice(),
            &TokenManager {
                share: Uint128::new(11u128),
                locked_balance: vec![
                    (
                        1u64,
                        VoterInfo {
                            vote: VoteOption::Yes,
                            balance: Uint128::new(5u128),
                        },
                    ),
                    (
                        2u64,
                        VoterInfo {
                            vote: VoteOption::Yes,
                            balance: Uint128::new(5u128),
                        },
                    ),
                ],
                participated_polls: vec![],
            },
        )
        .unwrap();

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(5u128)),
    };

    let _ = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let voter = poll_voter_read(&deps.storage, 1u64)
        .load(voter_addr_raw.as_slice())
        .unwrap();
    assert_eq!(
        voter,
        VoterInfo {
            vote: VoteOption::Yes,
            balance: Uint128::new(5u128),
        }
    );

    let token_manager = bank_read(&deps.storage)
        .load(voter_addr_raw.as_slice())
        .unwrap();
    assert_eq!(
        token_manager.locked_balance,
        vec![(
            1u64,
            VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::new(5u128),
            }
        )]
    );
}

#[test]
fn fails_withdraw_voting_tokens_no_stake() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Nothing staked"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_withdraw_too_many_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(10u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(10u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(10, 0, 10, 0, execute_res, deps.as_ref());

    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "User is trying to withdraw too many tokens.")
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_cast_vote_twice() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_create_poll_result(
        1,
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(11u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        11,
        DEFAULT_PROPOSAL_DEPOSIT,
        11,
        1,
        execute_res,
        deps.as_ref(),
    );

    let amount = 1u128;
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, amount, 1, VoteOption::Yes, execute_res);

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };
    let res = execute(deps.as_mut(), env, info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "User has already voted."),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_cast_vote_without_poll() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let msg = ExecuteMsg::CastVote {
        poll_id: 0,
        vote: VoteOption::Yes,
        amount: Uint128::from(1u128),
    };
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));

    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Poll does not exist"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_stake_voting_tokens() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(11, 0, 11, 0, execute_res, deps.as_ref());
}

#[test]
fn fails_insufficient_funds() {
    let mut deps = mock_dependencies(&[]);

    // initialize the store
    let msg = init_msg();
    let info = mock_info(TEST_VOTER, &coins(2, VOTING_TOKEN));
    let init_res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // insufficient token
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(0u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Insufficient funds sent"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_staking_wrong_token() {
    let mut deps = mock_dependencies(&[]);

    // initialize the store
    let msg = init_msg();
    let info = mock_info(TEST_VOTER, &coins(2, VOTING_TOKEN));
    let init_res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(11u128))],
    )]);

    // wrong token
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(&(VOTING_TOKEN.to_string() + "2"), &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn share_calculation() {
    let mut deps = mock_dependencies(&[]);

    // initialize the store
    let msg = init_msg();
    let info = mock_info(TEST_VOTER, &coins(2, VOTING_TOKEN));
    let init_res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // create 100 share
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(100u128))],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg);

    // add more balance(100) to make share:balance = 1:2
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(200u128 + 100u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "staking"),
            attr("sender", TEST_VOTER),
            attr("share", "50"),
            attr("amount", "100"),
        ]
    );

    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::new(100u128)),
    };
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw"),
            attr("recipient", TEST_VOTER),
            attr("amount", "100"),
        ]
    );

    // 100 tokens withdrawn
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::new(200u128))],
    )]);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let stake_info: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(stake_info.share, Uint128::new(100));
    assert_eq!(stake_info.balance, Uint128::new(200));
    assert_eq!(stake_info.locked_balance, vec![]);
}

#[test]
fn share_calculation_with_voter_rewards() {
    let mut deps = mock_dependencies(&[]);

    // initialize the store
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: Decimal::percent(50), // distribute 50% rewards to voters
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };
    let info = mock_info(TEST_VOTER, &coins(2, VOTING_TOKEN));
    let init_res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // create poll
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_create_poll_result(
        1,
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    // create 100 share
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(DEFAULT_PROPOSAL_DEPOSIT + 100u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "staking"),
            attr("sender", TEST_VOTER),
            attr("share", "100"),
            attr("amount", "100"),
        ]
    );

    // add more balance through dept reward, 50% reserved for voters
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(DEFAULT_PROPOSAL_DEPOSIT + 400u128 + 100u128),
        )],
    )]);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_COLLECTOR.to_string(),
        amount: Uint128::from(400u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {}).unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(DEFAULT_PROPOSAL_DEPOSIT + 400u128 + 100u128),
        )],
    )]);
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "staking"),
            attr("sender", TEST_VOTER),
            attr("share", "50"),
            attr("amount", "100"),
        ]
    );

    let msg = ExecuteMsg::WithdrawVotingTokens {
        amount: Some(Uint128::new(100u128)),
    };
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw"),
            attr("recipient", TEST_VOTER),
            attr("amount", "100"),
        ]
    );

    // 100 tokens withdrawn
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(DEFAULT_PROPOSAL_DEPOSIT + 400u128),
        )],
    )]);

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let stake_info: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(stake_info.share, Uint128::new(100));
    assert_eq!(stake_info.balance, Uint128::new(200));
    assert_eq!(stake_info.locked_balance, vec![]);
}

// helper to confirm the expected create_poll response
fn assert_create_poll_result(
    poll_id: u64,
    end_time: u64,
    creator: &str,
    execute_res: Response,
    deps: Deps,
) {
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", creator),
            attr("poll_id", poll_id.to_string()),
            attr("end_time", end_time.to_string()),
        ]
    );

    //confirm poll count
    let state: State = state_read(deps.storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count: 1,
            total_share: Uint128::zero(),
            total_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            pending_voting_rewards: Uint128::zero(),
        }
    );
}

fn assert_stake_tokens_result(
    total_share: u128,
    total_deposit: u128,
    new_share: u128,
    poll_count: u64,
    execute_res: Response,
    deps: Deps,
) {
    assert_eq!(
        execute_res.attributes.get(2).expect("no log"),
        &attr("share", new_share.to_string())
    );

    let state: State = state_read(deps.storage).load().unwrap();
    assert_eq!(
        state,
        State {
            contract_addr: deps.api.addr_canonicalize(MOCK_CONTRACT_ADDR).unwrap(),
            poll_count,
            total_share: Uint128::new(total_share),
            total_deposit: Uint128::new(total_deposit),
            pending_voting_rewards: Uint128::zero(),
        }
    );
}

fn assert_cast_vote_success(
    voter: &str,
    amount: u128,
    poll_id: u64,
    vote_option: VoteOption,
    execute_res: Response,
) {
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", poll_id.to_string()),
            attr("amount", amount.to_string()),
            attr("voter", voter),
            attr("vote_option", vote_option.to_string()),
        ]
    );
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    // update owner
    let info = mock_info(TEST_CREATOR, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("addr0001".to_string()),
        quorum: None,
        threshold: None,
        voting_period: None,
        effective_delay: None,
        expiration_period: None,
        proposal_deposit: None,
        voter_weight: None,
        snapshot_period: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!(Decimal::percent(DEFAULT_QUORUM), config.quorum);
    assert_eq!(Decimal::percent(DEFAULT_THRESHOLD), config.threshold);
    assert_eq!(DEFAULT_VOTING_PERIOD, config.voting_period);
    assert_eq!(DEFAULT_EFFECTIVE_DELAY, config.effective_delay);
    assert_eq!(DEFAULT_PROPOSAL_DEPOSIT, config.proposal_deposit.u128());

    // update left items
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        quorum: Some(Decimal::percent(20)),
        threshold: Some(Decimal::percent(75)),
        voting_period: Some(20000u64),
        effective_delay: Some(20000u64),
        expiration_period: Some(30000u64),
        proposal_deposit: Some(Uint128::new(123u128)),
        voter_weight: Some(Decimal::percent(1)),
        snapshot_period: Some(60u64),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!(Decimal::percent(20), config.quorum);
    assert_eq!(Decimal::percent(75), config.threshold);
    assert_eq!(20000u64, config.voting_period);
    assert_eq!(20000u64, config.effective_delay);
    assert_eq!(30000u64, config.expiration_period);
    assert_eq!(123u128, config.proposal_deposit.u128());
    assert_eq!(Decimal::percent(1), config.voter_weight);
    assert_eq!(60u64, config.snapshot_period);

    // Unauthorzied err
    let info = mock_info(TEST_CREATOR, &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        quorum: None,
        threshold: None,
        voting_period: None,
        effective_delay: None,
        expiration_period: None,
        proposal_deposit: None,
        voter_weight: None,
        snapshot_period: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn distribute_voting_rewards() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: Decimal::percent(50), // distribute 50% rewards to voters
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };

    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time =
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64;
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_create_poll_result(
        1,
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    let stake_amount = 100u128;

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg);

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT + 100u128) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_COLLECTOR.to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // FAIL - there is no finished polls, amount to withdraw is 0, returning error
    let msg = ExecuteMsg::WithdrawVotingRewards { poll_id: None };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, StdError::generic_err("Nothing to withdraw"));

    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // SUCCESS
    let msg = ExecuteMsg::WithdrawVotingRewards { poll_id: None };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    // user can withdraw 50% of total staked (weight = 50% poll share = 100%)
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw_voting_rewards"),
            attr("recipient", TEST_VOTER),
            attr("amount", 50.to_string()),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_VOTER.to_string(),
                amount: Uint128::from(50u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // voting info has been deleted
    assert!(poll_voter_read(&deps.storage, 1u64)
        .load(
            deps.api
                .addr_canonicalize(&TEST_VOTER.to_string())
                .unwrap()
                .as_slice()
        )
        .is_err())
}

#[test]
fn stake_voting_rewards() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: Decimal::percent(50), // distribute 50% rewards to voters
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };

    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time =
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64;
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let execute_res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_create_poll_result(
        1,
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    let stake_amount = 100u128;

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT + 100u128) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_COLLECTOR.to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // FAIL - there is no finished polls, amount to withdraw is 0, returning error
    let msg = ExecuteMsg::StakeVotingRewards { poll_id: None };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, StdError::generic_err("Nothing to withdraw"));

    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + 100u128) as u128),
        )],
    )]);

    // SUCCESS
    let msg = ExecuteMsg::StakeVotingRewards { poll_id: None };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    // user can stake 50% of the deposited reward (weight = 50% poll share = 100%)
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "stake_voting_rewards"),
            attr("staker", TEST_VOTER),
            attr("share", 33.to_string()),
            attr("amount", 50.to_string()),
        ]
    );
    assert_eq!(res.messages, vec![]);

    // FAIL - already withdrawn
    let msg = ExecuteMsg::StakeVotingRewards { poll_id: None };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(res, StdError::generic_err("Nothing to withdraw"));

    // voting info has been deleted
    assert!(poll_voter_read(&deps.storage, 1u64)
        .load(
            deps.api
                .addr_canonicalize(&TEST_VOTER.to_string())
                .unwrap()
                .as_slice()
        )
        .is_err());

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: TEST_VOTER.to_string(),
        },
    )
    .unwrap();
    let response: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        StakerResponse {
            balance: Uint128::new(stake_amount + 100u128),
            share: Uint128::new(stake_amount + 33u128),
            locked_balance: vec![],
            pending_voting_rewards: Uint128::new(0u128),
            withdrawable_polls: vec![],
        }
    );
}

#[test]
fn distribute_voting_rewards_with_multiple_active_polls_and_voters() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: Decimal::percent(50), // distribute 50% rewards to voters
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };
    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");

    // create polls
    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time =
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64;
    // poll 1
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    // poll 2
    let msg = create_poll_msg("test2".to_string(), "test2".to_string(), None, None);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    const ALICE: &str = "alice";
    const ALICE_STAKE: u128 = 750_000_000u128;
    const BOB: &str = "bob";
    const BOB_STAKE: u128 = 250_000_000u128;
    const CINDY: &str = "cindy";
    const CINDY_STAKE: u128 = 250_000_000u128;

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + DEFAULT_PROPOSAL_DEPOSIT * 2) as u128),
        )],
    )]);
    // Alice stakes 750 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: ALICE.to_string(),
        amount: Uint128::from(ALICE_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + BOB_STAKE + DEFAULT_PROPOSAL_DEPOSIT * 2) as u128),
        )],
    )]);
    // Bob stakes 250 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB.to_string(),
        amount: Uint128::from(BOB_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(
                (ALICE_STAKE + BOB_STAKE + CINDY_STAKE + DEFAULT_PROPOSAL_DEPOSIT * 2) as u128,
            ),
        )],
    )]);
    // Cindy stakes 250 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: CINDY.to_string(),
        amount: Uint128::from(CINDY_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Alice votes on proposal 1
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(ALICE_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(ALICE, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Bob votes on proposals 1 and 2
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Abstain,
        amount: Uint128::from(BOB_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    let msg = ExecuteMsg::CastVote {
        poll_id: 2,
        vote: VoteOption::No,
        amount: Uint128::from(BOB_STAKE),
    };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Cindy votes on proposal 2
    let msg = ExecuteMsg::CastVote {
        poll_id: 2,
        vote: VoteOption::Abstain,
        amount: Uint128::from(CINDY_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(CINDY, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(
                (ALICE_STAKE
                    + BOB_STAKE
                    + CINDY_STAKE
                    + DEFAULT_PROPOSAL_DEPOSIT * 2
                    + 2000000000u128) as u128,
            ),
        )],
    )]);

    // Collector sends 2000 MIR with 50% voting weight
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_COLLECTOR.to_string(),
        amount: Uint128::from(2000000000u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // End the polls
    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    let msg = ExecuteMsg::EndPoll { poll_id: 2 };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::WithdrawVotingRewards { poll_id: None };
    // ALICE withdraws voting rewards
    let env = mock_env_height(0, 10000);
    let info = mock_info(ALICE, &[]);
    let res = execute(deps.as_mut(), env, info, msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw_voting_rewards"),
            attr("recipient", ALICE),
            attr("amount", 375000000.to_string()),
        ]
    );

    // BOB withdraws voting rewards
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);

    let res = execute(deps.as_mut(), env, info, msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw_voting_rewards"),
            attr("recipient", BOB),
            attr("amount", 375000000.to_string()), // 125 from poll 1 + 250 from poll 2
        ]
    );

    // CINDY
    let env = mock_env_height(0, 10000);
    let info = mock_info(CINDY, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw_voting_rewards"),
            attr("recipient", CINDY),
            attr("amount", 250000000.to_string()),
        ]
    );
}

#[test]
fn distribute_voting_rewards_only_to_polls_in_progress() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: Decimal::percent(50), // distribute 50% rewards to voters
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };
    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");

    // make fake polls; one in progress & one in passed
    poll_store(&mut deps.storage)
        .save(
            &1u64.to_be_bytes(),
            &Poll {
                id: 1u64,
                creator: deps.api.addr_canonicalize(TEST_CREATOR).unwrap(),
                status: PollStatus::InProgress,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                end_time: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                total_balance_at_end_poll: None,
                voters_reward: Uint128::zero(),
                staked_amount: None,
            },
        )
        .unwrap();

    poll_store(&mut deps.storage)
        .save(
            &2u64.to_be_bytes(),
            &Poll {
                id: 2u64,
                creator: deps.api.addr_canonicalize(TEST_CREATOR).unwrap(),
                status: PollStatus::Passed,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                end_time: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                total_balance_at_end_poll: None,
                voters_reward: Uint128::zero(),
                staked_amount: None,
            },
        )
        .unwrap();

    poll_indexer_store(&mut deps.storage, &PollStatus::InProgress)
        .save(&1u64.to_be_bytes(), &true)
        .unwrap();
    poll_indexer_store(&mut deps.storage, &PollStatus::Passed)
        .save(&2u64.to_be_bytes(), &true)
        .unwrap();

    // Collector sends 2000 MIR with 50% voting weight
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_COLLECTOR.to_string(),
        amount: Uint128::from(2000000000u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Polls {
            filter: None,
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: PollsResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.polls,
        vec![
            PollResponse {
                id: 1u64,
                creator: TEST_CREATOR.to_string(),
                status: PollStatus::InProgress,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                end_time: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                total_balance_at_end_poll: None,
                voters_reward: Uint128::from(1000000000u128),
                staked_amount: None,
            },
            PollResponse {
                id: 2u64,
                creator: TEST_CREATOR.to_string(),
                status: PollStatus::Passed,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                abstain_votes: Uint128::zero(),
                end_time: 0u64,
                title: "title".to_string(),
                description: "description".to_string(),
                deposit_amount: Uint128::zero(),
                link: None,
                execute_data: None,
                total_balance_at_end_poll: None,
                voters_reward: Uint128::zero(),
                staked_amount: None,
            },
        ]
    );
}

#[test]
fn test_staking_and_voting_rewards() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: Decimal::percent(50), // distribute 50% rewards to voters
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };
    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time =
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64;
    // poll 1
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    const ALICE: &str = "alice";
    const ALICE_STAKE: u128 = 750_000_000u128;
    const BOB: &str = "bob";
    const BOB_STAKE: u128 = 250_000_000u128;

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Alice stakes 750 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: ALICE.to_string(),
        amount: Uint128::from(ALICE_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + BOB_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Bob stakes 250 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB.to_string(),
        amount: Uint128::from(BOB_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Alice votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(ALICE_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(ALICE, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Bob votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Abstain,
        amount: Uint128::from(BOB_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(
                (ALICE_STAKE + BOB_STAKE + DEFAULT_PROPOSAL_DEPOSIT + 2_000_000_000u128) as u128,
            ),
        )],
    )]);

    // Collector sends 2000 MIR with 50% voting weight
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_COLLECTOR.to_string(),
        amount: Uint128::from(2_000_000_000u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // End the poll
    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // deposit is returned to creator and collector deposit is added
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + BOB_STAKE + 2_000_000_000) as u128),
        )],
    )]);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap();
    let response: StateResponse = from_binary(&res).unwrap();
    assert_eq!(response.total_share, Uint128::new(1_000_000_000u128));
    assert_eq!(response.total_deposit, Uint128::zero());
    assert_eq!(
        response.pending_voting_rewards,
        Uint128::new(1_000_000_000u128)
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: ALICE.to_string(),
        },
    )
    .unwrap();
    let response: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        StakerResponse {
            balance: Uint128::new(ALICE_STAKE + 750_000_000u128),
            share: Uint128::new(ALICE_STAKE),
            locked_balance: vec![],
            pending_voting_rewards: Uint128::new(750_000_000u128),
            withdrawable_polls: vec![(1u64, Uint128::new(750_000_000u128))],
        }
    );
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Staker {
            address: BOB.to_string(),
        },
    )
    .unwrap();
    let response: StakerResponse = from_binary(&res).unwrap();
    assert_eq!(
        response,
        StakerResponse {
            balance: Uint128::new(BOB_STAKE + 250_000_000u128),
            share: Uint128::new(BOB_STAKE),
            locked_balance: vec![],
            pending_voting_rewards: Uint128::new(250_000_000u128),
            withdrawable_polls: vec![(1u64, Uint128::new(250_000_000u128))],
        }
    );

    let msg = ExecuteMsg::WithdrawVotingRewards { poll_id: None };
    // ALICE withdraws voting rewards
    let env = mock_env_height(0, 10000);
    let info = mock_info(ALICE, &[]);
    let res = execute(deps.as_mut(), env, info, msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw_voting_rewards"),
            attr("recipient", ALICE),
            attr("amount", ALICE_STAKE.to_string()),
        ]
    );

    // BOB withdraws voting rewards
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw_voting_rewards"),
            attr("recipient", BOB),
            attr("amount", BOB_STAKE.to_string()),
        ]
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + BOB_STAKE + 1_000_000_000) as u128),
        )],
    )]);

    // withdraw remaining voting tokens
    let msg = ExecuteMsg::WithdrawVotingTokens { amount: None };
    let info = mock_info(ALICE, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw"),
            attr("recipient", ALICE),
            attr("amount", "1500000000"),
        ]
    );
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((BOB_STAKE + 250_000_000) as u128),
        )],
    )]);
    // withdraw remaining voting tokens
    let msg = ExecuteMsg::WithdrawVotingTokens { amount: None };
    let info = mock_info(BOB, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw"),
            attr("recipient", BOB),
            attr("amount", "500000000"),
        ]
    );
}

#[test]
fn test_abstain_votes_theshold() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: Decimal::percent(50), // distribute 50% rewards to voters
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };

    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time =
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64;
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    const ALICE: &str = "alice";
    const ALICE_STAKE: u128 = 750_000_000u128;
    const BOB: &str = "bob";
    const BOB_STAKE: u128 = 250_000_000u128;
    const CINDY: &str = "cindy";
    const CINDY_STAKE: u128 = 260_000_000u128;

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Alice stakes 750 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: ALICE.to_string(),
        amount: Uint128::from(ALICE_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + BOB_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Bob stakes 250 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB.to_string(),
        amount: Uint128::from(BOB_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(
                (ALICE_STAKE + BOB_STAKE + CINDY_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128,
            ),
        )],
    )]);
    // Cindy stakes 260 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: CINDY.to_string(),
        amount: Uint128::from(CINDY_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Alice votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Abstain,
        amount: Uint128::from(ALICE_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(ALICE, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Bob votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::No,
        amount: Uint128::from(BOB_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Cindy votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(CINDY_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(CINDY, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    // abstain votes should not affect threshold, so poll is passed
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
}

#[test]
fn test_abstain_votes_quorum() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: Decimal::percent(50), // distribute 50% rewards to voters
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };

    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time =
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64;
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    const ALICE: &str = "alice";
    const ALICE_STAKE: u128 = 750_000_000u128;
    const BOB: &str = "bob";
    const BOB_STAKE: u128 = 50_000_000u128;
    const CINDY: &str = "cindy";
    const CINDY_STAKE: u128 = 20_000_000u128;

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Alice stakes 750 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: ALICE.to_string(),
        amount: Uint128::from(ALICE_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((ALICE_STAKE + BOB_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);
    // Bob stakes 50 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: BOB.to_string(),
        amount: Uint128::from(BOB_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(
                (ALICE_STAKE + BOB_STAKE + CINDY_STAKE + DEFAULT_PROPOSAL_DEPOSIT) as u128,
            ),
        )],
    )]);
    // Cindy stakes 50 MIR
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: CINDY.to_string(),
        amount: Uint128::from(CINDY_STAKE),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Alice votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Abstain,
        amount: Uint128::from(ALICE_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(ALICE, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Bob votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(BOB_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Cindy votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(CINDY_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(CINDY, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::EndPoll { poll_id: 1 };

    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    // abstain votes make the poll surpass quorum
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );

    let env = mock_env_height(0, 10000);
    let info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));
    let poll_end_time =
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64;
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // Alice doesn't vote

    // Bob votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 2,
        vote: VoteOption::Yes,
        amount: Uint128::from(BOB_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(BOB, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();
    // Cindy votes
    let msg = ExecuteMsg::CastVote {
        poll_id: 2,
        vote: VoteOption::Yes,
        amount: Uint128::from(CINDY_STAKE),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(CINDY, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::EndPoll { poll_id: 2 };

    let env = mock_env_height(0, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    // without abstain votes, quroum is not reached
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "2"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );
}

#[test]
fn test_query_shares() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let voter_0_addr_raw = CanonicalAddr::from(vec![
        1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    let voter_1_addr_raw = CanonicalAddr::from(vec![
        1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    let voter_2_addr_raw = CanonicalAddr::from(vec![
        1, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    let voter_0 = deps.api.addr_humanize(&voter_0_addr_raw).unwrap().to_string();
    let voter_1 = deps.api.addr_humanize(&voter_1_addr_raw).unwrap().to_string();
    let voter_2 = deps.api.addr_humanize(&voter_2_addr_raw).unwrap().to_string();

    bank_store(&mut deps.storage)
        .save(
            voter_0_addr_raw.as_slice(),
            &TokenManager {
                share: Uint128::new(11u128),
                locked_balance: vec![],
                participated_polls: vec![],
            },
        )
        .unwrap();
    bank_store(&mut deps.storage)
        .save(
            voter_1_addr_raw.as_slice(),
            &TokenManager {
                share: Uint128::new(22u128),
                locked_balance: vec![],
                participated_polls: vec![],
            },
        )
        .unwrap();
    bank_store(&mut deps.storage)
        .save(
            voter_2_addr_raw.as_slice(),
            &TokenManager {
                share: Uint128::new(33u128),
                locked_balance: vec![],
                participated_polls: vec![],
            },
        )
        .unwrap();

    // query everything Asc
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Shares {
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: SharesResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.stakers,
        vec![
            SharesResponseItem {
                staker: voter_0.clone(),
                share: Uint128::new(11u128),
            },
            SharesResponseItem {
                staker: voter_1.clone(),
                share: Uint128::new(22u128),
            },
            SharesResponseItem {
                staker: voter_2.clone(),
                share: Uint128::new(33u128),
            },
        ]
    );

    // query everything Desc
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Shares {
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let response: SharesResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.stakers,
        vec![
            SharesResponseItem {
                staker: voter_2.clone(),
                share: Uint128::new(33u128),
            },
            SharesResponseItem {
                staker: voter_1.clone(),
                share: Uint128::new(22u128),
            },
            SharesResponseItem {
                staker: voter_0.clone(),
                share: Uint128::new(11u128),
            },
        ]
    );

    // limit 2
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Shares {
            start_after: None,
            limit: Some(2u32),
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: SharesResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.stakers,
        vec![
            SharesResponseItem {
                staker: voter_0,
                share: Uint128::new(11u128),
            },
            SharesResponseItem {
                staker: voter_1.clone(),
                share: Uint128::new(22u128),
            },
        ]
    );

    // start after staker0001 and limit 1
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Shares {
            start_after: Some(voter_1),
            limit: Some(1u32),
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let response: SharesResponse = from_binary(&res).unwrap();
    assert_eq!(
        response.stakers,
        vec![SharesResponseItem {
            staker: voter_2,
            share: Uint128::new(33u128),
        },]
    );
}

#[test]
fn snapshot_poll() {
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(100, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());

    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let mut creator_env = mock_env();
    let creator_info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();
    let end_time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD)
        .nanos()
        / 1_000_000_000u64;
    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "create_poll"),
            attr("creator", TEST_CREATOR),
            attr("poll_id", "1"),
            attr("end_time", end_time.to_string()),
        ]
    );

    //must not be executed
    let snapshot_err = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap_err();
    assert_eq!(
        StdError::generic_err("Cannot snapshot at this height",),
        snapshot_err
    );

    // change time
    creator_env.block.time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD - 10);

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let fix_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap();

    assert_eq!(
        fix_res.attributes,
        vec![
            attr("action", "snapshot_poll"),
            attr("poll_id", "1"),
            attr("staked_amount", stake_amount.to_string()),
        ]
    );

    //must not be executed
    let snapshot_error = execute(
        deps.as_mut(),
        creator_env,
        creator_info,
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap_err();
    assert_eq!(
        StdError::generic_err("Snapshot has already occurred"),
        snapshot_error
    );
}

#[test]
fn happy_days_cast_vote_with_snapshot() {
    let mut deps = mock_dependencies(&[]);
    mock_instantiate(deps.as_mut());

    let env = mock_env_height(0, 0);
    let info = mock_info(VOTING_TOKEN, &[]);
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_create_poll_result(
        1,
        DEFAULT_VOTING_PERIOD,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(11u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        11,
        DEFAULT_PROPOSAL_DEPOSIT,
        11,
        1,
        execute_res,
        deps.as_ref(),
    );

    //cast_vote without snapshot
    let env = mock_env_height(0, 0);
    let info = mock_info(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let amount = 10u128;

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(amount),
    };

    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, amount, 1, VoteOption::Yes, execute_res);

    // balance be double
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(22u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(value.staked_amount, None);
    let end_time = value.end_time;

    //cast another vote
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // another voter cast a vote
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(10u128),
    };
    let env = mock_env_height(0, end_time - 9);
    let info = mock_info(TEST_VOTER_2, &[]);
    let execute_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_2, amount, 1, VoteOption::Yes, execute_res);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(value.staked_amount, Some(Uint128::new(22)));

    // snanpshot poll will not go through
    let snap_error = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap_err();
    assert_eq!(
        StdError::generic_err("Snapshot has already occurred"),
        snap_error
    );

    // balance be double
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(33u128 + DEFAULT_PROPOSAL_DEPOSIT),
        )],
    )]);

    // another voter cast a vote but the snapshot is already occurred
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_3.to_string(),
        amount: Uint128::from(11u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(10u128),
    };
    let env = mock_env_height(0, end_time - 8);
    let info = mock_info(TEST_VOTER_3, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_3, amount, 1, VoteOption::Yes, execute_res);

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(value.staked_amount, Some(Uint128::new(22)));
}

#[test]
fn fails_end_poll_quorum_inflation_without_snapshot_poll() {
    const POLL_START_TIME: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());

    let mut creator_env = mock_env_height(0, POLL_START_TIME);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .nanos()
            / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, 0);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_env.block.time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD - 10);

    // did not SnapshotPoll

    // staked amount get increased 10 times
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(((10 * stake_amount) + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    //cast another vote
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(8 * stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _handle_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // another voter cast a vote
    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(creator_env.block.height, 10000);
    let info = mock_info(TEST_VOTER_2, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER_2),
            attr("vote_option", "yes"),
        ]
    );

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(10);

    // quorum must reach
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", "Quorum not reached"),
            attr("passed", "false"),
        ]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(
        10 * stake_amount,
        value.total_balance_at_end_poll.unwrap().u128()
    );
}

#[test]
fn happy_days_end_poll_with_controlled_quorum() {
    const POLL_START_TIME: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;

    let mut deps = mock_dependencies(&coins(1000, VOTING_TOKEN));
    mock_instantiate(deps.as_mut());

    let mut creator_env = mock_env_height(0, POLL_START_TIME);
    let mut creator_info = mock_info(VOTING_TOKEN, &coins(2, VOTING_TOKEN));

    let exec_msg_bz = to_binary(&Cw20ExecuteMsg::Burn {
        amount: Uint128::new(123),
    })
    .unwrap();

    let msg = create_poll_msg(
        "test".to_string(),
        "test".to_string(),
        None,
        Some(PollExecuteMsg {
            contract: VOTING_TOKEN.to_string(),
            msg: exec_msg_bz,
        }),
    );

    let execute_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        msg,
    )
    .unwrap();

    assert_create_poll_result(
        1,
        creator_env
            .block
            .time
            .plus_seconds(DEFAULT_VOTING_PERIOD)
            .nanos()
            / 1_000_000_000u64,
        TEST_CREATOR,
        execute_res,
        deps.as_ref(),
    );

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new((stake_amount + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_stake_tokens_result(
        stake_amount,
        DEFAULT_PROPOSAL_DEPOSIT,
        stake_amount,
        1,
        execute_res,
        deps.as_ref(),
    );

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, POLL_START_TIME);
    let info = mock_info(TEST_VOTER, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", "1000"),
            attr("voter", TEST_VOTER),
            attr("vote_option", "yes"),
        ]
    );

    creator_env.block.time = creator_env
        .block
        .time
        .plus_seconds(DEFAULT_VOTING_PERIOD - 10);

    // send SnapshotPoll
    let fix_res = execute(
        deps.as_mut(),
        creator_env.clone(),
        creator_info.clone(),
        ExecuteMsg::SnapshotPoll { poll_id: 1 },
    )
    .unwrap();

    assert_eq!(
        fix_res.attributes,
        vec![
            attr("action", "snapshot_poll"),
            attr("poll_id", "1"),
            attr("staked_amount", stake_amount.to_string()),
        ]
    );

    // staked amount get increased 10 times
    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::new(((10 * stake_amount) + DEFAULT_PROPOSAL_DEPOSIT) as u128),
        )],
    )]);

    //cast another vote
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER_2.to_string(),
        amount: Uint128::from(8 * stake_amount as u128),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _execute_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(8 * stake_amount),
    };
    let env = mock_env_height(creator_env.block.height, 10000);
    let info = mock_info(TEST_VOTER_2, &[]);
    let execute_res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "cast_vote"),
            attr("poll_id", POLL_ID.to_string()),
            attr("amount", "8000"),
            attr("voter", TEST_VOTER_2),
            attr("vote_option", "yes"),
        ]
    );

    creator_info.sender = Addr::unchecked(TEST_CREATOR);
    creator_env.block.time = creator_env.block.time.plus_seconds(10);

    // quorum must reach
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let execute_res = execute(deps.as_mut(), creator_env, creator_info, msg).unwrap();

    assert_eq!(
        execute_res.attributes,
        vec![
            attr("action", "end_poll"),
            attr("poll_id", "1"),
            attr("rejected_reason", ""),
            attr("passed", "true"),
        ]
    );
    assert_eq!(
        execute_res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: VOTING_TOKEN.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: TEST_CREATOR.to_string(),
                amount: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(
        stake_amount,
        value.total_balance_at_end_poll.unwrap().u128()
    );

    assert_eq!(value.yes_votes.u128(), 9 * stake_amount);

    // actual staked amount is 10 times bigger than staked amount
    let actual_staked_weight = load_token_balance(
        &deps.as_ref().querier,
        VOTING_TOKEN.to_string(),
        &deps
            .api
            .addr_canonicalize(&MOCK_CONTRACT_ADDR.to_string())
            .unwrap(),
    )
    .unwrap()
    .checked_sub(Uint128::new(DEFAULT_PROPOSAL_DEPOSIT))
    .unwrap();

    assert_eq!(actual_staked_weight.u128(), (10 * stake_amount))
}

#[test]
fn test_unstake_before_claiming_voting_rewards() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        mirror_token: VOTING_TOKEN.to_string(),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        expiration_period: DEFAULT_EXPIRATION_PERIOD,
        proposal_deposit: Uint128::new(DEFAULT_PROPOSAL_DEPOSIT),
        voter_weight: Decimal::percent(50),
        snapshot_period: DEFAULT_SNAPSHOT_PERIOD,
    };

    let info = mock_info(TEST_CREATOR, &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg)
        .expect("contract successfully handles InstantiateMsg");

    let env = mock_env_height(0, 10000);
    let msg = create_poll_msg("test".to_string(), "test".to_string(), None, None);
    let info = mock_info(VOTING_TOKEN, &[]);
    let handle_res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    let poll_end_time =
        env.block.time.plus_seconds(DEFAULT_VOTING_PERIOD).nanos() / 1_000_000_000u64;
    assert_create_poll_result(1, poll_end_time, TEST_CREATOR, handle_res, deps.as_ref());

    let stake_amount = 100u128;

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::from(stake_amount + DEFAULT_PROPOSAL_DEPOSIT as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_VOTER.to_string(),
        amount: Uint128::from(stake_amount),
        msg: to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::CastVote {
        poll_id: 1,
        vote: VoteOption::Yes,
        amount: Uint128::from(stake_amount),
    };
    let env = mock_env_height(0, 10000);
    let info = mock_info(TEST_VOTER, &[]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::from((stake_amount + DEFAULT_PROPOSAL_DEPOSIT + 100u128) as u128),
        )],
    )]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: TEST_COLLECTOR.to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {}).unwrap(),
    });

    let info = mock_info(VOTING_TOKEN, &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // END POLL
    let env = mock_env_height(10000, poll_end_time);
    let info = mock_info(TEST_VOTER, &[]);
    let msg = ExecuteMsg::EndPoll { poll_id: 1 };
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    deps.querier.with_token_balances(&[(
        &VOTING_TOKEN.to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::from((stake_amount + 100u128) as u128),
        )],
    )]);

    // UNSTAKE VOTING TOKENS
    let msg = ExecuteMsg::WithdrawVotingTokens { amount: None };
    let info = mock_info(TEST_VOTER, &[]);
    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw"),
            attr("recipient", TEST_VOTER),
            attr("amount", (stake_amount + 50u128).to_string()), // 100 + 50% of 100
        ]
    );

    let token_manager = bank_read(&deps.storage)
        .load(deps.api.addr_canonicalize(TEST_VOTER).unwrap().as_slice())
        .unwrap();
    assert_eq!(
        token_manager.locked_balance,
        vec![(
            1u64,
            VoterInfo {
                vote: VoteOption::Yes,
                balance: Uint128::from(stake_amount),
            }
        )]
    );

    // SUCCESS
    let msg = ExecuteMsg::WithdrawVotingRewards { poll_id: None };
    let env = mock_env_height(0, 10000);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    // user can withdraw 50% of total staked (weight = 50% poll share = 100%)
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw_voting_rewards"),
            attr("recipient", TEST_VOTER),
            attr("amount", 50.to_string()),
        ]
    );

    // make sure now the state is clean
    let token_manager = bank_read(&deps.storage)
        .load(deps.api.addr_canonicalize(TEST_VOTER).unwrap().as_slice())
        .unwrap();
    assert_eq!(token_manager.locked_balance, vec![]);
    // expect err
    poll_voter_read(&deps.storage, 1u64)
        .load(deps.api.addr_canonicalize(TEST_VOTER).unwrap().as_slice())
        .unwrap_err();
}
