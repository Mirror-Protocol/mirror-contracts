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
    from_binary, Coin, HandleResponse, HandleResult, HumanAddr, InitResponse, StdError, Uint128,
};
use cosmwasm_vm::testing::{
    handle, init, mock_dependencies, mock_env, query, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_vm::Instance;
use mirror_factory::msg::{ConfigResponse, HandleMsg, InitMsg, QueryMsg};

// This line will test the output of cargo wasm
static WASM: &[u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/mirror_factory.wasm");
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

static TOKEN_CODE_ID: u64 = 10u64;
static BASE_DENOM: &str = "uusd";
#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res: HandleResponse = handle(&mut deps, env.clone(), msg).unwrap();

    // cannot update mirror token after initialization
    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let res: HandleResult = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: HumanAddr::from("owner0000"),
            mirror_token: HumanAddr::from("mirror0000"),
            mint_contract: HumanAddr::from("mint0000"),
            staking_contract: HumanAddr::from("staking0000"),
            commission_collector: HumanAddr::from("collector0000"),
            oracle_contract: HumanAddr::from("oracle0000"),
            terraswap_factory: HumanAddr::from("terraswapfactory"),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID,
            genesis_time: 1_571_797_419,
            distribution_schedule: vec![],
        }
    );
}

#[test]
fn test_update_config() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
        base_denom: BASE_DENOM.to_string(),
        token_code_id: TOKEN_CODE_ID,
        distribution_schedule: vec![],
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
        mint_contract: HumanAddr::from("mint0000"),
        staking_contract: HumanAddr::from("staking0000"),
        commission_collector: HumanAddr::from("collector0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        terraswap_factory: HumanAddr::from("terraswapfactory"),
    };
    let _res: HandleResponse = handle(&mut deps, env.clone(), msg).unwrap();

    // upate owner
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr::from("owner0001")),
        token_code_id: None,
        distribution_schedule: None,
    };

    let env = mock_env("owner0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: HumanAddr::from("owner0001"),
            mirror_token: HumanAddr::from("mirror0000"),
            mint_contract: HumanAddr::from("mint0000"),
            staking_contract: HumanAddr::from("staking0000"),
            commission_collector: HumanAddr::from("collector0000"),
            oracle_contract: HumanAddr::from("oracle0000"),
            terraswap_factory: HumanAddr::from("terraswapfactory"),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID,
            genesis_time: 1_571_797_419,
            distribution_schedule: vec![],
        }
    );

    // update rest part
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        token_code_id: Some(TOKEN_CODE_ID + 1),
        distribution_schedule: Some(vec![(1, 2, Uint128::from(123u128))]),
    };

    let env = mock_env("owner0001", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: HumanAddr::from("owner0001"),
            mirror_token: HumanAddr::from("mirror0000"),
            mint_contract: HumanAddr::from("mint0000"),
            staking_contract: HumanAddr::from("staking0000"),
            commission_collector: HumanAddr::from("collector0000"),
            oracle_contract: HumanAddr::from("oracle0000"),
            terraswap_factory: HumanAddr::from("terraswapfactory"),
            base_denom: BASE_DENOM.to_string(),
            token_code_id: TOKEN_CODE_ID + 1,
            genesis_time: 1_571_797_419,
            distribution_schedule: vec![(1, 2, Uint128::from(123u128))],
        }
    );

    // failed unauthoirzed
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        distribution_schedule: None,
        token_code_id: Some(TOKEN_CODE_ID + 1),
    };

    let env = mock_env("owner0000", &[]);
    let res: HandleResult = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }
}
