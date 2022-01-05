use crate::contract::{execute, instantiate, query};
use crate::state::{read_pool_info, store_pool_info, PoolInfo};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{
    from_binary, to_binary, Api, CosmosMsg, Decimal, StdError, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use mirror_protocol::staking::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg, RewardInfoResponse,
    RewardInfoResponseItem,
};

#[test]
fn test_deprecate() {
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

    let token_raw = deps.api.addr_canonicalize("asset").unwrap();
    let pool_info = read_pool_info(&deps.storage, &token_raw).unwrap();
    store_pool_info(
        &mut deps.storage,
        &token_raw,
        &PoolInfo {
            premium_rate: Decimal::percent(2),
            short_reward_weight: Decimal::percent(20),
            ..pool_info
        },
    )
    .unwrap();

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

    // bond 200 short token
    let msg = ExecuteMsg::IncreaseShortToken {
        asset_token: "asset".to_string(),
        staker_addr: "addr".to_string(),
        amount: Uint128::new(200u128),
    };
    let info = mock_info("mint", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // factory deposit 100 reward tokens
    // distribute weight => 80:20
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "factory".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {
            rewards: vec![("asset".to_string(), Uint128::new(100u128))],
        })
        .unwrap(),
    });
    let info = mock_info("reward", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // query pool and reward info
    let res: PoolInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PoolInfo {
                asset_token: "asset".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let res_cmp = res.clone();
    assert_eq!(
        res_cmp,
        PoolInfoResponse {
            total_bond_amount: Uint128::new(100u128),
            total_short_amount: Uint128::new(200u128),
            reward_index: Decimal::from_ratio(80u128, 100u128),
            short_reward_index: Decimal::from_ratio(20u128, 200u128),
            short_pending_reward: Uint128::zero(),
            migration_index_snapshot: None,
            migration_deprecated_staking_token: None,
            ..res
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
            reward_infos: vec![
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::new(100u128),
                    pending_reward: Uint128::new(80u128),
                    is_short: false,
                    should_migrate: None,
                },
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::new(200u128),
                    pending_reward: Uint128::new(20u128),
                    is_short: true,
                    should_migrate: None,
                },
            ],
        }
    );

    // execute deprecate
    let msg = ExecuteMsg::DeprecateStakingToken {
        asset_token: "asset".to_string(),
        new_staking_token: "new_staking".to_string(),
    };
    let info = mock_info("owner", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // deposit more rewards
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "factory".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {
            rewards: vec![("asset".to_string(), Uint128::new(100u128))],
        })
        .unwrap(),
    });
    let info = mock_info("reward", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // query again
    let res: PoolInfoResponse = from_binary(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PoolInfo {
                asset_token: "asset".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let res_cmp = res.clone();
    assert_eq!(
        res_cmp,
        PoolInfoResponse {
            staking_token: "new_staking".to_string(),
            total_bond_amount: Uint128::zero(), // reset
            total_short_amount: Uint128::new(200u128),
            reward_index: Decimal::from_ratio(80u128, 100u128), // stays the same
            short_reward_index: Decimal::from_ratio(40u128, 200u128), // increased 20
            short_pending_reward: Uint128::zero(),
            migration_index_snapshot: Some(Decimal::from_ratio(80u128, 100u128)),
            migration_deprecated_staking_token: Some("staking".to_string()),
            pending_reward: Uint128::new(80u128), // new reward waiting here
            ..res
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
            reward_infos: vec![
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::new(100u128),
                    pending_reward: Uint128::new(80u128), // did not change
                    is_short: false,
                    should_migrate: Some(true), // non-short pos should migrate
                },
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::new(200u128),
                    pending_reward: Uint128::new(40u128), // more rewards here
                    is_short: true,
                    should_migrate: None,
                },
            ],
        }
    );

    // try to bond new or old staking token, should fail both
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            asset_token: "asset".to_string(),
        })
        .unwrap(),
    });
    let info = mock_info("staking", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("The staking token for this asset has been migrated to new_staking")
    );
    let info = mock_info("new_staking", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("The LP token for this asset has been deprecated, withdraw all your deprecated tokens to migrate your position")
    );

    // unbond all the old tokens
    let msg = ExecuteMsg::Unbond {
        asset_token: "asset".to_string(),
        amount: Uint128::new(100u128),
    };
    let info = mock_info("addr", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    // make sure that we are receiving deprecated lp tokens tokens
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
            reward_infos: vec![
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::zero(),
                    pending_reward: Uint128::new(80u128), // still the same
                    is_short: false,
                    should_migrate: None, // now its back to empty
                },
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::new(200u128),
                    pending_reward: Uint128::new(40u128),
                    is_short: true,
                    should_migrate: None,
                },
            ],
        }
    );

    // now can bond the new staking token
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            asset_token: "asset".to_string(),
        })
        .unwrap(),
    });
    let info = mock_info("new_staking", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // deposit new rewards
    // will also add to the index the pending rewards from before the migration
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "factory".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {
            rewards: vec![("asset".to_string(), Uint128::new(100u128))],
        })
        .unwrap(),
    });
    let info = mock_info("reward", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // expect to have 80 * 3 rewards
    // initial + deposit after deprecation + deposit after bonding again
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
            reward_infos: vec![
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::new(100u128),
                    pending_reward: Uint128::new(240u128), // 80 * 3
                    is_short: false,
                    should_migrate: None,
                },
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::new(200u128),
                    pending_reward: Uint128::new(60u128), // 40 + 20
                    is_short: true,
                    should_migrate: None,
                },
            ],
        }
    );

    // completely new users can bond
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "newaddr".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            asset_token: "asset".to_string(),
        })
        .unwrap(),
    });
    let info = mock_info("new_staking", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let data = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::RewardInfo {
            asset_token: None,
            staker_addr: "newaddr".to_string(),
        },
    )
    .unwrap();
    let res: RewardInfoResponse = from_binary(&data).unwrap();
    assert_eq!(
        res,
        RewardInfoResponse {
            staker_addr: "newaddr".to_string(),
            reward_infos: vec![RewardInfoResponseItem {
                asset_token: "asset".to_string(),
                bond_amount: Uint128::new(100u128),
                pending_reward: Uint128::zero(),
                is_short: false,
                should_migrate: None,
            },],
        }
    );
}
