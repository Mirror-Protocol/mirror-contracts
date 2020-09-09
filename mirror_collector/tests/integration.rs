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
use cosmwasm_std::{from_binary, HumanAddr, InitResponse};
use cosmwasm_vm::testing::{init, mock_env, mock_instance, query};
use mirror_collector::msg::{ConfigResponse, InitMsg, QueryMsg};

// This line will test the output of cargo wasm
static WASM: &[u8] =
    include_bytes!("../target/wasm32-unknown-unknown/release/mirror_collector.wasm");
// You can uncomment this line instead to test productionified build from rust-optimizer
// static WASM: &[u8] = include_bytes!("../contract.wasm");

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        factory_contract: HumanAddr("factory0000".to_string()),
        staking_symbol: "staking".to_string(),
        collateral_denom: "uusd".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("factory0000", config.factory_contract.as_str());
    assert_eq!("staking", config.staking_symbol.as_str());
    assert_eq!("uusd", config.collateral_denom.as_str());
}
