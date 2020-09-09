//! This integration test tries to run and call the generated wasm.
//! It depends on a Wasm build being available, which you can create with `cargo wasm`.
//! Then running `cargo integration-test` will validate we can properly call into that generated Wasm.
//!
//! You can easily convert unit tests to integration tests as follows:
//! 1. Copy them over verbatim
//! 2. Then change
//!      let mut deps = mock_dependencies(20, &[]);
//!    to
//!      let mut deps = mock_instance(WASM, &[]);
//!      // now you don't mock_init anymore
//! 3. If you access raw storage, where ever you see something like:
//!      deps.storage.get(CONFIG_KEY).expect("no data stored");
//!    replace it with:
//!      deps.with_storage(|store| {
//!          let data = store.get(CONFIG_KEY).expect("no data stored");
//!          //...
//!      });
//! 4. Anywhere you see query(&deps, ...) you must replace it with query(&mut deps, ...)

use cosmwasm_std::{
    coins, from_binary, log, to_binary, CosmosMsg, Env, HandleResponse, HandleResult, HumanAddr,
    InitResponse, StdError, Uint128, WasmMsg,
};
use cosmwasm_storage::to_length_prefixed;
use cosmwasm_vm::testing::{handle, init, mock_env, mock_instance, query};
use cosmwasm_vm::{from_slice, Api, Storage};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use mirror_gov::contract::VOTING_TOKEN;
use mirror_gov::msg::{Cw20HookMsg, HandleMsg, InitMsg, PollResponse, QueryMsg};
use mirror_gov::state::State;

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/mirror_gov.wasm");
// You can uncomment this line instead to test productionified build from rust-optimizer
// static WASM: &[u8] = include_bytes!("../contract.wasm");

const DEFAULT_END_HEIGHT: u64 = 100800u64;
const TEST_CREATOR: &str = "creator";
const TEST_VOTER: &str = "voter1";
const TEST_VOTER_2: &str = "voter2";

fn mock_env_height(signer: &HumanAddr, height: u64, time: u64) -> Env {
    let mut env = mock_env(signer, &[]);
    env.block.height = height;
    env.block.time = time;
    env
}

fn init_msg() -> InitMsg {
    InitMsg {
        mirror_token: HumanAddr::from(VOTING_TOKEN),
    }
}

fn address(index: u8) -> HumanAddr {
    match index {
        0 => HumanAddr(TEST_CREATOR.to_string()), // contract initializer
        1 => HumanAddr(TEST_VOTER.to_string()),
        2 => HumanAddr(TEST_VOTER_2.to_string()),
        _ => panic!("Unsupported address index"),
    }
}

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = init_msg();
    let env = mock_env(
        &HumanAddr(TEST_CREATOR.to_string()),
        &coins(2, VOTING_TOKEN),
    );
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    let api = deps.api;

    deps.with_storage(|store| {
        let config_key_raw = to_length_prefixed(b"config");
        let state: State = from_slice(&store.get(&config_key_raw).0.unwrap().unwrap()).unwrap();
        assert_eq!(
            state,
            State {
                mirror_token: api
                    .canonical_address(&HumanAddr::from(VOTING_TOKEN))
                    .0
                    .unwrap(),
                owner: api
                    .canonical_address(&HumanAddr::from(&HumanAddr(TEST_CREATOR.to_string())))
                    .0
                    .unwrap(),
                poll_count: 0,
                staked_tokens: Uint128::zero(),
            }
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn poll_not_found() {
    let mut deps = mock_instance(WASM, &[]);

    let res = query(&mut deps, QueryMsg::Poll { poll_id: 1 });

    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Poll does not exist"),
        Err(e) => panic!("Unexpected error: {:?}", e),
        _ => panic!("Must return error"),
    }
}

#[test]
fn fails_create_poll_invalid_quorum_percentage() {
    let mut deps = mock_instance(WASM, &[]);
    let env = mock_env("voter", &coins(11, VOTING_TOKEN));

    let msg = create_poll_msg(101, "test".to_string(), None, None);

    let res: HandleResult = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "quorum_percentage must be 0 to 100")
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_create_poll_invalid_description() {
    let mut deps = mock_instance(WASM, &[]);
    let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));

    let msg: HandleMsg = create_poll_msg(30, "a".to_string(), None, None);

    let handle_res: HandleResult = handle(&mut deps, env.clone(), msg.clone());
    match handle_res.unwrap_err() {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "Description too short"),
        e => panic!("unexpected error: {:?}", e),
    }

    let msg = create_poll_msg(
        100,
        "01234567890123456789012345678901234567890123456789012345678901234".to_string(),
        None,
        None,
    );

    let res: HandleResult = handle(&mut deps, env.clone(), msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Description too long"),
        Err(_) => panic!("Unknown error"),
    }
}

