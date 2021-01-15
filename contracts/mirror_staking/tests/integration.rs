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
    from_binary, to_binary, Coin, CosmosMsg, Decimal, HandleResponse, HandleResult, HumanAddr,
    InitResponse, StdError, Uint128, WasmMsg,
};
use cosmwasm_vm::testing::{
    handle, init, mock_dependencies, mock_env, query, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_vm::Instance;
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use mirror_protocol::staking::{
    ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, PoolInfoResponse, QueryMsg,
    RewardInfoResponse, RewardInfoResponseItem,
};

// This line will test the output of cargo wasm
static WASM: &[u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/mirror_staking.wasm");
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
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let value: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0000", value.owner.as_str());
    assert_eq!("reward0000", value.mirror_token.as_str());
}

#[test]
fn test_bond_tokens() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
                staker: None,
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    let res = query(
        &mut deps,
        QueryMsg::RewardInfo {
            staker: HumanAddr::from("addr0000"),
            asset_token: Some(HumanAddr::from("asset0000")),
        },
    )
    .unwrap();
    let reward_info: RewardInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        reward_info,
        RewardInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_infos: vec![RewardInfoResponseItem {
                asset_token: HumanAddr::from("asset0000"),
                index: Decimal::zero(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128(100u128),
            }],
        }
    );

    let res = query(
        &mut deps,
        QueryMsg::PoolInfo {
            asset_token: HumanAddr::from("asset0000"),
        },
    )
    .unwrap();
    let pool_info: PoolInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: HumanAddr::from("asset0000"),
            staking_token: HumanAddr::from("staking0000"),
            total_bond_amount: Uint128(100u128),
            reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
        }
    );

    // bond 100 more tokens from other account
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0001"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
                staker: None,
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    let data = query(
        &mut deps,
        QueryMsg::PoolInfo {
            asset_token: HumanAddr::from("asset0000"),
        },
    )
    .unwrap();
    let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: HumanAddr::from("asset0000"),
            staking_token: HumanAddr::from("staking0000"),
            total_bond_amount: Uint128(200u128),
            reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
        }
    );

    // failed with unautorized
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
                staker: None,
            })
            .unwrap(),
        ),
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
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
                staker: None,
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // unauthoirzed
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("owner0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::DepositReward {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
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

    let data = query(
        &mut deps,
        QueryMsg::PoolInfo {
            asset_token: HumanAddr::from("asset0000"),
        },
    )
    .unwrap();
    let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: HumanAddr::from("asset0000"),
            staking_token: HumanAddr::from("staking0000"),
            total_bond_amount: Uint128(100u128),
            reward_index: Decimal::one(),
            pending_reward: Uint128::zero(),
        }
    );
}

#[test]
fn test_unbond() {
    let mut deps = mock_instance(WASM, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    // register asset
    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg.clone()).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
                staker: None,
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // unbond 150 tokens; failed
    let msg = HandleMsg::Unbond {
        asset_token: HumanAddr::from("asset0000"),
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
        asset_token: HumanAddr::from("asset0000"),
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
