use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, from_binary, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, StdError, SubMsg,
    Timestamp, Uint128,
};
use mirror_protocol::lock::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, PositionLockInfoResponse, QueryMsg,
};

fn mock_env_with_block_time(time: u64) -> Env {
    let env = mock_env();
    // register time
    Env {
        block: BlockInfo {
            height: 1,
            time: Timestamp::from_seconds(time),
            chain_id: "columbus".to_string(),
        },
        ..env
    }
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0000", config.owner.as_str());
    assert_eq!("uusd", config.base_denom);
    assert_eq!("mint0000", config.mint_contract.as_str());
    assert_eq!(100u64, config.lockup_period);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    // update owner
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("owner0001".to_string()),
        mint_contract: None,
        base_denom: None,
        lockup_period: None,
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());
    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0001", config.owner.as_str());
    // Unauthorized err
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        mint_contract: None,
        base_denom: None,
        lockup_period: None,
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));
}

#[test]
fn lock_position_funds() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    deps.querier.with_bank_balance(
        &MOCK_CONTRACT_ADDR.to_string(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128), // lock 100uusd
        }],
    );
    let msg = ExecuteMsg::LockPositionFundsHook {
        position_idx: Uint128::from(1u128),
        receiver: "addr0000".to_string(),
    };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));

    // successfull attempt
    let env = mock_env_with_block_time(20u64);
    let info = mock_info("mint0000", &[]);

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "lock_position_funds_hook"),
            attr("position_idx", "1"),
            attr("locked_amount", "100uusd"),
            attr("total_locked_amount", "100uusd"),
            attr("unlock_time", "120"),
        ]
    );

    // query lock info
    let res: PositionLockInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PositionLockInfo {
                position_idx: Uint128::from(1u128),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        PositionLockInfoResponse {
            idx: Uint128::from(1u128),
            receiver: "addr0000".to_string(),
            locked_amount: Uint128::from(100u128),
            unlock_time: 120u64,
        }
    );
}

#[test]
fn unlock_position_funds() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(1u64),
        &[(&"uusd".to_string(), &Uint128::from(100000000u128))],
    );
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // lock 100 UST 3 times at different heights
    let msg = ExecuteMsg::LockPositionFundsHook {
        position_idx: Uint128::from(1u128),
        receiver: "addr0000".to_string(),
    };
    let env = mock_env_with_block_time(1u64);
    let info = mock_info("mint0000", &[]);

    deps.querier.with_bank_balance(
        &MOCK_CONTRACT_ADDR.to_string(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128), // lock 100uusd
        }],
    );
    let _res = execute(deps.as_mut(), env, info, msg.clone()).unwrap();

    let env = mock_env_with_block_time(10u64);
    let info = mock_info("mint0000", &[]);

    deps.querier.with_bank_balance(
        &MOCK_CONTRACT_ADDR.to_string(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(200u128), // lock 100uusd more
        }],
    );
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // query lock info
    let res: PositionLockInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PositionLockInfo {
                position_idx: Uint128::from(1u128),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        res,
        PositionLockInfoResponse {
            idx: Uint128::from(1u128),
            receiver: "addr0000".to_string(),
            locked_amount: Uint128::from(200u128),
            unlock_time: 10u64 + 100u64, // from last lock time
        }
    );

    let msg = ExecuteMsg::UnlockPositionFunds {
        positions_idx: vec![Uint128::from(1u128)],
    };

    // unauthorized attempt
    let info = mock_info("addr0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("There are no unlockable funds for the provided positions")
    );

    // nothing to unlock
    let env = mock_env_with_block_time(50u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg.clone()).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("There are no unlockable funds for the provided positions")
    );

    // unlock 200 UST
    let env = mock_env_with_block_time(120u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "unlock_shorting_funds"),
            attr("unlocked_amount", "200uusd"),
            attr("tax_amount", "2uusd"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "addr0000".to_string(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(198u128), // minus tax
            }]
        }))]
    );

    // lock info does not exist anymore
    let env = mock_env_with_block_time(120u64);
    let info = mock_info("mint0000", &[]);
    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("There are no unlockable funds for the provided positions")
    );

    // lock 2 different positions
    let msg = ExecuteMsg::LockPositionFundsHook {
        position_idx: Uint128::from(2u128),
        receiver: "addr0000".to_string(),
    };
    let env = mock_env_with_block_time(1u64);
    deps.querier.with_bank_balance(
        &MOCK_CONTRACT_ADDR.to_string(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128), // lock 100uusd
        }],
    );
    execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::LockPositionFundsHook {
        position_idx: Uint128::from(3u128),
        receiver: "addr0000".to_string(),
    };
    let env = mock_env_with_block_time(2u64);
    let info = mock_info("mint0000", &[]);
    deps.querier.with_bank_balance(
        &MOCK_CONTRACT_ADDR.to_string(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(300u128), // lock 200uusd
        }],
    );
    execute(deps.as_mut(), env, info, msg).unwrap();

    // unlock both positions
    let msg = ExecuteMsg::UnlockPositionFunds {
        positions_idx: vec![Uint128::from(2u128), Uint128::from(3u128)],
    };
    let env = mock_env_with_block_time(102);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "unlock_shorting_funds"),
            attr("unlocked_amount", "300uusd"),
            attr("tax_amount", "3uusd"),
        ]
    );
}

#[test]
fn release_position_funds() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(1u64),
        &[(&"uusd".to_string(), &Uint128::from(100000000u128))],
    );
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
        lockup_period: 100u64,
    };
    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // lock 100 UST
    let msg = ExecuteMsg::LockPositionFundsHook {
        position_idx: Uint128::from(1u128),
        receiver: "addr0000".to_string(),
    };
    let env = mock_env_with_block_time(1u64);
    let info = mock_info("mint0000", &[]);
    deps.querier.with_bank_balance(
        &MOCK_CONTRACT_ADDR.to_string(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128), // lock 100uusd
        }],
    );
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::ReleasePositionFunds {
        position_idx: Uint128::from(1u128),
    };

    // unauthorized attempt
    let env = mock_env_with_block_time(1u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg.clone()).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));

    // only mint contract can unlock before lock period is over
    let env = mock_env_with_block_time(50u64);
    let info = mock_info("mint0000", &[]);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "release_shorting_funds"),
            attr("position_idx", "1"),
            attr("unlocked_amount", "100uusd"),
            attr("tax_amount", "1uusd"),
        ]
    );

    // lock info does not exist anymore
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(res.attributes.len(), 0);
}