fn create_poll_msg(
    quorum_percentage: u8,
    description: String,
    start_height: Option<u64>,
    end_height: Option<u64>,
) -> HandleMsg {
    let msg = HandleMsg::CreatePoll {
        quorum_percentage: Some(quorum_percentage),
        description,
        start_height,
        end_height,
        execute_msg: None,
    };
    msg
}

#[test]
fn happy_days_create_poll() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let env = mock_env(
        &HumanAddr(TEST_CREATOR.to_string()),
        &coins(2, VOTING_TOKEN),
    );
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let quorum = 30;
    let msg = create_poll_msg(quorum, "test".to_string(), None, Some(DEFAULT_END_HEIGHT));

    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    assert_create_poll_result(
        1,
        quorum,
        DEFAULT_END_HEIGHT,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );
}

#[test]
fn create_poll_no_quorum() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let env = mock_env_height(&HumanAddr(TEST_CREATOR.to_string()), 0, 10000);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let quorum = 0;
    let msg = create_poll_msg(quorum, "test".to_string(), None, None);

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_create_poll_result(
        1,
        quorum,
        DEFAULT_END_HEIGHT,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );
}

#[test]
fn fails_end_poll_before_end_height() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let env = mock_env_height(&HumanAddr(TEST_CREATOR.to_string()), 0, 10000);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let msg = create_poll_msg(0, "test".to_string(), None, Some(10001));

    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_create_poll_result(
        1,
        0,
        10001,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );

    let res = query(&mut deps, QueryMsg::Poll { poll_id: 1 }).unwrap();
    let value: PollResponse = from_binary(&res).unwrap();
    assert_eq!(Some(10001), value.end_height);

    let msg = HandleMsg::EndPoll { poll_id: 1 };

    let handle_res: HandleResult = handle(&mut deps, env.clone(), msg);

    match handle_res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Voting period has not expired."),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_end_poll() {
    const POLL_END_HEIGHT: u64 = 1000;
    const POLL_ID: u64 = 1;
    let stake_amount = 1000;
    let mut deps = mock_instance(WASM, &coins(stake_amount, VOTING_TOKEN));

    let msg = init_msg();
    let mut creator_env =
        mock_env_height(&HumanAddr(TEST_CREATOR.to_string()), POLL_END_HEIGHT, 10000);
    let res: InitResponse = init(&mut deps, creator_env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let msg = create_poll_msg(
        0,
        "test".to_string(),
        None,
        Some(creator_env.block.height + 1),
    );

    let handle_res: HandleResponse = handle(&mut deps, creator_env.clone(), msg).unwrap();

    assert_create_poll_result(
        POLL_ID,
        0,
        creator_env.block.height + 1,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER),
        amount: Uint128::from(stake_amount),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });

    let env = mock_env(VOTING_TOKEN, &[]);
    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let msg = HandleMsg::CastVote {
        poll_id: POLL_ID,
        vote: "yes".to_string(),
        weight: Uint128::from(stake_amount),
    };
    let env = mock_env(TEST_VOTER, &[]);
    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg).unwrap();

    assert_eq!(
        handle_res.log,
        vec![
            log("action", "vote_casted"),
            log("poll_id", POLL_ID),
            log("weight", "1000"),
            log("voter", TEST_VOTER),
        ]
    );
    creator_env.block.height = &creator_env.block.height + 1;

    let msg = HandleMsg::EndPoll { poll_id: POLL_ID };

    let handle_res: HandleResponse = handle(&mut deps, creator_env.clone(), msg).unwrap();

    assert_eq!(
        handle_res.log,
        vec![
            log("action", "end_poll"),
            log("poll_id", "1"),
            log("rejected_reason", ""),
            log("passed", "true"),
        ]
    );
}

