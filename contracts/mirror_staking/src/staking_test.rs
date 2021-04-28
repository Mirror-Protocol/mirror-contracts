#[cfg(test)]
mod tests {
    use crate::contract::{handle, init, query};
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{
        from_binary, log, to_binary, CosmosMsg, Decimal, HumanAddr, StdError, Uint128, WasmMsg,
    };
    use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
    use mirror_protocol::staking::{
        Cw20HookMsg, HandleMsg, InitMsg, PoolInfoResponse, QueryMsg, RewardInfoResponse,
        RewardInfoResponseItem,
    };

    #[test]
    fn test_bond_tokens() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_tolerance: Decimal::percent(2),
            short_reward_weight: Decimal::percent(20),
            premium_short_reward_weight: Decimal::percent(40),
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
        let data = query(
            &deps,
            QueryMsg::RewardInfo {
                asset_token: Some(HumanAddr::from("asset")),
                staker_addr: HumanAddr::from("addr"),
            },
        )
        .unwrap();
        let res: RewardInfoResponse = from_binary(&data).unwrap();
        assert_eq!(
            res,
            RewardInfoResponse {
                staker_addr: HumanAddr::from("addr"),
                reward_infos: vec![RewardInfoResponseItem {
                    asset_token: HumanAddr::from("asset"),
                    pending_reward: Uint128::zero(),
                    bond_amount: Uint128(100u128),
                    is_short: false,
                }],
            }
        );

        let data = query(
            &deps,
            QueryMsg::PoolInfo {
                asset_token: HumanAddr::from("asset"),
            },
        )
        .unwrap();

        let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
        assert_eq!(
            pool_info,
            PoolInfoResponse {
                asset_token: HumanAddr::from("asset"),
                staking_token: HumanAddr::from("staking"),
                total_bond_amount: Uint128(100u128),
                total_short_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                short_reward_index: Decimal::zero(),
                pending_reward: Uint128::zero(),
                short_pending_reward: Uint128::zero(),
                premium_rate: Decimal::zero(),
                premium_updated_time: 0,
            }
        );

        // bond 100 more tokens from other account
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr2"),
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

        let data = query(
            &deps,
            QueryMsg::PoolInfo {
                asset_token: HumanAddr::from("asset"),
            },
        )
        .unwrap();
        let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
        assert_eq!(
            pool_info,
            PoolInfoResponse {
                asset_token: HumanAddr::from("asset"),
                staking_token: HumanAddr::from("staking"),
                total_bond_amount: Uint128(200u128),
                total_short_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                short_reward_index: Decimal::zero(),
                pending_reward: Uint128::zero(),
                short_pending_reward: Uint128::zero(),
                premium_rate: Decimal::zero(),
                premium_updated_time: 0,
            }
        );

        // failed with unauthorized
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

