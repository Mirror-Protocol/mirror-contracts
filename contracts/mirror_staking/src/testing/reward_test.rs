use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies_with_querier;
use crate::state::{read_pool_info, rewards_read, store_pool_info, PoolInfo, RewardInfo};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{
    from_binary, to_binary, Addr, Api, CosmosMsg, Decimal, StdError, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use mirror_protocol::staking::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg, RewardInfoResponse,
    RewardInfoResponseItem,
};
use terraswap::asset::{Asset, AssetInfo};

#[test]
fn test_deposit_reward() {
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

    // store 3% premium rate
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

    // bond 100 short token
    let msg = ExecuteMsg::IncreaseShortToken {
        asset_token: "asset".to_string(),
        staker_addr: "addr".to_string(),
        amount: Uint128::new(100u128),
    };
    let info = mock_info("mint", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // factory deposit 100 reward tokens
    // premium is 0, so rewards distributed 80:20
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "factory".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {
            rewards: vec![("asset".to_string(), Uint128::new(100u128))],
        })
        .unwrap(),
    });
    let info = mock_info("reward", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    // Check pool state
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
            total_short_amount: Uint128::new(100u128),
            reward_index: Decimal::from_ratio(80u128, 100u128),
            short_reward_index: Decimal::from_ratio(20u128, 100u128),
            ..res
        }
    );

    // if premium_rate is over threshold, distribution weight should be 60:40
    let asset_token_raw = deps.api.addr_canonicalize("asset").unwrap();
    let pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw).unwrap();
    store_pool_info(
        &mut deps.storage,
        &asset_token_raw,
        &PoolInfo {
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            premium_rate: Decimal::percent(10),
            short_reward_weight: Decimal::percent(40),
            ..pool_info
        },
    )
    .unwrap();

    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

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
            total_short_amount: Uint128::new(100u128),
            reward_index: Decimal::from_ratio(60u128, 100u128),
            short_reward_index: Decimal::from_ratio(40u128, 100u128),
            ..res
        }
    );
}

#[test]
fn test_deposit_reward_when_no_bonding() {
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

    // store 3% premium rate
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

    // factory deposit 100 reward tokens
    // premium is 0, so rewards distributed 80:20
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "factory".to_string(),
        amount: Uint128::new(100u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {
            rewards: vec![("asset".to_string(), Uint128::new(100u128))],
        })
        .unwrap(),
    });
    let info = mock_info("reward", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    // Check pool state
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
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::new(80u128),
            short_pending_reward: Uint128::new(20u128),
            ..res
        }
    );

    // if premium_rate is over threshold, distribution weight should be 60:40
    let asset_token_raw = deps.api.addr_canonicalize("asset").unwrap();
    let pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw).unwrap();
    store_pool_info(
        &mut deps.storage,
        &asset_token_raw,
        &PoolInfo {
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::percent(10),
            short_reward_weight: Decimal::percent(40),
            ..pool_info
        },
    )
    .unwrap();

    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

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
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::new(60u128),
            short_pending_reward: Uint128::new(40u128),
            ..res
        }
    );
}