#[test]
fn end_poll_zero_quorum() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let env = mock_env_height(creator, 1000, 0);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    //create poll
    let env2 = mock_env_height(&address(0), 1001, 0);
    let msg = create_poll_msg(0, "test".to_string(), None, Some(env2.block.height));
    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_create_poll_result(1, 0, 1001, 0, creator, handle_res);

    //end poll
    let msg = HandleMsg::EndPoll { poll_id: 1 };
    let handle_res: HandleResponse = handle(&mut deps, env2.clone(), msg).unwrap();

    assert_eq!(
        handle_res.log,
        vec![
            log("action", "end_poll"),
            log("poll_id", "1"),
            log("rejected_reason", "Quorum not reached"),
            log("passed", "false"),
        ]
    );
}

#[test]
fn end_poll_quorum_rejected() {
    let stake_amount = 100;
    let mut deps = mock_instance(WASM, &coins(stake_amount, VOTING_TOKEN));
    let msg = init_msg();
    let env = mock_env(TEST_CREATOR, &coins(stake_amount, VOTING_TOKEN));
    let init_res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    let msg = create_poll_msg(30, "test".to_string(), None, Some(&env.block.height + 1));

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(
        handle_res.log,
        vec![
            log("action", "create_poll"),
            log("creator", &HumanAddr(TEST_CREATOR.to_string())),
            log("poll_id", "1"),
            log("quorum_percentage", "30"),
            log("end_height", "12346"),
            log("start_height", "0"),
        ]
    );

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_CREATOR),
        amount: Uint128::from(stake_amount),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });

    let env = mock_env(VOTING_TOKEN, &[]);
    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let msg = HandleMsg::CastVote {
        poll_id: 1,
        vote: "yes".to_string(),
        weight: Uint128::from(10u128),
    };
    let mut env = mock_env(TEST_CREATOR, &[]);
    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg).unwrap();

    assert_eq!(
        handle_res.log,
        vec![
            log("action", "vote_casted"),
            log("poll_id", "1"),
            log("weight", "10"),
            log("voter", TEST_CREATOR),
        ]
    );

    let msg = HandleMsg::EndPoll { poll_id: 1 };

    env.block.height = &env.block.height + 2;

    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        handle_res.log,
        vec![
            log("action", "end_poll"),
            log("poll_id", "1"),
            log("rejected_reason", "Quorum not reached"),
            log("passed", "false"),
        ]
    );
}

#[test]
fn end_poll_nay_rejected() {
    let voter1_stake = 100;
    let voter2_stake = 1000;
    let stake_amount = 100;
    let mut deps = mock_instance(WASM, &coins(stake_amount, VOTING_TOKEN));
    let msg = init_msg();
    let mut creator_env = mock_env(TEST_CREATOR, &[]);
    let init_res: InitResponse = init(&mut deps, creator_env.clone(), msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    let msg = create_poll_msg(
        10,
        "test".to_string(),
        None,
        Some(creator_env.block.height + 1),
    );

    let handle_res: HandleResponse = handle(&mut deps, creator_env.clone(), msg.clone()).unwrap();
    assert_eq!(
        handle_res.log,
        vec![
            log("action", "create_poll"),
            log("creator", &HumanAddr(TEST_CREATOR.to_string())),
            log("poll_id", "1"),
            log("quorum_percentage", "10"),
            log("end_height", "12346"),
            log("start_height", "0"),
        ]
    );

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER),
        amount: Uint128::from(voter1_stake as u128),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });

    let env = mock_env(VOTING_TOKEN, &[]);
    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER_2),
        amount: Uint128::from(voter2_stake as u128),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let env = mock_env(TEST_VOTER_2, &[]);
    let msg = HandleMsg::CastVote {
        poll_id: 1,
        vote: "no".to_string(),
        weight: Uint128::from(voter2_stake),
    };
    let handle_res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_cast_vote_success(TEST_VOTER_2, voter2_stake, 1, handle_res);

    let msg = HandleMsg::EndPoll { poll_id: 1 };

    creator_env.block.height = &creator_env.block.height + 2;
    let handle_res: HandleResponse = handle(&mut deps, creator_env.clone(), msg.clone()).unwrap();
    assert_eq!(
        handle_res.log,
        vec![
            log("action", "end_poll"),
            log("poll_id", "1"),
            log("rejected_reason", "Threshold not reached"),
            log("passed", "false"),
        ]
    );
}

