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
    coins, from_binary, Decimal, HandleResponse, HandleResult, HumanAddr, InitResponse, StdError,
    Uint128,
};
use cosmwasm_storage::to_length_prefixed;
use cosmwasm_vm::testing::{handle, init, mock_env, mock_instance, query};
use cosmwasm_vm::{from_slice, Api, Storage};
use mirror_gov::msg::{ConfigResponse, HandleMsg, InitMsg, QueryMsg};
use mirror_gov::state::Config;

// This line will test the output of cargo wasm
static WASM: &[u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/mirror_gov.wasm");
// You can uncomment this line instead to test productionified build from rust-optimizer
// static WASM: &[u8] = include_bytes!("../contract.wasm");

const VOTING_TOKEN: &str = "voting_token";
const TEST_CREATOR: &str = "creator";
const DEFAULT_QUORUM: u64 = 30u64;
const DEFAULT_THRESHOLD: u64 = 50u64;
const DEFAULT_VOTING_PERIOD: u64 = 10000u64;
const DEFAULT_EFFECTIVE_DELAY: u64 = 10000u64;
const DEFAULT_PROPOSAL_DEPOSIT: u128 = 10000000000u128;

fn init_msg() -> InitMsg {
    InitMsg {
        mirror_token: HumanAddr::from(VOTING_TOKEN),
        quorum: Decimal::percent(DEFAULT_QUORUM),
        threshold: Decimal::percent(DEFAULT_THRESHOLD),
        voting_period: DEFAULT_VOTING_PERIOD,
        effective_delay: DEFAULT_EFFECTIVE_DELAY,
        proposal_deposit: Uint128(DEFAULT_PROPOSAL_DEPOSIT),
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
        let state: Config = from_slice(&store.get(&config_key_raw).0.unwrap().unwrap()).unwrap();
        assert_eq!(
            state,
            Config {
                mirror_token: api
                    .canonical_address(&HumanAddr::from(VOTING_TOKEN))
                    .0
                    .unwrap(),
                owner: api
                    .canonical_address(&HumanAddr::from(TEST_CREATOR))
                    .0
                    .unwrap(),
                quorum: Decimal::percent(DEFAULT_QUORUM),
                threshold: Decimal::percent(DEFAULT_THRESHOLD),
                voting_period: DEFAULT_VOTING_PERIOD,
                effective_delay: DEFAULT_EFFECTIVE_DELAY,
                proposal_deposit: Uint128(DEFAULT_PROPOSAL_DEPOSIT),
            }
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn update_config() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = init_msg();
    let env = mock_env(
        &HumanAddr(TEST_CREATOR.to_string()),
        &coins(2, VOTING_TOKEN),
    );
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // update owner
    let env = mock_env(TEST_CREATOR, &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("addr0001".to_string())),
        quorum: None,
        threshold: None,
        voting_period: None,
        effective_delay: None,
        proposal_deposit: None,
    };

    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!(Decimal::percent(DEFAULT_QUORUM), config.quorum);
    assert_eq!(Decimal::percent(DEFAULT_THRESHOLD), config.threshold);
    assert_eq!(DEFAULT_VOTING_PERIOD, config.voting_period);
    assert_eq!(DEFAULT_PROPOSAL_DEPOSIT, config.proposal_deposit.u128());

    // update left items
    let env = mock_env("addr0001", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        quorum: Some(Decimal::percent(20)),
        threshold: Some(Decimal::percent(75)),
        voting_period: Some(20000u64),
        effective_delay: Some(20000u64),
        proposal_deposit: Some(Uint128(123u128)),
    };

    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!(Decimal::percent(20), config.quorum);
    assert_eq!(Decimal::percent(75), config.threshold);
    assert_eq!(20000u64, config.voting_period);
    assert_eq!(123u128, config.proposal_deposit.u128());

    // Unauthorzied err
    let env = mock_env(TEST_CREATOR, &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        quorum: None,
        threshold: None,
        voting_period: None,
        effective_delay: None,
        proposal_deposit: None,
    };

    let res: HandleResult = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}
