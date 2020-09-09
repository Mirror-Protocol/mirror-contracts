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
    from_binary, log, to_binary, CosmosMsg, Decimal, Env, HandleResponse, HandleResult, HumanAddr,
    InitResponse, StdError, Uint128, WasmMsg,
};
use cosmwasm_vm::testing::{handle, init, mock_env, mock_instance, query, MOCK_CONTRACT_ADDR};
use cw20::Cw20HandleMsg;
use mirror_factory::msg::{
    ConfigResponse, DistributionInfoResponse, HandleMsg, InitMsg, QueryMsg, StakingCw20HookMsg,
    WhitelistInfoResponse,
};

fn mock_env_height(signer: &HumanAddr, height: u64, time: u64) -> Env {
    let mut env = mock_env(signer, &[]);
    env.block.height = height;
    env.block.time = time;
    env
}

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/mirror_factory.wasm");
// You can uncomment this line instead to test productionified build from rust-optimizer
// static WASM: &[u8] = include_bytes!("../contract.wasm");

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitlize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("addr0000", config.owner.as_str());
    assert_eq!("token0000", config.mirror_token.as_str());
    assert_eq!(Uint128(100u128), config.mint_per_block);
}

#[test]
fn test_update_config() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitlize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // upate owner
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr::from("addr0001")),
        mint_per_block: None,
    };

    let env = mock_env("addr0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!("token0000", config.mirror_token.as_str());
    assert_eq!(Uint128(100u128), config.mint_per_block);

    // update rest part
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        mint_per_block: Some(Uint128(200u128)),
    };

    let env = mock_env("addr0001", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!("token0000", config.mirror_token.as_str());
    assert_eq!(Uint128(200u128), config.mint_per_block);

    // failed unauthoirzed
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        mint_per_block: Some(Uint128(200u128)),
    };

    let env = mock_env("addr0000", &[]);
    let res: HandleResult = handle(&mut deps, env, msg);
    match res.unwrap_err() {
        StdError::Unauthorized { .. } => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_whitelist() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitlize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Whitelist {
        symbol: "mAPPL".to_string(),
        weight: Decimal::from_ratio(15u64, 10u64),
        mint_contract: HumanAddr::from("mint0000"),
        market_contract: HumanAddr::from("market0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        token_contract: HumanAddr::from("token0000"),
        staking_contract: HumanAddr::from("staking0000"),
    };
    let env = mock_env("addr0000", &[]);
    let res: HandleResponse = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    assert_eq!(
        res.log,
        vec![
            log("action", "whitelist"),
            log("symbol", "mAPPL"),
            log("weight", "1.5")
        ]
    );

    let res = query(
        &mut deps,
        QueryMsg::WhitelistInfo {
            symbol: "mAPPL".to_string(),
        },
    )
    .unwrap();
    let res: WhitelistInfoResponse = from_binary(&res).unwrap();
    assert_eq!(res.mint_contract, HumanAddr::from("mint0000"));
    assert_eq!(res.market_contract, HumanAddr::from("market0000"));
    assert_eq!(res.oracle_contract, HumanAddr::from("oracle0000"));
    assert_eq!(res.token_contract, HumanAddr::from("token0000"));
    assert_eq!(res.staking_contract, HumanAddr::from("staking0000"));

    let res = query(
        &mut deps,
        QueryMsg::DistributionInfo {
            symbol: "mAPPL".to_string(),
        },
    )
    .unwrap();
    let res: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(res.weight, Decimal::from_ratio(15u64, 10u64));
    assert_eq!(res.last_height, 12345u64);

    let res: HandleResult = handle(&mut deps, env.clone(), msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "whitelist mAPPL already exists"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env("addr0001", &[]);
    let res: HandleResult = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_update_weight() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitlize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Whitelist {
        symbol: "mAPPL".to_string(),
        weight: Decimal::from_ratio(15u64, 10u64),
        mint_contract: HumanAddr::from("mint0000"),
        market_contract: HumanAddr::from("market0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        token_contract: HumanAddr::from("token0000"),
        staking_contract: HumanAddr::from("staking0000"),
    };
    let env = mock_env("addr0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::UpdateWeight {
        symbol: "mAPPL".to_string(),
        weight: Decimal::from_ratio(2u64, 1u64),
    };
    let res: HandleResponse = handle(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "update_weight"),
            log("symbol", "mAPPL"),
            log("weight", "2")
        ]
    );

    let res = query(
        &mut deps,
        QueryMsg::DistributionInfo {
            symbol: "mAPPL".to_string(),
        },
    )
    .unwrap();
    let res: DistributionInfoResponse = from_binary(&res).unwrap();
    assert_eq!(res.weight, Decimal::from_ratio(2u64, 1u64));
}

#[test]
fn test_mint() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitlize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Whitelist {
        symbol: "mAPPL".to_string(),
        weight: Decimal::from_ratio(15u64, 10u64),
        mint_contract: HumanAddr::from("mint0000"),
        market_contract: HumanAddr::from("market0000"),
        oracle_contract: HumanAddr::from("oracle0000"),
        token_contract: HumanAddr::from("token0000"),
        staking_contract: HumanAddr::from("staking0000"),
    };
    let env = mock_env("addr0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env.clone(), msg).unwrap();

    // height is not increased so zero amount will be minted
    let msg = HandleMsg::Mint {
        symbol: "mAPPL".to_string(),
    };
    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "mint"),
            log("symbol", "mAPPL"),
            log("mint_amount", "0"),
        ]
    );

    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("token0000"),
                msg: to_binary(&Cw20HandleMsg::Mint {
                    recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    amount: Uint128::zero(),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("token0000"),
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: HumanAddr::from("staking0000"),
                    amount: Uint128::zero(),
                    msg: Some(to_binary(&StakingCw20HookMsg::DepositReward {}).unwrap()),
                })
                .unwrap(),
                send: vec![],
            }),
        ],
    );

    // one height increase
    let msg = HandleMsg::Mint {
        symbol: "mAPPL".to_string(),
    };
    let env = mock_env_height(&HumanAddr::from("addr0000"), 12346u64, 12345u64);
    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "mint"),
            log("symbol", "mAPPL"),
            log("mint_amount", "150"),
        ]
    );

    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("token0000"),
                msg: to_binary(&Cw20HandleMsg::Mint {
                    recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    amount: Uint128(150u128),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("token0000"),
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: HumanAddr::from("staking0000"),
                    amount: Uint128(150u128),
                    msg: Some(to_binary(&StakingCw20HookMsg::DepositReward {}).unwrap()),
                })
                .unwrap(),
                send: vec![],
            }),
        ],
    );
}