#[test]
fn fails_end_poll_before_start_height() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let env = mock_env(TEST_CREATOR, &[]);
    let init_res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    let start_height = &env.block.height + 10;
    let quorum_percentage = 30;
    let msg = create_poll_msg(
        quorum_percentage,
        "test".to_string(),
        Some(start_height),
        Some(DEFAULT_END_HEIGHT),
    );

    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_create_poll_result(
        1,
        quorum_percentage,
        DEFAULT_END_HEIGHT,
        start_height,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );
    let msg = HandleMsg::EndPoll { poll_id: 1 };

    let handle_res: HandleResult = handle(&mut deps, env.clone(), msg);

    match handle_res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Voting period has not started."),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_cast_vote_not_enough_staked() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let env = mock_env(TEST_CREATOR, &[]);
    let init_res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    let msg = create_poll_msg(0, "test".to_string(), None, Some(DEFAULT_END_HEIGHT));

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_create_poll_result(
        1,
        0,
        DEFAULT_END_HEIGHT,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );

    let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let msg = HandleMsg::CastVote {
        poll_id: 1,
        vote: "yes".to_string(),
        weight: Uint128::from(1u128),
    };

    let res: HandleResult = handle(&mut deps, env, msg);

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
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let env = mock_env_height(creator, 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let quorum_percentage = 30;

    let msg = create_poll_msg(quorum_percentage, "test".to_string(), None, None);

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_create_poll_result(
        1,
        quorum_percentage,
        DEFAULT_END_HEIGHT,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER),
        amount: Uint128::from(11u128),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });
    let env = mock_env(VOTING_TOKEN, &[]);

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));
    let weight = 10u128;
    let msg = HandleMsg::CastVote {
        poll_id: 1,
        vote: "yes".to_string(),
        weight: Uint128::from(weight),
    };

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_cast_vote_success(TEST_VOTER, weight, 1, handle_res);
}

