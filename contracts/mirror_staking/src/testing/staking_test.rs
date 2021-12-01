use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies_with_querier;
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Coin, CosmosMsg, Decimal, StdError, SubMsg, Uint128,
    WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use mirror_protocol::staking::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg, RewardInfoResponse,
    RewardInfoResponseItem,
};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::pair::ExecuteMsg as PairExecuteMsg;

#[test]
fn test_bond_tokens() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        mirror_token: "reward".to_string(),
        mint_contract: "mint".to_string(),
        oracle_contract: "oracle".to_string(),
        terraswap_factory: "terraswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        premium_min_update_interval: 3600,
        short_reward_contract: "short_reward".to_string(),
    };

    let info = mock_info("addr", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset".to_string(),
        staking_token: "staking".to_string(),
    };

    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            asset_token: "asset".to_string(),
        })
        .unwrap(),
    });

    let info = mock_info("staking", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::RewardInfo {
            asset_token: Some("asset".to_string()),
            staker_addr: "addr".to_string(),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: "addr".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                asset_token: "asset".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::new(100u128),
                is_short: false,
                should_migrate: None,
            }],
        }
    );

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            asset_token: "asset".to_string(),
        },
    )
    .unwrap();

    let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: "asset".to_string(),
            staking_token: "staking".to_string(),
            total_bond_amount: Uint128::new(100u128),
            total_short_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::zero(),
            short_reward_weight: Decimal::zero(),
            premium_updated_time: 0,
            migration_deprecated_staking_token: None,
            migration_index_snapshot: None,
        }
    );

    // bond 100 more tokens from other account
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr2".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            asset_token: "asset".to_string(),
        })
        .unwrap(),
    });
    let info = mock_info("staking", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            asset_token: "asset".to_string(),
        },
    )
    .unwrap();
    let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: "asset".to_string(),
            staking_token: "staking".to_string(),
            total_bond_amount: Uint128::new(200u128),
            total_short_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::zero(),
            short_reward_weight: Decimal::zero(),
            premium_updated_time: 0,
            migration_deprecated_staking_token: None,
            migration_index_snapshot: None,
        }
    );

    // failed with unauthorized
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            asset_token: "asset".to_string(),
        })
        .unwrap(),
    });

    let info = mock_info("staking2", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_unbond() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        mirror_token: "reward".to_string(),
        mint_contract: "mint".to_string(),
        oracle_contract: "oracle".to_string(),
        terraswap_factory: "terraswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        premium_min_update_interval: 3600,
        short_reward_contract: "short_reward".to_string(),
    };

    let info = mock_info("addr", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // register asset
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset".to_string(),
        staking_token: "staking".to_string(),
    };

    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            asset_token: "asset".to_string(),
        })
        .unwrap(),
    });
    let info = mock_info("staking", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // unbond 150 tokens; failed
    let msg = ExecuteMsg::Unbond {
        asset_token: "asset".to_string(),
        amount: Uint128::new(150u128),
    };

    let info = mock_info("addr", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot unbond more than bond amount");
        }
        _ => panic!("Must return generic error"),
    };

    // normal unbond
    let msg = ExecuteMsg::Unbond {
        asset_token: "asset".to_string(),
        amount: Uint128::new(100u128),
    };

    let info = mock_info("addr", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "staking".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr".to_string(),
                amount: Uint128::new(100u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            asset_token: "asset".to_string(),
        },
    )
    .unwrap();
    let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: "asset".to_string(),
            staking_token: "staking".to_string(),
            total_bond_amount: Uint128::zero(),
            total_short_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::zero(),
            short_reward_weight: Decimal::zero(),
            premium_updated_time: 0,
            migration_deprecated_staking_token: None,
            migration_index_snapshot: None,
        }
    );

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::RewardInfo {
            asset_token: None,
            staker_addr: "addr".to_string(),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: "addr".to_string(),
            reward_infos: vec![],
        }
    );
}

#[test]
fn test_increase_short_token() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        mirror_token: "reward".to_string(),
        mint_contract: "mint".to_string(),
        oracle_contract: "oracle".to_string(),
        terraswap_factory: "terraswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        premium_min_update_interval: 3600,
        short_reward_contract: "short_reward".to_string(),
    };

    let info = mock_info("addr", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset".to_string(),
        staking_token: "staking".to_string(),
    };

    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::IncreaseShortToken {
        asset_token: "asset".to_string(),
        staker_addr: "addr".to_string(),
        amount: Uint128::from(100u128),
    };

    let info = mock_info("addr", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info("mint", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        vec![
            attr("action", "increase_short_token"),
            attr("staker_addr", "addr"),
            attr("asset_token", "asset"),
            attr("amount", "100"),
        ],
        res.attributes
    );

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            asset_token: "asset".to_string(),
        },
    )
    .unwrap();

    let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: "asset".to_string(),
            staking_token: "staking".to_string(),
            total_bond_amount: Uint128::zero(),
            total_short_amount: Uint128::from(100u128),
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::zero(),
            short_reward_weight: Decimal::zero(),
            premium_updated_time: 0,
            migration_deprecated_staking_token: None,
            migration_index_snapshot: None,
        }
    );

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::RewardInfo {
            asset_token: None,
            staker_addr: "addr".to_string(),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: "addr".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                asset_token: "asset".to_string(),
                pending_reward: Uint128::zero(),
                bond_amount: Uint128::new(100u128),
                is_short: true,
                should_migrate: None,
            }],
        }
    );
}

