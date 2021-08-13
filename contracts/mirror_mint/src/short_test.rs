#[cfg(test)]
mod test {
    use crate::contract::{execute, instantiate, query};
    use crate::mock_querier::mock_dependencies;
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{
        attr, from_binary, to_binary, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, SubMsg,
        Timestamp, Uint128, WasmMsg,
    };
    use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
    use mirror_protocol::lock::ExecuteMsg as LockExecuteMsg;
    use mirror_protocol::mint::{
        Cw20HookMsg, ExecuteMsg, InstantiateMsg, PositionResponse, QueryMsg, ShortParams,
    };
    use mirror_protocol::staking::ExecuteMsg as StakingExecuteMsg;
    use terraswap::{
        asset::{Asset, AssetInfo},
        pair::Cw20HookMsg as PairCw20HookMsg,
    };

    static TOKEN_CODE_ID: u64 = 10u64;
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
    fn open_short_position() {
        let mut deps = mock_dependencies(&[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InstantiateMsg {
            owner: "owner0000".to_string(),
            oracle: "oracle0000".to_string(),
            collector: "collector0000".to_string(),
            collateral_oracle: "collateraloracle0000".to_string(),
            staking: "staking0000".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            lock: "lock0000".to_string(),
            base_denom,
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        let msg = ExecuteMsg::RegisterAsset {
            asset_token: "asset0000".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        let msg = ExecuteMsg::RegisterAsset {
            asset_token: "asset0001".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let env = mock_env();
        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &"pair0000".to_string(),
        )]);

        let msg = ExecuteMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(1000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(1000000u128),
            }],
        );
        let res = execute(deps.as_mut(), env, info, msg).unwrap();

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "open_position"),
                attr("position_idx", "1"),
                attr("mint_amount", "666666asset0000"),
                attr("collateral_amount", "1000000uusd"),
                attr("is_short", "true"),
            ]
        );

        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Mint {
                        recipient: MOCK_CONTRACT_ADDR.to_string(),
                        amount: Uint128::from(666666u128),
                    })
                    .unwrap()
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: "pair0000".to_string(),
                        amount: Uint128::from(666666u128),
                        msg: to_binary(&PairCw20HookMsg::Swap {
                            belief_price: None,
                            max_spread: None,
                            to: Some("lock0000".to_string()),
                        })
                        .unwrap()
                    })
                    .unwrap()
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "lock0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&LockExecuteMsg::LockPositionFundsHook {
                        position_idx: Uint128::from(1u128),
                        receiver: "addr0000".to_string(),
                    })
                    .unwrap(),
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "staking0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&StakingExecuteMsg::IncreaseShortToken {
                        asset_token: "asset0000".to_string(),
                        staker_addr: "addr0000".to_string(),
                        amount: Uint128::from(666666u128),
                    })
                    .unwrap(),
                }))
            ]
        );

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Position {
                position_idx: Uint128::from(1u128),
            },
        )
        .unwrap();
        let position: PositionResponse = from_binary(&res).unwrap();
        assert_eq!(
            position,
            PositionResponse {
                idx: Uint128::from(1u128),
                owner: "addr0000".to_string(),
                asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: "asset0000".to_string(),
                    },
                    amount: Uint128::from(666666u128),
                },
                collateral: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    amount: Uint128::from(1000000u128),
                },
                is_short: true,
            }
        );
    }

    #[test]
    fn mint_short_position() {
        let mut deps = mock_dependencies(&[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InstantiateMsg {
            owner: "owner0000".to_string(),
            oracle: "oracle0000".to_string(),
            collector: "collector0000".to_string(),
            collateral_oracle: "collateraloracle0000".to_string(),
            staking: "staking0000".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            lock: "lock0000".to_string(),
            base_denom,
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let env = mock_env();
        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

        let msg = ExecuteMsg::RegisterAsset {
            asset_token: "asset0000".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterAsset {
            asset_token: "asset0001".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &"pair0000".to_string(),
        )]);

        let msg = ExecuteMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(1000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(200),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(1000000u128),
            }],
        );
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // mint more tokens from the short position
        let msg = ExecuteMsg::Mint {
            position_idx: Uint128::from(1u128),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: "asset0000".to_string(),
                },
                amount: Uint128::from(100u128),
            },
            short_params: None,
        };
        let env = mock_env_with_block_time(1000);
        let info = mock_info("addr0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "mint"),
                attr("position_idx", "1"),
                attr("mint_amount", "100asset0000"),
            ]
        );

        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Mint {
                        recipient: MOCK_CONTRACT_ADDR.to_string(),
                        amount: Uint128::from(100u128),
                    })
                    .unwrap()
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: "pair0000".to_string(),
                        amount: Uint128::from(100u128),
                        msg: to_binary(&PairCw20HookMsg::Swap {
                            belief_price: None,
                            max_spread: None,
                            to: Some("lock0000".to_string()),
                        })
                        .unwrap()
                    })
                    .unwrap()
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "lock0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&LockExecuteMsg::LockPositionFundsHook {
                        position_idx: Uint128::from(1u128),
                        receiver: "addr0000".to_string(),
                    })
                    .unwrap(),
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "staking0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&StakingExecuteMsg::IncreaseShortToken {
                        asset_token: "asset0000".to_string(),
                        staker_addr: "addr0000".to_string(),
                        amount: Uint128::from(100u128),
                    })
                    .unwrap(),
                }))
            ]
        );
    }

    #[test]
    fn burn_short_position() {
        let mut deps = mock_dependencies(&[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InstantiateMsg {
            owner: "owner0000".to_string(),
            oracle: "oracle0000".to_string(),
            collector: "collector0000".to_string(),
            collateral_oracle: "collateraloracle0000".to_string(),
            staking: "staking0000".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            lock: "lock0000".to_string(),
            base_denom,
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterAsset {
            asset_token: "asset0000".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterAsset {
            asset_token: "asset0001".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &"pair0000".to_string(),
        )]);

        let msg = ExecuteMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(1000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(200),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(1000000u128),
            }],
        );
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // burn asset tokens from the short position
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "addr0000".to_string(),
            amount: Uint128::from(100u128),
            msg: to_binary(&Cw20HookMsg::Burn {
                position_idx: Uint128::from(1u128),
            })
            .unwrap(),
        });
        let env = mock_env_with_block_time(1000);
        let info = mock_info("asset0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap();

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "burn"),
                attr("position_idx", "1"),
                attr("burn_amount", "100asset0000"), // value = 100
                attr("protocol_fee", "1uusd"),
            ]
        );

        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Burn {
                        amount: Uint128::from(100u128),
                    })
                    .unwrap(),
                })),
                SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                    to_address: "collector0000".to_string(),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::from(1u128)
                    }],
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "staking0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&StakingExecuteMsg::DecreaseShortToken {
                        staker_addr: "addr0000".to_string(),
                        asset_token: "asset0000".to_string(),
                        amount: Uint128::from(100u128),
                    })
                    .unwrap(),
                }))
            ]
        );
    }

    #[test]
    fn auction_short_position() {
        let mut deps = mock_dependencies(&[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InstantiateMsg {
            owner: "owner0000".to_string(),
            oracle: "oracle0000".to_string(),
            collector: "collector0000".to_string(),
            staking: "staking0000".to_string(),
            collateral_oracle: "collateraloracle0000".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            lock: "lock0000".to_string(),
            base_denom,
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterAsset {
            asset_token: "asset0000".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterAsset {
            asset_token: "asset0001".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            ipo_params: None,
        };

        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &"pair0000".to_string(),
        )]);

        let msg = ExecuteMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(1000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(1000000u128),
            }],
        );
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // asset price increased
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(115)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);

        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "addr0000".to_string(),
            amount: Uint128::from(100u128),
            msg: to_binary(&Cw20HookMsg::Auction {
                position_idx: Uint128::from(1u128),
            })
            .unwrap(),
        });
        let env = mock_env_with_block_time(1000);
        let info = mock_info("asset0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "auction"),
                attr("position_idx", "1"),
                attr("owner", "addr0000"),
                attr("return_collateral_amount", "142uusd"),
                attr("liquidated_amount", "100asset0000"),
                attr("tax_amount", "0uusd"),
                attr("protocol_fee", "1uusd"),
            ]
        );

        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Burn {
                        amount: Uint128::from(100u128),
                    })
                    .unwrap(),
                    funds: vec![],
                })),
                SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                    to_address: "addr0000".to_string(),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::from(142u128)
                    }],
                })),
                SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                    to_address: "collector0000".to_string(),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::from(1u128)
                    }]
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "staking0000".to_string(),
                    funds: vec![],
                    msg: to_binary(&StakingExecuteMsg::DecreaseShortToken {
                        staker_addr: "addr0000".to_string(),
                        asset_token: "asset0000".to_string(),
                        amount: Uint128::from(100u128),
                    })
                    .unwrap(),
                }))
            ]
        );
    }

    #[test]
    fn close_short_position() {
        let mut deps = mock_dependencies(&[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
        ]);

        let base_denom = "uusd".to_string();

        let msg = InstantiateMsg {
            owner: "owner0000".to_string(),
            oracle: "oracle0000".to_string(),
            collector: "collector0000".to_string(),
            collateral_oracle: "collateraloracle0000".to_string(),
            staking: "staking0000".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            lock: "lock0000".to_string(),
            base_denom,
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };

        let info = mock_info("addr0000", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg::RegisterAsset {
            asset_token: "asset0000".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(200),
            ipo_params: None,
        };

        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // register terraswap pair
        deps.querier.with_terraswap_pair(&[(
            &"uusd".to_string(),
            &"asset0000".to_string(),
            &"pair0000".to_string(),
        )]);

        let msg = ExecuteMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(200u128), // will mint 100 mAsset and lock 100 UST
            },
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(200),
            short_params: Some(ShortParams {
                belief_price: None,
                max_spread: None,
            }),
        };

        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(200u128),
            }],
        );
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // burn all asset tokens from the short position
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "addr0000".to_string(),
            amount: Uint128::from(100u128),
            msg: to_binary(&Cw20HookMsg::Burn {
                position_idx: Uint128::from(1u128),
            })
            .unwrap(),
        });
        let env = mock_env_with_block_time(1000);
        let info = mock_info("asset0000", &[]);
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // withdraw all collateral
        let msg = ExecuteMsg::Withdraw {
            position_idx: Uint128::from(1u128),
            collateral: Some(Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(199u128), // 1 collateral spent as protocol fee
            }),
        };
        let env = mock_env_with_block_time(1000);
        let info = mock_info("addr0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap();

        dbg!(&res.messages);
        // refunds collateral and releases locked funds from lock contract
        assert!(res
            .messages
            .contains(&SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "lock0000".to_string(),
                funds: vec![],
                msg: to_binary(&LockExecuteMsg::ReleasePositionFunds {
                    position_idx: Uint128::from(1u128),
                })
                .unwrap(),
            }))))
    }
}