#[test]
fn happy_days_withdraw_voting_tokens() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let env = mock_env_height(creator, 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let staked_tokens = 11;
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER),
        amount: Uint128::from(staked_tokens as u128),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });
    let env = mock_env(VOTING_TOKEN, &[]);

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let api = deps.api;
    //confirm stake increased
    deps.with_storage(|store| {
        let config_key_raw = to_length_prefixed(b"config");
        let state: State = from_slice(&store.get(&config_key_raw).0.unwrap().unwrap()).unwrap();
        assert_eq!(
            state,
            State {
                mirror_token: api
                    .canonical_address(&HumanAddr::from(VOTING_TOKEN))
                    .0
                    .unwrap(),
                owner: api
                    .canonical_address(&HumanAddr::from(&HumanAddr(TEST_CREATOR.to_string())))
                    .0
                    .unwrap(),
                poll_count: 0,
                staked_tokens: Uint128::from(staked_tokens),
            }
        );
        Ok(())
    })
    .unwrap();

    // withdraw all stake
    let env = mock_env(TEST_VOTER, &coins(staked_tokens, VOTING_TOKEN));
    let msg = HandleMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(staked_tokens)),
    };

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    let msg = handle_res.messages.get(0).expect("no message");

    assert_eq!(
        msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from(VOTING_TOKEN),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from(TEST_VOTER),
                amount: Uint128::from(staked_tokens as u128),
            })
            .unwrap(),
            send: vec![],
        })
    );

    // staked is reduced
    &deps
        .with_storage(|store| {
            let config_key_raw = to_length_prefixed(b"config");
            let state: State = from_slice(&store.get(&config_key_raw).0.unwrap().unwrap()).unwrap();
            assert_eq!(
                state,
                State {
                    mirror_token: api
                        .canonical_address(&HumanAddr::from(VOTING_TOKEN))
                        .0
                        .unwrap(),
                    owner: api
                        .canonical_address(&HumanAddr::from(&HumanAddr(TEST_CREATOR.to_string())))
                        .0
                        .unwrap(),
                    poll_count: 0,
                    staked_tokens: Uint128::zero(),
                }
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn fails_withdraw_voting_tokens_no_stake() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let env = mock_env_height(creator, 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let msg = HandleMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let res: HandleResult = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Nothing staked"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_withdraw_too_many_tokens() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let env = mock_env_height(creator, 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER),
        amount: Uint128::from(10u128),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });
    let env = mock_env(VOTING_TOKEN, &[]);

    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let env = mock_env(TEST_VOTER, &[]);
    let msg = HandleMsg::WithdrawVotingTokens {
        amount: Some(Uint128::from(11u128)),
    };

    let res: HandleResult = handle(&mut deps, env, msg);

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
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let env = mock_env_height(creator, 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let quorum_percentage = 30;
    let msg = create_poll_msg(quorum_percentage, "test".to_string(), None, None);
    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    assert_create_poll_result(
        1,
        quorum_percentage,
        DEFAULT_END_HEIGHT,
        0,
        &HumanAddr(TEST_CREATOR.to_string()),
        handle_res,
    );

    let env = mock_env(VOTING_TOKEN, &[]);
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER),
        amount: Uint128::from(11u128),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });

    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());

    let weight = 1u128;
    let msg = HandleMsg::CastVote {
        poll_id: 1,
        vote: "yes".to_string(),
        weight: Uint128::from(weight),
    };
    let env = mock_env(TEST_VOTER, &[]);
    let handle_res: HandleResponse = handle(&mut deps, env.clone(), msg).unwrap();
    assert_cast_vote_success(TEST_VOTER, weight, 1, handle_res);

    let msg = HandleMsg::CastVote {
        poll_id: 1,
        vote: "yes".to_string(),
        weight: Uint128::from(weight),
    };
    let res: HandleResult = handle(&mut deps, env.clone(), msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "User has already voted."),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_cast_vote_without_poll() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let env = mock_env_height(creator, 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let msg = HandleMsg::CastVote {
        poll_id: 0,
        vote: "yes".to_string(),
        weight: Uint128::from(1u128),
    };
    let env = mock_env(TEST_VOTER, &coins(11, VOTING_TOKEN));

    let res: HandleResult = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Poll does not exist"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn happy_days_stake_voting_tokens() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let creator = &address(0);
    let env = mock_env_height(creator, 0, 0);
    let res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    let env = mock_env(VOTING_TOKEN, &[]);

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER),
        amount: Uint128::from(11u128),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });
    let handle_res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(handle_res, HandleResponse::default());
}

#[test]
fn fails_insufficient_funds() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let env = mock_env(TEST_VOTER, &coins(2, VOTING_TOKEN));
    let init_res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, init_res.messages.len());

    // insufficient token
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER),
        amount: Uint128::from(0u128),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });
    let env = mock_env(VOTING_TOKEN, &[]);

    let res: HandleResult = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Insufficient funds sent"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn fails_staking_wrong_token() {
    let mut deps = mock_instance(WASM, &[]);

    // initialize the store
    let msg = init_msg();
    let env = mock_env(TEST_VOTER, &coins(2, VOTING_TOKEN));
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // wrong token
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from(TEST_VOTER),
        amount: Uint128::from(11u128),
        msg: Some(to_binary(&Cw20HookMsg::StakeVotingTokens {}).unwrap()),
    });
    let env = mock_env("play money", &[]);

    let res: HandleResult = handle(&mut deps, env, msg);

    match res {
        Ok(_) => panic!("Must return error"),
        Err(StdError::Unauthorized { .. }) => {}
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

// helper to confirm the expected create_poll response
fn assert_create_poll_result(
    poll_id: u64,
    quorum: u8,
    end_height: u64,
    start_height: u64,
    creator: &HumanAddr,
    handle_res: HandleResponse,
) {
    assert_eq!(
        handle_res.log,
        vec![
            log("action", "create_poll"),
            log("creator", creator),
            log("poll_id", poll_id.to_string()),
            log("quorum_percentage", quorum.to_string()),
            log("end_height", end_height.to_string()),
            log("start_height", start_height.to_string()),
        ]
    );
}

fn assert_cast_vote_success(voter: &str, weight: u128, poll_id: u64, handle_res: HandleResponse) {
    assert_eq!(
        handle_res.log,
        vec![
            log("action", "vote_casted"),
            log("poll_id", poll_id.to_string()),
            log("weight", weight.to_string()),
            log("voter", voter),
        ]
    );
}
