#[cfg(test)]
mod tests {
    use crate::contract::{handle, init, query};
    use crate::state::{read_pool_info, rewards_read, store_pool_info, PoolInfo, RewardInfo};
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR,
    };
    use cosmwasm_std::{
        from_binary, from_slice, to_binary, Api, Coin, CosmosMsg, Decimal, Empty, Extern,
        HumanAddr, Querier, QuerierResult, QueryRequest, StdError, SystemError, Uint128, WasmMsg,
        WasmQuery,
    };
    use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
    use mirror_protocol::oracle::{PriceResponse, QueryMsg as OracleQueryMsg};
    use mirror_protocol::staking::{
        Cw20HookMsg, HandleMsg, InitMsg, PoolInfoResponse, QueryMsg, RewardInfoResponse,
        RewardInfoResponseItem,
    };
    use terraswap::{
        asset::Asset, asset::AssetInfo, asset::PairInfo, factory::QueryMsg as FactoryQueryMsg,
        pair::PoolResponse, pair::QueryMsg as PairQueryMsg,
    };

    #[test]
    fn test_deposit_reward() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_min_update_interval: 3600,
        };

        let env = mock_env("addr", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset"),
            staking_token: HumanAddr::from("staking"),
        };

        let env = mock_env("owner", &[]);
        let _res = handle(&mut deps, env, msg.clone()).unwrap();

        // store 3% premium rate
        let token_raw = deps
            .api
            .canonical_address(&HumanAddr::from("asset"))
            .unwrap();
        let pool_info = read_pool_info(&deps.storage, &token_raw).unwrap();
        store_pool_info(
            &mut deps.storage,
            &token_raw,
            &PoolInfo {
                premium_rate: Decimal::percent(2),
                ..pool_info
            },
        )
        .unwrap();

        // bond 100 tokens
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Bond {
                    asset_token: HumanAddr::from("asset"),
                })
                .unwrap(),
            ),
        });
        let env = mock_env("staking", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // bond 100 short token
        let msg = HandleMsg::IncreaseShortToken {
            asset_token: HumanAddr::from("asset"),
            staker_addr: HumanAddr::from("addr"),
            amount: Uint128(100u128),
        };
        let env = mock_env("mint", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // factory deposit 100 reward tokens
        // premium is 0, so rewards distributed 80:20
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("factory"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::DepositReward {
                    rewards: vec![(HumanAddr::from("asset"), Uint128(100u128))],
                })
                .unwrap(),
            ),
        });
        let env = mock_env("reward", &[]);
        let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

        // Check pool state
        let res: PoolInfoResponse = from_binary(
            &query(
                &deps,
                QueryMsg::PoolInfo {
                    asset_token: HumanAddr::from("asset"),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            res.clone(),
            PoolInfoResponse {
                total_bond_amount: Uint128(100u128),
                total_short_amount: Uint128(100u128),
                reward_index: Decimal::from_ratio(80u128, 100u128),
                short_reward_index: Decimal::from_ratio(20u128, 100u128),
                ..res
            }
        );

        // if premium_rate is over threshold, distribution weight should be 60:40
        let asset_token_raw = deps
            .api
            .canonical_address(&HumanAddr::from("asset"))
            .unwrap();
        let pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw).unwrap();
        store_pool_info(
            &mut deps.storage,
            &asset_token_raw,
            &PoolInfo {
                reward_index: Decimal::zero(),
                short_reward_index: Decimal::zero(),
                premium_rate: Decimal::percent(10),
                ..pool_info
            },
        )
        .unwrap();

        let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

        let res: PoolInfoResponse = from_binary(
            &query(
                &deps,
                QueryMsg::PoolInfo {
                    asset_token: HumanAddr::from("asset"),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            res.clone(),
            PoolInfoResponse {
                total_bond_amount: Uint128(100u128),
                total_short_amount: Uint128(100u128),
                reward_index: Decimal::from_ratio(60u128, 100u128),
                short_reward_index: Decimal::from_ratio(40u128, 100u128),
                ..res
            }
        );
    }

    #[test]
    fn test_deposit_reward_when_no_bonding() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_min_update_interval: 3600,
        };

        let env = mock_env("addr", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset"),
            staking_token: HumanAddr::from("staking"),
        };

        let env = mock_env("owner", &[]);
        let _res = handle(&mut deps, env, msg.clone()).unwrap();

        // store 3% premium rate
        let token_raw = deps
            .api
            .canonical_address(&HumanAddr::from("asset"))
            .unwrap();
        let pool_info = read_pool_info(&deps.storage, &token_raw).unwrap();
        store_pool_info(
            &mut deps.storage,
            &token_raw,
            &PoolInfo {
                premium_rate: Decimal::percent(2),
                ..pool_info
            },
        )
        .unwrap();

        // factory deposit 100 reward tokens
        // premium is 0, so rewards distributed 80:20
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("factory"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::DepositReward {
                    rewards: vec![(HumanAddr::from("asset"), Uint128(100u128))],
                })
                .unwrap(),
            ),
        });
        let env = mock_env("reward", &[]);
        let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

        // Check pool state
        let res: PoolInfoResponse = from_binary(
            &query(
                &deps,
                QueryMsg::PoolInfo {
                    asset_token: HumanAddr::from("asset"),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            res.clone(),
            PoolInfoResponse {
                reward_index: Decimal::zero(),
                short_reward_index: Decimal::zero(),
                pending_reward: Uint128(80u128),
                short_pending_reward: Uint128(20u128),
                ..res
            }
        );

        // if premium_rate is over threshold, distribution weight should be 60:40
        let asset_token_raw = deps
            .api
            .canonical_address(&HumanAddr::from("asset"))
            .unwrap();
        let pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw).unwrap();
        store_pool_info(
            &mut deps.storage,
            &asset_token_raw,
            &PoolInfo {
                pending_reward: Uint128::zero(),
                short_pending_reward: Uint128::zero(),
                premium_rate: Decimal::percent(10),
                ..pool_info
            },
        )
        .unwrap();

        let _res = handle(&mut deps, env.clone(), msg.clone()).unwrap();

        let res: PoolInfoResponse = from_binary(
            &query(
                &deps,
                QueryMsg::PoolInfo {
                    asset_token: HumanAddr::from("asset"),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            res.clone(),
            PoolInfoResponse {
                reward_index: Decimal::zero(),
                short_reward_index: Decimal::zero(),
                pending_reward: Uint128(60u128),
                short_pending_reward: Uint128(40u128),
                ..res
            }
        );
    }

    #[test]
    fn test_before_share_changes() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_min_update_interval: 3600,
        };

        let env = mock_env("addr", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset"),
            staking_token: HumanAddr::from("staking"),
        };

        let env = mock_env("owner", &[]);
        let _res = handle(&mut deps, env, msg.clone()).unwrap();

        // store 3% premium rate
        let token_raw = deps
            .api
            .canonical_address(&HumanAddr::from("asset"))
            .unwrap();
        let pool_info = read_pool_info(&deps.storage, &token_raw).unwrap();
        store_pool_info(
            &mut deps.storage,
            &token_raw,
            &PoolInfo {
                premium_rate: Decimal::percent(2),
                ..pool_info
            },
        )
        .unwrap();

        // bond 100 tokens
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Bond {
                    asset_token: HumanAddr::from("asset"),
                })
                .unwrap(),
            ),
        });
        let env = mock_env("staking", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // bond 100 short token
        let msg = HandleMsg::IncreaseShortToken {
            asset_token: HumanAddr::from("asset"),
            staker_addr: HumanAddr::from("addr"),
            amount: Uint128(100u128),
        };
        let env = mock_env("mint", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // factory deposit 100 reward tokens
        // premium is 0, so rewards distributed 80:20
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("factory"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::DepositReward {
                    rewards: vec![(HumanAddr::from("asset"), Uint128(100u128))],
                })
                .unwrap(),
            ),
        });

        let env = mock_env("reward", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let asset_raw = deps
            .api
            .canonical_address(&HumanAddr::from("asset"))
            .unwrap();
        let addr_raw = deps
            .api
            .canonical_address(&HumanAddr::from("addr"))
            .unwrap();
        let reward_bucket = rewards_read(&deps.storage, &addr_raw, false);
        let reward_info: RewardInfo = reward_bucket.load(asset_raw.as_slice()).unwrap();
        assert_eq!(
            RewardInfo {
                pending_reward: Uint128::zero(),
                bond_amount: Uint128(100u128),
                index: Decimal::zero(),
            },
            reward_info
        );

        // bond 100 more tokens
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Bond {
                    asset_token: HumanAddr::from("asset"),
                })
                .unwrap(),
            ),
        });
        let env = mock_env("staking", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let reward_bucket = rewards_read(&deps.storage, &addr_raw, false);
        let reward_info: RewardInfo = reward_bucket.load(asset_raw.as_slice()).unwrap();
        assert_eq!(
            RewardInfo {
                pending_reward: Uint128(80u128),
                bond_amount: Uint128(200u128),
                index: Decimal::from_ratio(80u128, 100u128),
            },
            reward_info
        );

        // factory deposit 100 reward tokens; = 0.8 + 0.4 = 1.2 is reward_index
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("factory"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::DepositReward {
                    rewards: vec![(HumanAddr::from("asset"), Uint128(100u128))],
                })
                .unwrap(),
            ),
        });
        let env = mock_env("reward", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // unbond
        let msg = HandleMsg::Unbond {
            asset_token: HumanAddr::from("asset"),
            amount: Uint128(100u128),
        };
        let env = mock_env("addr", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let reward_bucket = rewards_read(&deps.storage, &addr_raw, false);
        let reward_info: RewardInfo = reward_bucket.load(asset_raw.as_slice()).unwrap();
        assert_eq!(
            RewardInfo {
                pending_reward: Uint128(160u128),
                bond_amount: Uint128(100u128),
                index: Decimal::from_ratio(120u128, 100u128),
            },
            reward_info
        );
    }

    #[test]
    fn test_withdraw() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_min_update_interval: 3600,
        };

        let env = mock_env("addr", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset"),
            staking_token: HumanAddr::from("staking"),
        };

        let env = mock_env("owner", &[]);
        let _res = handle(&mut deps, env, msg.clone()).unwrap();

        // store 3% premium rate
        let token_raw = deps
            .api
            .canonical_address(&HumanAddr::from("asset"))
            .unwrap();
        let pool_info = read_pool_info(&deps.storage, &token_raw).unwrap();
        store_pool_info(
            &mut deps.storage,
            &token_raw,
            &PoolInfo {
                premium_rate: Decimal::percent(2),
                ..pool_info
            },
        )
        .unwrap();

        // bond 100 tokens
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Bond {
                    asset_token: HumanAddr::from("asset"),
                })
                .unwrap(),
            ),
        });
        let env = mock_env("staking", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // factory deposit 100 reward tokens
        // premium_rate is zero; distribute weight => 80:20
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("factory"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::DepositReward {
                    rewards: vec![(HumanAddr::from("asset"), Uint128(100u128))],
                })
                .unwrap(),
            ),
        });
        let env = mock_env("reward", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::Withdraw {
            asset_token: Some(HumanAddr::from("asset")),
        };
        let env = mock_env("addr", &[]);
        let res = handle(&mut deps, env, msg).unwrap();

        assert_eq!(
            res.messages,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("reward"),
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("addr"),
                    amount: Uint128(80u128),
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
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_min_update_interval: 3600,
        };

        let env = mock_env("addr", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset"),
            staking_token: HumanAddr::from("staking"),
        };

        let env = mock_env("owner", &[]);
        let _res = handle(&mut deps, env, msg.clone()).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset2"),
            staking_token: HumanAddr::from("staking2"),
        };

        let env = mock_env("owner", &[]);
        let _res = handle(&mut deps, env, msg.clone()).unwrap();

        // store 3% premium rate
        let token_raw = deps
            .api
            .canonical_address(&HumanAddr::from("asset"))
            .unwrap();
        let pool_info = read_pool_info(&deps.storage, &token_raw).unwrap();
        store_pool_info(
            &mut deps.storage,
            &token_raw,
            &PoolInfo {
                premium_rate: Decimal::percent(2),
                ..pool_info
            },
        )
        .unwrap();

        // store 3% premium rate for asset2
        let token_raw = deps
            .api
            .canonical_address(&HumanAddr::from("asset2"))
            .unwrap();
        let pool_info = read_pool_info(&deps.storage, &token_raw).unwrap();
        store_pool_info(
            &mut deps.storage,
            &token_raw,
            &PoolInfo {
                premium_rate: Decimal::percent(2),
                ..pool_info
            },
        )
        .unwrap();

        // bond 100 tokens
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Bond {
                    asset_token: HumanAddr::from("asset"),
                })
                .unwrap(),
            ),
        });
        let env = mock_env("staking", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // bond second 1000 tokens
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr"),
            amount: Uint128(1000u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Bond {
                    asset_token: HumanAddr::from("asset2"),
                })
                .unwrap(),
            ),
        });
        let env = mock_env("staking2", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // bond 50 short token
        let msg = HandleMsg::IncreaseShortToken {
            asset_token: HumanAddr::from("asset"),
            staker_addr: HumanAddr::from("addr"),
            amount: Uint128(50u128),
        };
        let env = mock_env("mint", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // factory deposit asset
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("factory"),
            amount: Uint128(300u128),
            msg: Some(
                to_binary(&Cw20HookMsg::DepositReward {
                    rewards: vec![
                        (HumanAddr::from("asset"), Uint128(100u128)),
                        (HumanAddr::from("asset2"), Uint128(200u128)),
                    ],
                })
                .unwrap(),
            ),
        });
        let env = mock_env("reward", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let data = query(
            &deps,
            QueryMsg::RewardInfo {
                asset_token: None,
                staker_addr: HumanAddr::from("addr"),
            },
        )
        .unwrap();
        let res: RewardInfoResponse = from_binary(&data).unwrap();
        assert_eq!(
            res,
            RewardInfoResponse {
                staker_addr: HumanAddr::from("addr"),
                reward_infos: vec![
                    RewardInfoResponseItem {
                        asset_token: HumanAddr::from("asset"),
                        bond_amount: Uint128(100u128),
                        pending_reward: Uint128(80u128),
                        is_short: false,
                    },
                    RewardInfoResponseItem {
                        asset_token: HumanAddr::from("asset2"),
                        bond_amount: Uint128(1000u128),
                        pending_reward: Uint128(160u128),
                        is_short: false,
                    },
                    RewardInfoResponseItem {
                        asset_token: HumanAddr::from("asset"),
                        bond_amount: Uint128(50u128),
                        pending_reward: Uint128(20u128),
                        is_short: true,
                    },
                ],
            }
        );

        // withdraw all
        let msg = HandleMsg::Withdraw { asset_token: None };
        let env = mock_env("addr", &[]);
        let res = handle(&mut deps, env, msg).unwrap();

        assert_eq!(
            res.messages,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("reward"),
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("addr"),
                    amount: Uint128(260u128),
                })
                .unwrap(),
                send: vec![],
            })]
        );

        let data = query(
            &deps,
            QueryMsg::RewardInfo {
                asset_token: None,
                staker_addr: HumanAddr::from("addr"),
            },
        )
        .unwrap();
        let res: RewardInfoResponse = from_binary(&data).unwrap();
        assert_eq!(
            res,
            RewardInfoResponse {
                staker_addr: HumanAddr::from("addr"),
                reward_infos: vec![
                    RewardInfoResponseItem {
                        asset_token: HumanAddr::from("asset"),
                        bond_amount: Uint128(100u128),
                        pending_reward: Uint128::zero(),
                        is_short: false,
                    },
                    RewardInfoResponseItem {
                        asset_token: HumanAddr::from("asset2"),
                        bond_amount: Uint128(1000u128),
                        pending_reward: Uint128::zero(),
                        is_short: false,
                    },
                    RewardInfoResponseItem {
                        asset_token: HumanAddr::from("asset"),
                        bond_amount: Uint128(50u128),
                        pending_reward: Uint128::zero(),
                        is_short: true,
                    },
                ],
            }
        );
    }

    #[test]
    fn test_adjust_premium() {
        let mut deps = mock_dependencies_with_querier(20, &[]);

        // terraswap price 100
        // oracle price 100
        // premium zero
        deps.querier.with_pair_info(HumanAddr::from("pair"));
        deps.querier.with_pool_assets([
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset"),
                },
                amount: Uint128::from(1u128),
            },
        ]);
        deps.querier
            .with_oracle_price(Decimal::from_ratio(100u128, 1u128));

        let msg = InitMsg {
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_min_update_interval: 3600,
        };

        let env = mock_env("addr", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset"),
            staking_token: HumanAddr::from("staking"),
        };

        let env = mock_env("owner", &[]);
        let _res = handle(&mut deps, env, msg.clone()).unwrap();

        let msg = HandleMsg::AdjustPremium {
            asset_tokens: vec![HumanAddr::from("asset")],
        };
        let mut env = mock_env("addr", &[]);
        let _ = handle(&mut deps, env.clone(), msg.clone()).unwrap();

        // Check pool state
        let res: PoolInfoResponse = from_binary(
            &query(
                &deps,
                QueryMsg::PoolInfo {
                    asset_token: HumanAddr::from("asset"),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(res.premium_rate, Decimal::zero());
        assert_eq!(res.premium_updated_time, env.block.time);

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
                    contract_addr: HumanAddr::from("asset"),
                },
                amount: Uint128::from(1u128),
            },
        ]);

        // assert premium update interval
        let res = handle(&mut deps, env.clone(), msg.clone());
        match res {
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(
                msg,
                "cannot adjust premium before premium_min_update_interval passed"
            ),
            _ => panic!("DO NOT ENTER HERE"),
        }

        env.block.time += 3600;
        let _ = handle(&mut deps, env.clone(), msg.clone()).unwrap();

        // Check pool state
        let res: PoolInfoResponse = from_binary(
            &query(
                &deps,
                QueryMsg::PoolInfo {
                    asset_token: HumanAddr::from("asset"),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(res.premium_rate, Decimal::zero());
        assert_eq!(res.premium_updated_time, env.block.time);

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
                    contract_addr: HumanAddr::from("asset"),
                },
                amount: Uint128::from(1u128),
            },
        ]);

        env.block.time += 3600;
        let _ = handle(&mut deps, env.clone(), msg.clone()).unwrap();

        // Check pool state
        let res: PoolInfoResponse = from_binary(
            &query(
                &deps,
                QueryMsg::PoolInfo {
                    asset_token: HumanAddr::from("asset"),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(res.premium_rate, Decimal::percent(5));
        assert_eq!(res.premium_updated_time, env.block.time);
    }

    ////////////////////////////////////////////
    /// Custom Querier for premium adjustment
    pub struct WasmMockQuerier {
        base: MockQuerier,
        pair_addr: HumanAddr,
        pool_assets: [Asset; 2],
        oracle_price: Decimal,
    }

    fn mock_dependencies_with_querier(
        canonical_length: usize,
        contract_balance: &[Coin],
    ) -> Extern<MockStorage, MockApi, WasmMockQuerier> {
        let contract_addr = HumanAddr::from(MOCK_CONTRACT_ADDR);
        let custom_querier: WasmMockQuerier = WasmMockQuerier::new(
            MockQuerier::new(&[(&contract_addr, contract_balance)]),
            MockApi::new(canonical_length),
            canonical_length,
        );

        Extern {
            storage: MockStorage::default(),
            api: MockApi::new(canonical_length),
            querier: custom_querier,
        }
    }

    impl Querier for WasmMockQuerier {
        fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
            // MockQuerier doesn't support Custom, so we ignore it completely here
            let request: QueryRequest<Empty> = match from_slice(bin_request) {
                Ok(v) => v,
                Err(e) => {
                    return Err(SystemError::InvalidRequest {
                        error: format!("Parsing query request: {}", e),
                        request: bin_request.into(),
                    })
                }
            };
            self.handle_query(&request)
        }
    }

    impl WasmMockQuerier {
        pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
            match &request {
                QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: _,
                    msg,
                }) => match from_binary(&msg) {
                    Ok(FactoryQueryMsg::Pair { asset_infos }) => Ok(to_binary(&PairInfo {
                        asset_infos: asset_infos.clone(),
                        contract_addr: self.pair_addr.clone(),
                        liquidity_token: HumanAddr::default(),
                    })),
                    _ => match from_binary(&msg) {
                        Ok(PairQueryMsg::Pool {}) => Ok(to_binary(&PoolResponse {
                            assets: self.pool_assets.clone(),
                            total_share: Uint128::zero(),
                        })),
                        _ => match from_binary(&msg) {
                            Ok(OracleQueryMsg::Price {
                                base_asset: _,
                                quote_asset: _,
                            }) => Ok(to_binary(&PriceResponse {
                                rate: self.oracle_price,
                                last_updated_base: 100,
                                last_updated_quote: 100,
                            })),
                            _ => panic!("DO NOT ENTER HERE"),
                        },
                    },
                },
                _ => self.base.handle_query(request),
            }
        }
    }

    impl WasmMockQuerier {
        pub fn new<A: Api>(base: MockQuerier<Empty>, _api: A, _canonical_length: usize) -> Self {
            WasmMockQuerier {
                base,
                pair_addr: HumanAddr::default(),
                pool_assets: [
                    Asset {
                        info: AssetInfo::NativeToken {
                            denom: "uusd".to_string(),
                        },
                        amount: Uint128::zero(),
                    },
                    Asset {
                        info: AssetInfo::Token {
                            contract_addr: HumanAddr::from("asset"),
                        },
                        amount: Uint128::zero(),
                    },
                ],
                oracle_price: Decimal::zero(),
            }
        }

        pub fn with_pair_info(&mut self, pair_addr: HumanAddr) {
            self.pair_addr = pair_addr;
        }

        pub fn with_pool_assets(&mut self, pool_assets: [Asset; 2]) {
            self.pool_assets = pool_assets;
        }

        pub fn with_oracle_price(&mut self, oracle_price: Decimal) {
            self.oracle_price = oracle_price;
        }
    }
}