        let env = mock_env("staking2", &[]);
        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }
    }

    #[test]
    fn test_unbond() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_tolerance: Decimal::percent(2),
            short_reward_weight: Decimal::percent(20),
            premium_short_reward_weight: Decimal::percent(40),
            premium_min_update_interval: 3600,
        };

        let env = mock_env("addr", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        // register asset
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset"),
            staking_token: HumanAddr::from("staking"),
        };

        let env = mock_env("owner", &[]);
        let _res = handle(&mut deps, env, msg.clone()).unwrap();

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

        // unbond 150 tokens; failed
        let msg = HandleMsg::Unbond {
            asset_token: HumanAddr::from("asset"),
            amount: Uint128(150u128),
        };

        let env = mock_env("addr", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Cannot unbond more than bond amount");
            }
            _ => panic!("Must return generic error"),
        };

        // normal unbond
        let msg = HandleMsg::Unbond {
            asset_token: HumanAddr::from("asset"),
            amount: Uint128(100u128),
        };

        let env = mock_env("addr", &[]);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            res.messages,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("staking"),
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("addr"),
                    amount: Uint128(100u128),
                })
                .unwrap(),
                send: vec![],
            })]
        );

        let data = query(
            &deps,
            QueryMsg::PoolInfo {
                asset_token: HumanAddr::from("asset"),
            },
        )
        .unwrap();
        let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
        assert_eq!(
            pool_info,
            PoolInfoResponse {
                asset_token: HumanAddr::from("asset"),
                staking_token: HumanAddr::from("staking"),
                total_bond_amount: Uint128::zero(),
                total_short_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                short_reward_index: Decimal::zero(),
                pending_reward: Uint128::zero(),
                short_pending_reward: Uint128::zero(),
                premium_rate: Decimal::zero(),
                premium_updated_time: 0,
            }
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
                reward_infos: vec![],
            }
        );
    }

    #[test]
    fn test_increase_short_token() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_tolerance: Decimal::percent(2),
            short_reward_weight: Decimal::percent(20),
            premium_short_reward_weight: Decimal::percent(40),
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

        let msg = HandleMsg::IncreaseShortToken {
            asset_token: HumanAddr::from("asset"),
            staker_addr: HumanAddr::from("addr"),
            amount: Uint128::from(100u128),
        };

        let env = mock_env("addr", &[]);
        let res = handle(&mut deps, env, msg.clone());
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("DO NOT ENTER HERE"),
        }

        let env = mock_env("mint", &[]);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            vec![
                log("action", "increase_short_token"),
                log("staker_addr", "addr"),
                log("asset_token", "asset"),
                log("amount", "100"),
            ],
            res.log
        );

        let data = query(
            &deps,
            QueryMsg::PoolInfo {
                asset_token: HumanAddr::from("asset"),
            },
        )
        .unwrap();

        let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
        assert_eq!(
            pool_info,
            PoolInfoResponse {
                asset_token: HumanAddr::from("asset"),
                staking_token: HumanAddr::from("staking"),
                total_bond_amount: Uint128::zero(),
                total_short_amount: Uint128::from(100u128),
                reward_index: Decimal::zero(),
                short_reward_index: Decimal::zero(),
                pending_reward: Uint128::zero(),
                short_pending_reward: Uint128::zero(),
                premium_rate: Decimal::zero(),
                premium_updated_time: 0,
            }
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
                reward_infos: vec![RewardInfoResponseItem {
                    asset_token: HumanAddr::from("asset"),
                    pending_reward: Uint128::zero(),
                    bond_amount: Uint128(100u128),
                    is_short: true,
                }],
            }
        );
    }

    #[test]
    fn test_decrease_short_token() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr::from("owner"),
            mirror_token: HumanAddr::from("reward"),
            mint_contract: HumanAddr::from("mint"),
            oracle_contract: HumanAddr::from("oracle"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            premium_tolerance: Decimal::percent(2),
            short_reward_weight: Decimal::percent(20),
            premium_short_reward_weight: Decimal::percent(40),
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

        let msg = HandleMsg::IncreaseShortToken {
            asset_token: HumanAddr::from("asset"),
            staker_addr: HumanAddr::from("addr"),
            amount: Uint128::from(100u128),
        };

        let env = mock_env("mint", &[]);
        let _ = handle(&mut deps, env.clone(), msg).unwrap();

        let msg = HandleMsg::DecreaseShortToken {
            asset_token: HumanAddr::from("asset"),
            staker_addr: HumanAddr::from("addr"),
            amount: Uint128::from(100u128),
        };
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            vec![
                log("action", "decrease_short_token"),
                log("staker_addr", "addr"),
                log("asset_token", "asset"),
                log("amount", "100"),
            ],
            res.log
        );

        let data = query(
            &deps,
            QueryMsg::PoolInfo {
                asset_token: HumanAddr::from("asset"),
            },
        )
        .unwrap();

        let pool_info: PoolInfoResponse = from_binary(&data).unwrap();
        assert_eq!(
            pool_info,
            PoolInfoResponse {
                asset_token: HumanAddr::from("asset"),
                staking_token: HumanAddr::from("staking"),
                total_bond_amount: Uint128::zero(),
                total_short_amount: Uint128::zero(),
                reward_index: Decimal::zero(),
                short_reward_index: Decimal::zero(),
                pending_reward: Uint128::zero(),
                short_pending_reward: Uint128::zero(),
                premium_rate: Decimal::zero(),
                premium_updated_time: 0,
            }
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
                reward_infos: vec![],
            }
        );
    }
}