#[test]
fn test_decrease_short_token() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        mirror_token: "reward".to_string(),
        mint_contract: "mint".to_string(),
        oracle_contract: "oracle".to_string(),
        terraswap_factory: "terraswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        premium_min_update_interval: 3600,
        short_reward_contract: "short_reward".to_string(),
    };

    let info = mock_info("addr", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset".to_string(),
        staking_token: "staking".to_string(),
    };

    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::IncreaseShortToken {
        asset_token: "asset".to_string(),
        staker_addr: "addr".to_string(),
        amount: Uint128::from(100u128),
    };

    let info = mock_info("mint", &[]);
    let _ = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::DecreaseShortToken {
        asset_token: "asset".to_string(),
        staker_addr: "addr".to_string(),
        amount: Uint128::from(100u128),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        vec![
            attr("action", "decrease_short_token"),
            attr("staker_addr", "addr"),
            attr("asset_token", "asset"),
            attr("amount", "100"),
        ],
        res.attributes
    );

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            asset_token: "asset".to_string(),
        },
    )
    .unwrap();

    let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: "asset".to_string(),
            staking_token: "staking".to_string(),
            total_bond_amount: Uint128::zero(),
            total_short_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::zero(),
            short_reward_weight: Decimal::zero(),
            premium_updated_time: 0,
            migration_deprecated_staking_token: None,
            migration_index_snapshot: None,
        }
    );

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::RewardInfo {
            asset_token: None,
            staker_addr: "addr".to_string(),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: "addr".to_string(),
            reward_infos: vec![],
        }
    );
}

#[test]
fn test_auto_stake() {
    let mut deps = mock_dependencies_with_querier(&[]);
    deps.querier.with_pair_info(Addr::unchecked("pair"));
    deps.querier.with_pool_assets([
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(100u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: "asset".to_string(),
            },
            amount: Uint128::from(1u128),
        },
    ]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        mirror_token: "reward".to_string(),
        mint_contract: "mint".to_string(),
        oracle_contract: "oracle".to_string(),
        terraswap_factory: "terraswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        premium_min_update_interval: 3600,
        short_reward_contract: "short_reward".to_string(),
    };

    let info = mock_info("addr", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset".to_string(),
        staking_token: "lptoken".to_string(),
    };

    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // no token asset
    let msg = ExecuteMsg::AutoStake {
        assets: [
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::new(100u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::new(100u128),
            },
        ],
        slippage_tolerance: None,
    };
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(100u128),
        }],
    );
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(res, StdError::generic_err("Missing token asset"));

    // no native asset
    let msg = ExecuteMsg::AutoStake {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: "asset".to_string(),
                },
                amount: Uint128::from(1u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: "asset".to_string(),
                },
                amount: Uint128::from(1u128),
            },
        ],
        slippage_tolerance: None,
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(res, StdError::generic_err("Missing native asset"));

    let msg = ExecuteMsg::AutoStake {
        assets: [
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::new(100u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: "asset".to_string(),
                },
                amount: Uint128::new(1u128),
            },
        ],
        slippage_tolerance: None,
    };

    // attempt with no coins
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err(
            "Native token balance mismatch between the argument and the transferred"
        )
    );

    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(100u128),
        }],
    );
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: "addr0000".to_string(),
                    recipient: MOCK_CONTRACT_ADDR.to_string(),
                    amount: Uint128::new(1u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                    spender: "pair".to_string(),
                    amount: Uint128::new(1),
                    expires: None,
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "pair".to_string(),
                msg: to_binary(&PairExecuteMsg::ProvideLiquidity {
                    assets: [
                        Asset {
                            info: AssetInfo::NativeToken {
                                denom: "uusd".to_string()
                            },
                            amount: Uint128::new(99u128),
                        },
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: "asset".to_string()
                            },
                            amount: Uint128::new(1u128),
                        },
                    ],
                    slippage_tolerance: None,
                    receiver: None,
                })
                .unwrap(),
                funds: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::new(99u128), // 1% tax
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                msg: to_binary(&ExecuteMsg::AutoStakeHook {
                    asset_token: "asset".to_string(),
                    staking_token: "lptoken".to_string(),
                    staker_addr: "addr0000".to_string(),
                    prev_staking_token_amount: Uint128::new(0),
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );

    deps.querier.with_token_balance(Uint128::new(100u128)); // recive 100 lptoken

    // wrong asset
    let msg = ExecuteMsg::AutoStakeHook {
        asset_token: "asset1".to_string(),
        staking_token: "lptoken".to_string(),
        staker_addr: "addr0000".to_string(),
        prev_staking_token_amount: Uint128::new(0),
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err(); // pool not found error

    // valid msg
    let msg = ExecuteMsg::AutoStakeHook {
        asset_token: "asset".to_string(),
        staking_token: "lptoken".to_string(),
        staker_addr: "addr0000".to_string(),
        prev_staking_token_amount: Uint128::new(0),
    };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));

    // successfull attempt
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond"),
            attr("staker_addr", "addr0000"),
            attr("asset_token", "asset"),
            attr("amount", "100"),
        ]
    );

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            asset_token: "asset".to_string(),
        },
    )
    .unwrap();
    let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: "asset".to_string(),
            staking_token: "lptoken".to_string(),
            total_bond_amount: Uint128::new(100u128),
            total_short_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::zero(),
            short_reward_weight: Decimal::zero(),
            premium_updated_time: 0,
            migration_deprecated_staking_token: None,
            migration_index_snapshot: None,
        }
    );
}