#[test]
fn test_before_share_changes() {
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

    // store 3% premium rate
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

    // bond 100 short token
    let msg = ExecuteMsg::IncreaseShortToken {
        asset_token: "asset".to_string(),
        staker_addr: "addr".to_string(),
        amount: Uint128::new(100u128),
    };
    let info = mock_info("mint", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // factory deposit 100 reward tokens
    // premium is 0, so rewards distributed 80:20
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

    let asset_raw = deps.api.addr_canonicalize("asset").unwrap();
    let addr_raw = deps.api.addr_canonicalize("addr").unwrap();
    let reward_bucket = rewards_read(&deps.storage, &addr_raw, false);
    let reward_info: RewardInfo = reward_bucket.load(asset_raw.as_slice()).unwrap();
    assert_eq!(
        RewardInfo {
            pending_reward: Uint128::zero(),
            bond_amount: Uint128::new(100u128),
            index: Decimal::zero(),
        },
        reward_info
    );

    // bond 100 more tokens
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

    let reward_bucket = rewards_read(&deps.storage, &addr_raw, false);
    let reward_info: RewardInfo = reward_bucket.load(asset_raw.as_slice()).unwrap();
    assert_eq!(
        RewardInfo {
            pending_reward: Uint128::new(80u128),
            bond_amount: Uint128::new(200u128),
            index: Decimal::from_ratio(80u128, 100u128),
        },
        reward_info
    );

    // factory deposit 100 reward tokens; = 0.8 + 0.4 = 1.2 is reward_index
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

    // unbond
    let msg = ExecuteMsg::Unbond {
        asset_token: "asset".to_string(),
        amount: Uint128::new(100u128),
    };
    let info = mock_info("addr", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let reward_bucket = rewards_read(&deps.storage, &addr_raw, false);
    let reward_info: RewardInfo = reward_bucket.load(asset_raw.as_slice()).unwrap();
    assert_eq!(
        RewardInfo {
            pending_reward: Uint128::new(160u128),
            bond_amount: Uint128::new(100u128),
            index: Decimal::from_ratio(120u128, 100u128),
        },
        reward_info
    );
}

#[test]
fn test_withdraw() {
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

    // store 3% premium rate
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

    // factory deposit 100 reward tokens
    // premium_rate is zero; distribute weight => 80:20
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

    let msg = ExecuteMsg::Withdraw {
        asset_token: Some("asset".to_string()),
    };
    let info = mock_info("addr", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "reward".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr".to_string(),
                amount: Uint128::new(80u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn withdraw_multiple_rewards() {
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

    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset2".to_string(),
        staking_token: "staking2".to_string(),
    };

    let info = mock_info("owner", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // store 3% premium rate
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

    // store 3% premium rate for asset2
    let token_raw = deps.api.addr_canonicalize("asset2").unwrap();
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

    // bond second 1000 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr".to_string(),
        amount: Uint128::new(1000u128),
        msg: to_binary(&Cw20HookMsg::Bond {
            asset_token: "asset2".to_string(),
        })
        .unwrap(),
    });
    let info = mock_info("staking2", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 50 short token
    let msg = ExecuteMsg::IncreaseShortToken {
        asset_token: "asset".to_string(),
        staker_addr: "addr".to_string(),
        amount: Uint128::new(50u128),
    };
    let info = mock_info("mint", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // factory deposit asset
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "factory".to_string(),
        amount: Uint128::new(300u128),
        msg: to_binary(&Cw20HookMsg::DepositReward {
            rewards: vec![
                ("asset".to_string(), Uint128::new(100u128)),
                ("asset2".to_string(), Uint128::new(200u128)),
            ],
        })
        .unwrap(),
    });
    let info = mock_info("reward", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

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
                },
                RewardInfoResponseItem {
                    asset_token: "asset2".to_string(),
                    bond_amount: Uint128::new(1000u128),
                    pending_reward: Uint128::new(160u128),
                    is_short: false,
                },
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::new(50u128),
                    pending_reward: Uint128::new(20u128),
                    is_short: true,
                },
            ],
        }
    );

    // withdraw all
    let msg = ExecuteMsg::Withdraw { asset_token: None };
    let info = mock_info("addr", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "reward".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr".to_string(),
                amount: Uint128::new(260u128),
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
                    bond_amount: Uint128::new(100u128),
                    pending_reward: Uint128::zero(),
                    is_short: false,
                },
                RewardInfoResponseItem {
                    asset_token: "asset2".to_string(),
                    bond_amount: Uint128::new(1000u128),
                    pending_reward: Uint128::zero(),
                    is_short: false,
                },
                RewardInfoResponseItem {
                    asset_token: "asset".to_string(),
                    bond_amount: Uint128::new(50u128),
                    pending_reward: Uint128::zero(),
                    is_short: true,
                },
            ],
        }
    );
}

#[test]
fn test_adjust_premium() {
    let mut deps = mock_dependencies_with_querier(&[]);

    // terraswap price 100
    // oracle price 100
    // premium zero
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
    deps.querier
        .with_oracle_price(Decimal::from_ratio(100u128, 1u128));

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

    let msg = ExecuteMsg::AdjustPremium {
        asset_tokens: vec!["asset".to_string()],
    };
    let mut env = mock_env();
    let info = mock_info("addr", &[]);
    let _ = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    // Check pool state
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
    assert_eq!(res.premium_rate, Decimal::zero());
    assert_eq!(
        res.premium_updated_time,
        env.block.time.seconds()
    );

    // terraswap price = 90
    // premium rate = 0
    deps.querier.with_pool_assets([
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(90u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: "asset".to_string(),
            },
            amount: Uint128::from(1u128),
        },
    ]);

    // assert premium update interval
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(
            msg,
            "cannot adjust premium before premium_min_update_interval passed"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }

    env.block.time = env.block.time.plus_seconds(3600);
    let _ = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

    // Check pool state
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
    assert_eq!(res.premium_rate, Decimal::zero());
    assert_eq!(
        res.premium_updated_time,
        env.block.time.seconds()
    );

    // terraswap price = 105
    // premium rate = 5%
    deps.querier.with_pool_assets([
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(105u128),
        },
        Asset {
            info: AssetInfo::Token {
                contract_addr: "asset".to_string(),
            },
            amount: Uint128::from(1u128),
        },
    ]);

    env.block.time = env.block.time.plus_seconds(3600);
    let _ = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Check pool state
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
    assert_eq!(res.premium_rate, Decimal::percent(5));
    assert_eq!(
        res.premium_updated_time,
        env.block.time.seconds()
    );
}

