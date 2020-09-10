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
    coins, from_binary, log, Env, HandleResponse, HandleResult, HumanAddr, InitResponse, StdError,
    Uint128,
};
use cosmwasm_storage::to_length_prefixed;
use cosmwasm_vm::testing::{handle, init, mock_env, mock_instance, query, MOCK_CONTRACT_ADDR};
use cosmwasm_vm::{from_slice, Api, Storage};
use mirror_gov::contract::VOTING_TOKEN;
use mirror_gov::msg::{HandleMsg, InitMsg, PollResponse, QueryMsg};
use mirror_gov::state::State;

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/mirror_gov.wasm");
// You can uncomment this line instead to test productionified build from rust-optimizer
// static WASM: &[u8] = include_bytes!("../contract.wasm");

const DEFAULT_END_HEIGHT: u64 = 100800u64;
const TEST_CREATOR: &str = "creator";
const TEST_VOTER: &str = "voter1";

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
                contract_addr: api
                    .canonical_address(&HumanAddr::from(&HumanAddr(MOCK_CONTRACT_ADDR.to_string())))
                    .0
                    .unwrap(),
                poll_count: 0,
                total_share: Uint128::zero(),
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
        share: Uint128::from(1u128),
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
