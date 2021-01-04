use crate::contract::{handle, init, query};
use cosmwasm_std::testing::{mock_dependencies, mock_env};
use cosmwasm_std::{
    from_binary, log, to_binary, CosmosMsg, Decimal, HumanAddr, StdError, Uint128, WasmMsg,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use mirror_protocol::staking::{
    ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, PoolInfoResponse, QueryMsg,
    RewardInfoResponse, RewardInfoResponseItem,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0000", config.owner.as_str());
    assert_eq!("reward0000", config.mirror_token.as_str());
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        mirror_token: HumanAddr::from("mirror0000"),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    // update owner
    let env = mock_env("owner0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("owner0001".to_string())),
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0001", config.owner.as_str());

    // Unauthorzied err
    let env = mock_env("owner0000", &[]);
    let msg = HandleMsg::UpdateConfig { owner: None };

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_register() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    // failed with unauthorized error
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg.clone()).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let env = mock_env("owner0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "register_asset"),
            log("asset_token", "asset0000"),
        ]
    );

    let res = query(
        &deps,
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
            total_bond_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
        }
    );
}

#[test]
fn test_bond_tokens() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg.clone()).unwrap();

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });

    let env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let data = query(
        &deps,
        QueryMsg::RewardInfo {
            asset_token: Some(HumanAddr::from("asset0000")),
            staker: HumanAddr::from("addr0000"),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
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

    let data = query(
        &deps,
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
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let data = query(
        &deps,
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
            })
            .unwrap(),
        ),
    });

    let env = mock_env("staking0001", &[]);
    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_deposit_reward() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg.clone()).unwrap();

    let deposit_msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("owner0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::DepositReward {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });

    let env = mock_env("reward0000", &[]);
    let _res = handle(&mut deps, env, deposit_msg.clone()).unwrap();

    let data = query(
        &deps,
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
            total_bond_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            pending_reward: Uint128::from(100u128),
        }
    );

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // unauthoirzed
    let env = mock_env("wrongtoken", &[]);
    let res = handle(&mut deps, env, deposit_msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }

    // factory deposit 100 reward tokens
    let env = mock_env("reward0000", &[]);
    let _res = handle(&mut deps, env, deposit_msg).unwrap();

    let data = query(
        &deps,
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
            reward_index: Decimal::from_ratio(2u128, 1u128),
            pending_reward: Uint128::zero(),
        }
    );
}

#[test]
fn test_unbond() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // register asset
    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg.clone()).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // unbond 150 tokens; failed
    let msg = HandleMsg::Unbond {
        asset_token: HumanAddr::from("asset0000"),
        amount: Uint128(150u128),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
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
    let res = handle(&mut deps, env, msg).unwrap();
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

#[test]
fn test_before_share_changes() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg.clone()).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // factory deposit 100 reward tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("factory0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::DepositReward {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("reward0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // bond 100 more tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let data = query(
        &deps,
        QueryMsg::RewardInfo {
            asset_token: Some(HumanAddr::from("asset0000")),
            staker: HumanAddr::from("addr0000"),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_infos: vec![RewardInfoResponseItem {
                asset_token: HumanAddr::from("asset0000"),
                index: Decimal::one(),
                pending_reward: Uint128(100u128),
                bond_amount: Uint128(200u128),
            }],
        }
    );

    // factory deposit 100 reward tokens; 1 + 0.5 = 1.5 is reward_index
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("factory0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::DepositReward {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("reward0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // unbond
    let msg = HandleMsg::Unbond {
        asset_token: HumanAddr::from("asset0000"),
        amount: Uint128(100u128),
    };
    let env = mock_env("addr0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let data = query(
        &deps,
        QueryMsg::RewardInfo {
            asset_token: Some(HumanAddr::from("asset0000")),
            staker: HumanAddr::from("addr0000"),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_infos: vec![RewardInfoResponseItem {
                asset_token: HumanAddr::from("asset0000"),
                index: Decimal::from_ratio(3u64, 2u64),
                pending_reward: Uint128(200u128),
                bond_amount: Uint128(100u128),
            }],
        }
    );
}

#[test]
fn test_withdraw() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg.clone()).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // factory deposit 100 reward tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("factory0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::DepositReward {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("reward0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Withdraw {
        asset_token: Some(HumanAddr::from("asset0000")),
    };
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("reward0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128(100u128),
            })
            .unwrap(),
            send: vec![],
        })]
    );
}

#[test]
fn withdraw_multiple_rewards() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        mirror_token: HumanAddr("reward0000".to_string()),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        staking_token: HumanAddr::from("staking0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg.clone()).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0001"),
        staking_token: HumanAddr::from("staking0001"),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg.clone()).unwrap();

    // bond 100 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // bond second 1000 tokens
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Bond {
                asset_token: HumanAddr::from("asset0001"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("staking0001", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // factory deposit asset0000
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("factory0000"),
        amount: Uint128(100u128),
        msg: Some(
            to_binary(&Cw20HookMsg::DepositReward {
                asset_token: HumanAddr::from("asset0000"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("reward0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // factory deposit asset0001
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("factory0000"),
        amount: Uint128(200u128),
        msg: Some(
            to_binary(&Cw20HookMsg::DepositReward {
                asset_token: HumanAddr::from("asset0001"),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("reward0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let data = query(
        &deps,
        QueryMsg::RewardInfo {
            asset_token: None,
            staker: HumanAddr::from("addr0000"),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_infos: vec![
                RewardInfoResponseItem {
                    asset_token: HumanAddr::from("asset0000"),
                    index: Decimal::zero(),
                    bond_amount: Uint128(100u128),
                    pending_reward: Uint128(0u128),
                },
                RewardInfoResponseItem {
                    asset_token: HumanAddr::from("asset0001"),
                    index: Decimal::zero(),
                    bond_amount: Uint128(1000u128),
                    pending_reward: Uint128(0u128),
                }
            ],
        }
    );

    // withdraw all
    let msg = HandleMsg::Withdraw { asset_token: None };
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("reward0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128(300u128),
            })
            .unwrap(),
            send: vec![],
        })]
    );

    let data = query(
        &deps,
        QueryMsg::RewardInfo {
            asset_token: None,
            staker: HumanAddr::from("addr0000"),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker: HumanAddr::from("addr0000"),
            reward_infos: vec![
                RewardInfoResponseItem {
                    asset_token: HumanAddr::from("asset0000"),
                    index: Decimal::one(),
                    bond_amount: Uint128(100u128),
                    pending_reward: Uint128::zero(),
                },
                RewardInfoResponseItem {
                    asset_token: HumanAddr::from("asset0001"),
                    index: Decimal::from_ratio(200u128, 1000u128),
                    bond_amount: Uint128(1000u128),
                    pending_reward: Uint128::zero(),
                }
            ],
        }
    );
}
