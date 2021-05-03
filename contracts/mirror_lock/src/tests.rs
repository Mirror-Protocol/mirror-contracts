use crate::contract::{handle, init, query};
use crate::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, log, BankMsg, Coin, CosmosMsg, Decimal, HumanAddr, StdError, Uint128,
};
use mirror_protocol::lock::{
    ConfigResponse, HandleMsg, InitMsg, PositionLockInfoResponse, QueryMsg,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        mint_contract: HumanAddr::from("mint0000"),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let env = mock_env("addr0000", &[]);
    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env.clone(), msg).unwrap();
    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0000", config.owner.as_str());
    assert_eq!("uusd", config.base_denom.to_string());
    assert_eq!("mint0000", config.mint_contract.as_str());
    assert_eq!(100u64, config.lockup_period);
}
#[test]
fn update_config() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        mint_contract: HumanAddr::from("mint0000"),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();
    // update owner
    let env = mock_env("owner0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("owner0001".to_string())),
        mint_contract: None,
        base_denom: None,
        lockup_period: None,
    };
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());
    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0001", config.owner.as_str());
    // Unauthorized err
    let env = mock_env("owner0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        mint_contract: None,
        base_denom: None,
        lockup_period: None,
    };
    let res = handle(&mut deps, env, msg).unwrap_err();
    assert_eq!(res, StdError::unauthorized());
}

#[test]
fn lock_position_funds() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        mint_contract: HumanAddr::from("mint0000"),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128), // lock 100uusd
        }],
    );
    let msg = HandleMsg::LockPositionFundsHook {
        position_idx: Uint128(1u128),
        receiver: HumanAddr::from("addr0000"),
    };

    // unauthorized attempt
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, StdError::unauthorized());

    // successfull attempt
    let env = mock_env("mint0000", &[]);
    let lock_height = env.block.height;
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "lock_position_funds_hook"),
            log("position_idx", "1"),
            log("locked_amount", "100uusd"),
            log("height", lock_height.to_string()),
        ]
    );
}

#[test]
fn unlock_position_funds() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::percent(1u64),
        &[(&"uusd".to_string(), &Uint128(100000000u128))],
    );
    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        mint_contract: HumanAddr::from("mint0000"),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    // lock 100 UST 3 times at different heights
    let msg = HandleMsg::LockPositionFundsHook {
        position_idx: Uint128(1u128),
        receiver: HumanAddr::from("addr0000"),
    };
    let mut env = mock_env("mint0000", &[]);
    env.block.height = 1;
    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128), // lock 100uusd
        }],
    );
    let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    env.block.height = 10;
    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(200u128), // lock 100uusd
        }],
    );
    let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    env.block.height = 20;
    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(300u128), // lock 100uusd
        }],
    );
    let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    // query lock info
    let res: PositionLockInfoResponse = from_binary(
        &query(
            &deps,
            QueryMsg::PositionLockInfo {
                position_idx: Uint128(1u128),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        PositionLockInfoResponse {
            idx: Uint128(1u128),
            receiver: HumanAddr::from("addr0000"),
            locked_funds: vec![
                (1u64, Uint128(100u128)),
                (10u64, Uint128(100u128)),
                (20u64, Uint128(100u128)),
            ]
        }
    );

    let msg = HandleMsg::UnlockPositionFunds {
        position_idx: Uint128(1u128),
    };

    // unauthorized attempt
    let env = mock_env("addr0001", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, StdError::unauthorized());

    // nothing to unlock
    let mut env = mock_env("addr0000", &[]);
    env.block.height = 50;
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, StdError::generic_err("Nothing to unlock"));

    // unlock 100 UST
    env.block.height = 101;
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "unlock_shorting_funds"),
            log("position_idx", "1"),
            log("unlocked_amount", "100uusd"),
            log("tax_amount", "1uusd"),
        ]
    );

    // query lock info
    let res: PositionLockInfoResponse = from_binary(
        &query(
            &deps,
            QueryMsg::PositionLockInfo {
                position_idx: Uint128(1u128),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        PositionLockInfoResponse {
            idx: Uint128(1u128),
            receiver: HumanAddr::from("addr0000"),
            locked_funds: vec![(10u64, Uint128(100u128)), (20u64, Uint128(100u128)),]
        }
    );

    // unlock everything else
    env.block.height = 120;
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "unlock_shorting_funds"),
            log("position_idx", "1"),
            log("unlocked_amount", "200uusd"),
            log("tax_amount", "2uusd"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("addr0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128(198u128),
            }]
        })]
    );

    // lock info does not exist anymore
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("There are no locked funds for this position idx")
    );
}

#[test]
fn release_position_funds() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::percent(1u64),
        &[(&"uusd".to_string(), &Uint128(100000000u128))],
    );
    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        mint_contract: HumanAddr::from("mint0000"),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    // lock 100 UST
    let msg = HandleMsg::LockPositionFundsHook {
        position_idx: Uint128(1u128),
        receiver: HumanAddr::from("addr0000"),
    };
    let mut env = mock_env("mint0000", &[]);
    env.block.height = 1;
    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128), // lock 100uusd
        }],
    );
    let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

    let msg = HandleMsg::ReleasePositionFunds {
        position_idx: Uint128(1u128),
    };

    // unauthorized attempt
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap_err();
    assert_eq!(res, StdError::unauthorized());

    // only mint contract can unlock before lock period is over
    let mut env = mock_env("mint0000", &[]);
    env.block.height = 50;
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "unlock_shorting_funds"),
            log("position_idx", "1"),
            log("unlocked_amount", "100uusd"),
            log("tax_amount", "1uusd"),
        ]
    );

    // lock info does not exist anymore
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("There are no locked funds for this position idx")
    );
}
