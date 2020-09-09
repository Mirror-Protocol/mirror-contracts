use crate::contract::{handle, init, query_config, query_distribution_info, query_whitelist_info};
use crate::msg::{
    ConfigResponse, DistributionInfoResponse, HandleMsg, InitMsg, StakingCw20HookMsg,
    WhitelistInfoResponse,
};
use cosmwasm_std::testing::{mock_dependencies, mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    log, to_binary, CosmosMsg, Decimal, Env, HumanAddr, StdError, Uint128, WasmMsg,
};
use cw20::Cw20HandleMsg;

fn mock_env_height(signer: &HumanAddr, height: u64) -> Env {
    let mut env = mock_env(signer, &[]);
    env.block.height = height;
    env
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // cannot update mirror token after initialization
    let msg = HandleMsg::PostInitialize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res = handle(&mut deps, env, msg).unwrap_err();

    // it worked, let's query the state
    let config: ConfigResponse = query_config(&deps).unwrap();
    assert_eq!("addr0000", config.owner.as_str());
    assert_eq!("token0000", config.mirror_token.as_str());
    assert_eq!(Uint128(100u128), config.mint_per_block);
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res = handle(&mut deps, env, msg).unwrap();

    // upate owner
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr::from("addr0001")),
        mint_per_block: None,
    };

    let env = mock_env("addr0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let config: ConfigResponse = query_config(&deps).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!("token0000", config.mirror_token.as_str());
    assert_eq!(Uint128(100u128), config.mint_per_block);

    // update rest part
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        mint_per_block: Some(Uint128(200u128)),
    };

    let env = mock_env("addr0001", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let config: ConfigResponse = query_config(&deps).unwrap();
    assert_eq!("addr0001", config.owner.as_str());
    assert_eq!("token0000", config.mirror_token.as_str());
    assert_eq!(Uint128(200u128), config.mint_per_block);

    // failed unauthoirzed
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        mint_per_block: Some(Uint128(200u128)),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_whitelist() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res = handle(&mut deps, env, msg).unwrap();

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
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    assert_eq!(
        res.log,
        vec![
            log("action", "whitelist"),
            log("symbol", "mAPPL"),
            log("weight", "1.5")
        ]
    );

    let res: WhitelistInfoResponse = query_whitelist_info(&deps, "mAPPL".to_string()).unwrap();
    assert_eq!(res.mint_contract, HumanAddr::from("mint0000"));
    assert_eq!(res.market_contract, HumanAddr::from("market0000"));
    assert_eq!(res.oracle_contract, HumanAddr::from("oracle0000"));
    assert_eq!(res.token_contract, HumanAddr::from("token0000"));
    assert_eq!(res.staking_contract, HumanAddr::from("staking0000"));

    let res: DistributionInfoResponse =
        query_distribution_info(&deps, "mAPPL".to_string()).unwrap();
    assert_eq!(res.weight, Decimal::from_ratio(15u64, 10u64));
    assert_eq!(res.last_height, 12345u64);

    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "whitelist mAPPL already exists"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env("addr0001", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_update_weight() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res = handle(&mut deps, env, msg).unwrap();

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
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::UpdateWeight {
        symbol: "mAPPL".to_string(),
        weight: Decimal::from_ratio(2u64, 1u64),
    };
    let res = handle(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "update_weight"),
            log("symbol", "mAPPL"),
            log("weight", "2")
        ]
    );

    let res: DistributionInfoResponse =
        query_distribution_info(&deps, "mAPPL".to_string()).unwrap();
    assert_eq!(res.weight, Decimal::from_ratio(2u64, 1u64));
}

#[test]
fn test_mint() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        mint_per_block: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let msg = HandleMsg::PostInitialize {
        mirror_token: HumanAddr("token0000".to_string()),
    };
    let _res = handle(&mut deps, env, msg).unwrap();

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
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // height is not increased so zero amount will be minted
    let msg = HandleMsg::Mint {
        symbol: "mAPPL".to_string(),
    };
    let res = handle(&mut deps, env, msg).unwrap();
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
    let env = mock_env_height(&HumanAddr::from("addr0000"), 12346u64);
    let res = handle(&mut deps, env, msg).unwrap();
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
