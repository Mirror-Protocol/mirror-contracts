use crate::contract::{handle, init, query};
use crate::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, log, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, HumanAddr, StdError,
    Uint128,
};
use mirror_protocol::lock::{
    ConfigResponse, HandleMsg, InitMsg, PositionLockInfoResponse, QueryMsg,
};

fn mock_env_with_block_time<U: Into<HumanAddr>>(sender: U, sent: &[Coin], time: u64) -> Env {
    let env = mock_env(sender, sent);
    // register time
    return Env {
        block: BlockInfo {
            height: 1,
            time,
            chain_id: "columbus".to_string(),
        },
        ..env
    };
}

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
    let env = mock_env_with_block_time("mint0000", &[], 20u64);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "lock_position_funds_hook"),
            log("position_idx", "1"),
            log("locked_amount", "100uusd"),
            log("total_locked_amount", "100uusd"),
            log("unlock_time", "120"),
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
            locked_amount: Uint128(100u128),
            unlock_time: 120u64,
        }
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
    let env = mock_env_with_block_time("mint0000", &[], 1u64);
    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128), // lock 100uusd
        }],
    );
    let _res = handle(&mut deps, env, msg.clone()).unwrap();
    let env = mock_env_with_block_time("mint0000", &[], 10u64);
    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(200u128), // lock 100uusd more
        }],
    );
    let _res = handle(&mut deps, env, msg.clone()).unwrap();

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
            locked_amount: Uint128(200),
            unlock_time: 10u64 + 100u64, // from last lock time
        }
    );

    let msg = HandleMsg::UnlockPositionFunds {
        positions_idx: vec![Uint128(1u128)],
    };

    // unauthorized attempt
    let env = mock_env("addr0001", &[]);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("There are no unlockable funds for the provided positions")
    );

    // nothing to unlock
    let env = mock_env_with_block_time("addr0000", &[], 50u64);
    let res = handle(&mut deps, env, msg.clone()).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("There are no unlockable funds for the provided positions")
    );

    // unlock 200 UST
    let env = mock_env_with_block_time("addr0000", &[], 120u64);
    let res = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "unlock_shorting_funds"),
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
                amount: Uint128(198u128), // minus tax
            }]
        })]
    );

    // lock info does not exist anymore
    let env = mock_env_with_block_time("addr0000", &[], 120u64);
    let res = handle(&mut deps, env, msg.clone()).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("There are no unlockable funds for the provided positions")
    );

    // lock 2 different positions
    let msg = HandleMsg::LockPositionFundsHook {
        position_idx: Uint128(2u128),
        receiver: HumanAddr::from("addr0000"),
    };
    let env = mock_env_with_block_time("mint0000", &[], 1u64);
    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128), // lock 100uusd
        }],
    );
    handle(&mut deps, env, msg.clone()).unwrap();

    let msg = HandleMsg::LockPositionFundsHook {
        position_idx: Uint128(3u128),
        receiver: HumanAddr::from("addr0000"),
    };
    let env = mock_env_with_block_time("mint0000", &[], 2u64);
    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(300u128), // lock 200uusd
        }],
    );
    handle(&mut deps, env, msg).unwrap();

    // unlock both positions
    let msg = HandleMsg::UnlockPositionFunds {
        positions_idx: vec![Uint128(2u128), Uint128(3u128)],
    };
    let env = mock_env_with_block_time("addr0000", &[], 102);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "unlock_shorting_funds"),
            log("unlocked_amount", "300uusd"),
            log("tax_amount", "3uusd"),
        ]
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
    let env = mock_env_with_block_time("mint0000", &[], 1u64);
    deps.querier.with_bank_balance(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128), // lock 100uusd
        }],
    );
    let _res = handle(&mut deps, env, msg.clone()).unwrap();

    let msg = HandleMsg::ReleasePositionFunds {
        position_idx: Uint128(1u128),
    };

    // unauthorized attempt
    let env = mock_env_with_block_time("addr0000", &[], 1u64);
    let res = handle(&mut deps, env, msg.clone()).unwrap_err();
    assert_eq!(res, StdError::unauthorized());

    // only mint contract can unlock before lock period is over
    let env = mock_env_with_block_time("mint0000", &[], 50u64);
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "release_shorting_funds"),
            log("position_idx", "1"),
            log("unlocked_amount", "100uusd"),
            log("tax_amount", "1uusd"),
        ]
    );

    // lock info does not exist anymore, graceful return
    let res = handle(&mut deps, env, msg.clone()).unwrap();
    assert_eq!(res.log.len(), 0);
}
