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
    from_binary, Coin, Decimal, HandleResponse, HandleResult, HumanAddr, InitResponse, StdError,
};
use cosmwasm_vm::testing::{
    handle, init, mock_dependencies, mock_env, query, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_vm::Instance;

use mirror_mint::msg::{ConfigAssetResponse, ConfigGeneralResponse, HandleMsg, InitMsg, QueryMsg};

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/mirror_mint.wasm");
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
        collateral_denom: "uusd".to_string(),
        auction_discount: Decimal::percent(10),
        auction_threshold_rate: Decimal::percent(80),
        mint_capacity: Decimal::percent(70),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();

    // post initalize
    let msg = HandleMsg::PostInitialize {
        asset_oracle: HumanAddr::from("oracle0000"),
        asset_token: HumanAddr::from("asset0000"),
        asset_symbol: "mAPPL".to_string(),
    };

    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::ConfigGeneral {}).unwrap();
    let config: ConfigGeneralResponse = from_binary(&res).unwrap();
    let res = query(&mut deps, QueryMsg::ConfigAsset {}).unwrap();
    let asset: ConfigAssetResponse = from_binary(&res).unwrap();
    assert_eq!("addr0000", config.owner.as_str());
    assert_eq!("uusd", config.collateral_denom.as_str());
    assert_eq!(Decimal::percent(10), config.auction_discount);
    assert_eq!(Decimal::percent(80), config.auction_threshold_rate);
    assert_eq!(Decimal::percent(70), config.mint_capacity);
    assert_eq!("oracle0000", asset.oracle.as_str());
    assert_eq!("asset0000", asset.token.as_str());
    assert_eq!("mAPPL", asset.symbol.as_str());
}

#[test]
fn update_config() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        auction_discount: Decimal::percent(10),
        auction_threshold_rate: Decimal::percent(80),
        mint_capacity: Decimal::percent(70),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();

    // post initalize
    let msg = HandleMsg::PostInitialize {
        asset_oracle: HumanAddr::from("oracle0000"),
        asset_token: HumanAddr::from("asset0000"),
        asset_symbol: "mAPPL".to_string(),
    };

    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // update owner
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("addr0001".to_string())),
        auction_discount: None,
        auction_threshold_rate: None,
        mint_capacity: Some(Decimal::percent(75)),
    };

    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let query_result = query(&mut deps, QueryMsg::ConfigGeneral {}).unwrap();
    let value: ConfigGeneralResponse = from_binary(&query_result).unwrap();
    assert_eq!("addr0001", value.owner.as_str());
    assert_eq!("uusd", value.collateral_denom.as_str());
    assert_eq!(Decimal::percent(10), value.auction_discount);
    assert_eq!(Decimal::percent(80), value.auction_threshold_rate);
    assert_eq!(Decimal::percent(75), value.mint_capacity);

    // Unauthorzied err
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        auction_discount: None,
        auction_threshold_rate: None,
        mint_capacity: None,
    };

    let res: HandleResult = handle(&mut deps, env, msg);
    match res.unwrap_err() {
        StdError::Unauthorized { .. } => {}
        _ => panic!("Must return unauthorized error"),
    }
}
