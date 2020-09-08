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
    from_binary, to_binary, CosmosMsg, Decimal, HandleResponse, HandleResult, HumanAddr,
    InitResponse, StdError, Uint128, WasmMsg,
};
use cosmwasm_vm::testing::{handle, init, mock_env, mock_instance, query};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use mirror_staking::msg::{ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, QueryMsg};
use mirror_staking::state::{PoolInfo, RewardInfo};

// This line will test the output of cargo wasm
static WASM: &[u8] = include_bytes!("../target/wasm32-unknown-unknown/release/mirror_staking.wasm");
// You can uncomment this line instead to test productionified build from rust-optimizer
// static WASM: &[u8] = include_bytes!("../contract.wasm");

#[test]
fn proper_initialization() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        staking_token: HumanAddr("staking0000".to_string()),
        reward_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let value: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("staking0000", value.staking_token.as_str());
    assert_eq!("reward0000", value.reward_token.as_str());
}

#[test]
fn test_bond_tokens() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        staking_token: HumanAddr("staking0000".to_string()),
        reward_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    let env = mock_env("staking0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    let res = query(
        &mut deps,
        QueryMsg::RewardInfo {
            address: HumanAddr::from("addr0000"),
        },
    )
    .unwrap();
    let value: RewardInfo = from_binary(&res).unwrap();
    assert_eq!(value.bond_amount, Uint128(100u128));
    assert_eq!(value.pending_reward, Uint128::zero());
    assert_eq!(value.index, Decimal::zero());

    let res = query(&mut deps, QueryMsg::PoolInfo {}).unwrap();
    let value: PoolInfo = from_binary(&res).unwrap();
    assert_eq!(value.reward_index, Decimal::zero());
    assert_eq!(value.total_bond_amount, Uint128(100u128));

    // bond 100 more tokens from other account
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0001"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    let env = mock_env("staking0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    let res = query(&mut deps, QueryMsg::PoolInfo {}).unwrap();
    let value: PoolInfo = from_binary(&res).unwrap();
    assert_eq!(value.reward_index, Decimal::zero());
    assert_eq!(value.total_bond_amount, Uint128(200u128));

    // failed with unautorized
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    let env = mock_env("staking0001", &[]);
    let res: HandleResult = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_deposit_reward() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        staking_token: HumanAddr("staking0000".to_string()),
        reward_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    let env = mock_env("staking0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // unauthoirzed
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("factory0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::DepositReward {}).unwrap()),
    });

    let env = mock_env("wrongtoken", &[]);
    let res: HandleResult = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }

    // factory deposit 100 reward tokens
    let env = mock_env("reward0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    let res = query(&mut deps, QueryMsg::PoolInfo {}).unwrap();
    let value: PoolInfo = from_binary(&res).unwrap();
    assert_eq!(value.reward_index, Decimal::one());
    assert_eq!(value.total_bond_amount, Uint128(100u128));
}

#[test]
fn test_unbond() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        staking_token: HumanAddr("staking0000".to_string()),
        reward_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(to_binary(&Cw20HookMsg::Bond {}).unwrap()),
    });
    let env = mock_env("staking0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // unbond 150 tokens; failed
    let msg = HandleMsg::Unbond {
        amount: Uint128(150u128),
    };

    let env = mock_env("addr0000", &[]);
    let res: HandleResult = handle(&mut deps, env, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot unbond more than bond amount");
        }
        _ => panic!("Must return generic error"),
    };

    // normal unbond
    let msg = HandleMsg::Unbond {
        amount: Uint128(100u128),
    };

    let env = mock_env("addr0000", &[]);
    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("staking0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128(100u128),
            })
            .unwrap(),
            send: vec![],
        })]
    );
}
