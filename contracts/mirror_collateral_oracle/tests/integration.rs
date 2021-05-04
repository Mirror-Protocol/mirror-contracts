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
//! 3. If you access raw storage, where ever you see something like:
//!      deps.storage.get(CONFIG_KEY).expect("no data stored");
//!    replace it with:
//!      deps.with_storage(|store| {
//!          let data = store.get(CONFIG_KEY).expect("no data stored");
//!          //...
//!      });
//! 4. Anywhere you see query(&deps, ...) you must replace it with query(&mut deps, ...)

use cosmwasm_std::{
    from_binary, Coin, HandleResponse, HandleResult, HumanAddr, InitResponse, StdError,
};
use cosmwasm_vm::testing::{
    handle, init, mock_dependencies, mock_env, query, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_vm::Instance;
use mirror_protocol::collateral_oracle::{ConfigResponse, HandleMsg, InitMsg, QueryMsg};

// This line will test the output of cargo wasm
static WASM: &[u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/mirror_collateral_oracle.wasm");
// You can uncomment this line instead to test productionified build from rust-optimizer
// static WASM: &[u8] = include_bytes!("../contract.wasm");

const DEFAULT_GAS_LIMIT: u64 = 500_000;

pub fn mock_instance(
    wasm: &[u8],
    contract_balance: &[Coin],
) -> Instance<MockStorage, MockApi, MockQuerier> {
    // TODO: check_wasm is not exported from cosmwasm_vm
    // let terra_features = features_from_csv("staking,terra");
    // check_wasm(wasm, &terra_features).unwrap();
    let deps = mock_dependencies(20, contract_balance);
    Instance::from_code(wasm, deps, DEFAULT_GAS_LIMIT).unwrap()
}

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        mint_contract: HumanAddr("mint0000".to_string()),
        factory_contract: HumanAddr("factory0000".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let value: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0000", value.owner.as_str());
}

#[test]
fn update_owner() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        mint_contract: HumanAddr("mint0000".to_string()),
        factory_contract: HumanAddr("factory0000".to_string()),
        base_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();
    // update owner
    let env = mock_env("owner0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("owner0001".to_string())),
        mint_contract: Some(HumanAddr("mint0000".to_string())),
        factory_contract: Some(HumanAddr("factory0000".to_string())),
        base_denom: Some("uusd".to_string()),
    };

    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let query_result = query(&mut deps, QueryMsg::Config {}).unwrap();
    let value: ConfigResponse = from_binary(&query_result).unwrap();
    assert_eq!("owner0001", value.owner.as_str());
    assert_eq!("mint0000", value.mint_contract.as_str());
    assert_eq!("factory0000", value.factory_contract.as_str());
    assert_eq!("uusd", value.base_denom.as_str());

    // Unauthorzied err
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        mint_contract: None,
        factory_contract: None,
        base_denom: None,
    };

    let res: HandleResult = handle(&mut deps, env, msg);
    match res.unwrap_err() {
        StdError::Unauthorized { .. } => {}
        _ => panic!("Must return unauthorized error"),
    }
}
