#[cfg(test)]
mod test {
    use crate::contract::{handle, init, query};
    use crate::mock_querier::mock_dependencies;
    use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{
        from_binary, log, to_binary, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, HumanAddr,
        Uint128, WasmMsg,
    };
    use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
    use mirror_protocol::lock::HandleMsg as LockHandleMsg;
    use mirror_protocol::mint::{
        Cw20HookMsg, HandleMsg, InitMsg, PositionResponse, QueryMsg, ShortParams,
    };
    use mirror_protocol::staking::HandleMsg as StakingHandleMsg;
    use terraswap::{
        asset::{Asset, AssetInfo},
        pair::Cw20HookMsg as PairCw20HookMsg,
    };

    static TOKEN_CODE_ID: u64 = 10u64;
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
    fn open_short_position() {
        let mut deps = mock_dependencies(20, &[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            collateral_oracle: HumanAddr::from("collateraloracle0000"),
            staking: HumanAddr::from("staking0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            lock: HumanAddr::from("lock0000"),
            base_denom: base_denom.clone(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0001"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &HumanAddr::from("pair0000"),
        )]);

        let msg = HandleMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(1000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(1000000u128),
            }],
            1000,
        );
        let res = handle(&mut deps, env.clone(), msg).unwrap();

        assert_eq!(
            res.log,
            vec![
                log("action", "open_position"),
                log("position_idx", "1"),
                log("mint_amount", "666666asset0000"),
                log("collateral_amount", "1000000uusd"),
                log("is_short", "true"),
            ]
        );

        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Mint {
                        recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                        amount: Uint128(666666u128),
                    })
                    .unwrap()
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Send {
                        contract: HumanAddr::from("pair0000"),
                        amount: Uint128(666666u128),
                        msg: Some(
                            to_binary(&PairCw20HookMsg::Swap {
                                belief_price: None,
                                max_spread: None,
                                to: Some(HumanAddr::from("lock0000")),
                            })
                            .unwrap()
                        )
                    })
                    .unwrap()
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("lock0000"),
                    send: vec![],
                    msg: to_binary(&LockHandleMsg::LockPositionFundsHook {
                        position_idx: Uint128(1u128),
                        receiver: HumanAddr::from("addr0000"),
                    })
                    .unwrap(),
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("staking0000"),
                    send: vec![],
                    msg: to_binary(&StakingHandleMsg::IncreaseShortToken {
                        asset_token: HumanAddr::from("asset0000"),
                        staker_addr: HumanAddr::from("addr0000"),
                        amount: Uint128(666666u128),
                    })
                    .unwrap(),
                })
            ]
        );

        let res = query(
            &deps,
            QueryMsg::Position {
                position_idx: Uint128(1u128),
            },
        )
        .unwrap();
        let position: PositionResponse = from_binary(&res).unwrap();
        assert_eq!(
            position,
            PositionResponse {
                idx: Uint128(1u128),
                owner: HumanAddr::from("addr0000"),
                asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0000"),
                    },
                    amount: Uint128(666666u128),
                },
                collateral: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    amount: Uint128(1000000u128),
                },
                is_short: true,
            }
        );
    }

    #[test]
    fn mint_short_position() {
        let mut deps = mock_dependencies(20, &[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            collateral_oracle: HumanAddr::from("collateraloracle0000"),
            staking: HumanAddr::from("staking0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            lock: HumanAddr::from("lock0000"),
            base_denom: base_denom.clone(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0001"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &HumanAddr::from("pair0000"),
        )]);

        let msg = HandleMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(1000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            collateral_ratio: Decimal::percent(200),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(1000000u128),
            }],
            1000,
        );
        let _res = handle(&mut deps, env.clone(), msg).unwrap();

        // mint more tokens from the short position
        let msg = HandleMsg::Mint {
            position_idx: Uint128(1u128),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                amount: Uint128(100u128),
            },
            short_params: None,
        };
        let env = mock_env_with_block_time("addr0000", &[], 1000);
        let res = handle(&mut deps, env.clone(), msg).unwrap();
        assert_eq!(
            res.log,
            vec![
                log("action", "mint"),
                log("position_idx", "1"),
                log("mint_amount", "100asset0000"),
            ]
        );

        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Mint {
                        recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                        amount: Uint128(100u128),
                    })
                    .unwrap()
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Send {
                        contract: HumanAddr::from("pair0000"),
                        amount: Uint128(100u128),
                        msg: Some(
                            to_binary(&PairCw20HookMsg::Swap {
                                belief_price: None,
                                max_spread: None,
                                to: Some(HumanAddr::from("lock0000")),
                            })
                            .unwrap()
                        )
                    })
                    .unwrap()
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("lock0000"),
                    send: vec![],
                    msg: to_binary(&LockHandleMsg::LockPositionFundsHook {
                        position_idx: Uint128(1u128),
                        receiver: HumanAddr::from("addr0000"),
                    })
                    .unwrap(),
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("staking0000"),
                    send: vec![],
                    msg: to_binary(&StakingHandleMsg::IncreaseShortToken {
                        asset_token: HumanAddr::from("asset0000"),
                        staker_addr: HumanAddr::from("addr0000"),
                        amount: Uint128(100u128),
                    })
                    .unwrap(),
                })
            ]
        );
    }

    #[test]
    fn burn_short_position() {
        let mut deps = mock_dependencies(20, &[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            collateral_oracle: HumanAddr::from("collateraloracle0000"),
            staking: HumanAddr::from("staking0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            lock: HumanAddr::from("lock0000"),
            base_denom: base_denom.clone(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0001"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &HumanAddr::from("pair0000"),
        )]);

        let msg = HandleMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(1000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            collateral_ratio: Decimal::percent(200),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(1000000u128),
            }],
            1000,
        );
        let _res = handle(&mut deps, env.clone(), msg).unwrap();

        // burn asset tokens from the short position
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr0000"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Burn {
                    position_idx: Uint128(1u128),
                })
                .unwrap(),
            ),
        });
        let env = mock_env("asset0000", &[]);
        let res = handle(&mut deps, env.clone(), msg).unwrap();

        assert_eq!(
            res.log,
            vec![
                log("action", "burn"),
                log("position_idx", "1"),
                log("burn_amount", "100asset0000"),
            ]
        );

        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Burn {
                        amount: Uint128(100u128),
                    })
                    .unwrap(),
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("staking0000"),
                    send: vec![],
                    msg: to_binary(&StakingHandleMsg::DecreaseShortToken {
                        staker_addr: HumanAddr::from("addr0000"),
                        asset_token: HumanAddr::from("asset0000"),
                        amount: Uint128(100u128),
                    })
                    .unwrap(),
                })
            ]
        );
    }

    #[test]
    fn auction_short_position() {
        let mut deps = mock_dependencies(20, &[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            staking: HumanAddr::from("staking0000"),
            collateral_oracle: HumanAddr::from("collateraloracle0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            lock: HumanAddr::from("lock0000"),
            base_denom: base_denom.clone(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0001"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &HumanAddr::from("pair0000"),
        )]);

        let msg = HandleMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(1000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(1000000u128),
            }],
            1000,
        );
        let _res = handle(&mut deps, env.clone(), msg).unwrap();

        // asset price increased
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(115)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr0000"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Auction {
                    position_idx: Uint128(1u128),
                })
                .unwrap(),
            ),
        });
        let env = mock_env_with_block_time(HumanAddr::from("asset0000"), &[], 1000);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            res.log,
            vec![
                log("action", "auction"),
                log("position_idx", "1"),
                log("owner", "addr0000"),
                log("return_collateral_amount", "142uusd"),
                log("liquidated_amount", "100asset0000"),
                log("tax_amount", "0uusd"),
                log("protocol_fee", "1uusd"),
            ]
        );

        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    msg: to_binary(&Cw20HandleMsg::Burn {
                        amount: Uint128(100u128),
                    })
                    .unwrap(),
                    send: vec![],
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    to_address: HumanAddr::from("addr0000"),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128(142u128)
                    }],
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    to_address: HumanAddr::from("collector0000"),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128(1u128)
                    }]
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("staking0000"),
                    send: vec![],
                    msg: to_binary(&StakingHandleMsg::DecreaseShortToken {
                        staker_addr: HumanAddr::from("addr0000"),
                        asset_token: HumanAddr::from("asset0000"),
                        amount: Uint128(100u128),
                    })
                    .unwrap(),
                })
            ]
        );
    }

    #[test]
    fn close_short_position() {
        let mut deps = mock_dependencies(20, &[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            collateral_oracle: HumanAddr::from("collateraloracle0000"),
            staking: HumanAddr::from("staking0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            lock: HumanAddr::from("lock0000"),
            base_denom: base_denom.clone(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(200),
            ipo_params: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &HumanAddr::from("pair0000"),
        )]);

        let msg = HandleMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(200u128), // will mint 100 mAsset and lock 100 UST
            },
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            collateral_ratio: Decimal::percent(200),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(200u128),
            }],
            1000,
        );
        let _res = handle(&mut deps, env.clone(), msg).unwrap();

        // burn all asset tokens from the short position
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr0000"),
            amount: Uint128(100u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Burn {
                    position_idx: Uint128(1u128),
                })
                .unwrap(),
            ),
        });
        let env = mock_env("asset0000", &[]);
        let _res = handle(&mut deps, env.clone(), msg).unwrap();

        // withdraw all collateral
        let msg = HandleMsg::Withdraw {
            position_idx: Uint128(1u128),
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(200u128),
            },
        };
        let env = mock_env_with_block_time("addr0000", &[], 1000);
        let res = handle(&mut deps, env.clone(), msg).unwrap();

        dbg!(&res.messages);
        // refunds collateral and releases locked funds from lock contract
        assert_eq!(
            res.messages.contains(&CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("lock0000"),
                send: vec![],
                msg: to_binary(&LockHandleMsg::ReleasePositionFunds {
                    position_idx: Uint128(1u128),
                })
                .unwrap(),
            })),
            true
        );
    }
}
