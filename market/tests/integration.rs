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

use mirror_market::msg::{
    ConfigAssetResponse, ConfigGeneralResponse, ConfigSwapResponse, HandleMsg, InitMsg, QueryMsg,
};

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/mirror_market.wasm");
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
        liquidity_token: HumanAddr("liquidity0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        asset_symbol: "mAPPL".to_string(),
        asset_token: HumanAddr("asset0000".to_string()),
        asset_oracle: HumanAddr("oracle0000".to_string()),
        active_commission: Decimal::permille(3),
        inactive_commission: Decimal::permille(1),
        max_spread: Decimal::percent(20),
        max_minus_spread: Decimal::percent(2),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::ConfigGeneral {}).unwrap();
    let config_general: ConfigGeneralResponse = from_binary(&res).unwrap();
    assert_eq!("addr0000", config_general.owner.as_str());
    assert_eq!(
        "collector0000",
        config_general.commission_collector.as_str()
    );
    assert_eq!("uusd", config_general.collateral_denom.as_str());
    assert_eq!("liquidity0000", config_general.liquidity_token.as_str());

    let res = query(&mut deps, QueryMsg::ConfigAsset {}).unwrap();
    let config_asset: ConfigAssetResponse = from_binary(&res).unwrap();
    assert_eq!("oracle0000", config_asset.oracle.as_str());
    assert_eq!("mAPPL", config_asset.symbol.as_str());
    assert_eq!("asset0000", config_asset.token.as_str());

    let res = query(&mut deps, QueryMsg::ConfigSwap {}).unwrap();
    let config_swap: ConfigSwapResponse = from_binary(&res).unwrap();
    assert_eq!(Decimal::permille(3), config_swap.active_commission);
    assert_eq!(Decimal::permille(1), config_swap.inactive_commission);
    assert_eq!(Decimal::percent(20), config_swap.max_spread);
    assert_eq!(Decimal::percent(2), config_swap.max_minus_spread);
}

#[test]
fn update_config() {
    let mut deps = mock_instance(WASM, &[]);
    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        liquidity_token: HumanAddr("liquidity0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        asset_symbol: "mAPPL".to_string(),
        asset_token: HumanAddr("asset0000".to_string()),
        asset_oracle: HumanAddr("oracle0000".to_string()),
        active_commission: Decimal::permille(3),
        inactive_commission: Decimal::permille(1),
        max_spread: Decimal::percent(20),
        max_minus_spread: Decimal::percent(2),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    // update owner
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("addr0001".to_string())),
        active_commission: None,
        inactive_commission: None,
        max_minus_spread: None,
        max_spread: None,
    };

    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let query_result = query(&mut deps, QueryMsg::ConfigGeneral {}).unwrap();
    let config_general: ConfigGeneralResponse = from_binary(&query_result).unwrap();
    assert_eq!("addr0001", config_general.owner.as_str());

    // Unauthorzied err
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        active_commission: Some(Decimal::percent(1)),
        inactive_commission: Some(Decimal::percent(2)),
        max_minus_spread: Some(Decimal::percent(5)),
        max_spread: Some(Decimal::percent(6)),
    };

    let res: HandleResult = handle(&mut deps, env, msg);
    match res.unwrap_err() {
        StdError::Unauthorized { .. } => {}
        _ => panic!("Must return unauthorized error"),
    }
}
