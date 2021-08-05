#[cfg(test)]
mod tests {
    use crate::contract::{execute, instantiate, query};
    use crate::mock_querier::mock_dependencies;
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{
        attr, from_binary, to_binary, Addr, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env,
        StdError, Timestamp, Uint128, WasmMsg,
    };
    use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
    use mirror_protocol::collateral_oracle::ExecuteMsg::RevokeCollateralAsset;
    use mirror_protocol::mint::{
        AssetConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PositionResponse,
        PositionsResponse, QueryMsg,
    };
    use terraswap::asset::{Asset, AssetInfo};

    static TOKEN_CODE_ID: u64 = 10u64;
    fn mock_env_with_block_time(time: u64) -> Env {
        let env = mock_env();
        // register time
        return Env {
            block: BlockInfo {
                height: 1,
                time: Timestamp::from_seconds(time),
                chain_id: "columbus".to_string(),
            },
            ..env
        };
    }

    #[test]
    fn register_migration() {
        let mut deps = mock_dependencies(&[]);
        deps.querier.with_token_balances(&[(
            &"asset0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
        )]);

        let base_denom = "uusd".to_string();

        let msg = InstantiateMsg {
            owner: "owner0000".to_string(),
            oracle: "oracle0000".to_string(),
            collector: "collector0000".to_string(),
            collateral_oracle: "collateraloracle0000".to_string(),
            staking: "staking0000".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            lock: "lock0000".to_string(),
            base_denom: base_denom.clone(),
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

        let msg = ExecuteMsg::RegisterMigration {
            asset_token: "asset0000".to_string(),
            end_price: Decimal::from_ratio(2u128, 1u128),
        };
        let info = mock_info("addr0000", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        match res {
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = ExecuteMsg::RegisterMigration {
            asset_token: "asset0001".to_string(),
            end_price: Decimal::from_ratio(2u128, 1u128),
        };
        let info = mock_info("owner0000", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        match res {
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "no asset data stored"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = ExecuteMsg::RegisterMigration {
            asset_token: "asset0000".to_string(),
            end_price: Decimal::percent(50),
        };
        let info = mock_info("owner0000", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "migrate_asset"),
                attr("asset_token", "asset0000"),
                attr("end_price", "0.5"),
            ]
        );
        assert_eq!(
            res.messages,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "collateraloracle0000".to_string(),
                send: vec![],
                msg: to_binary(&RevokeCollateralAsset {
                    asset: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                })
                .unwrap(),
            })]
        );

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::AssetConfig {
                asset_token: "asset0000".to_string(),
            },
        )
        .unwrap();
        let asset_config_res: AssetConfigResponse = from_binary(&res).unwrap();
        assert_eq!(
            asset_config_res,
            AssetConfigResponse {
                token: "asset0000".to_string(),
                auction_discount: Decimal::percent(20),
                min_collateral_ratio: Decimal::percent(100),
                end_price: Some(Decimal::percent(50)),
                ipo_params: None,
            }
        );
    }
    #[test]
    fn migrated_asset() {
        let mut deps = mock_dependencies(&[]);
        deps.querier.with_token_balances(&[(
            &"asset0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
        )]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);
        deps.querier.with_collateral_infos(&[(
            &"asset0000".to_string(),
            &Decimal::percent(100),
            &Decimal::one(),
            &false,
        )]);

        let base_denom = "uusd".to_string();

        let msg = InstantiateMsg {
            owner: "owner0000".to_string(),
            oracle: "oracle0000".to_string(),
            collector: "collector0000".to_string(),
            collateral_oracle: "collateraloracle0000".to_string(),
            staking: "staking0000".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            lock: "lock0000".to_string(),
            base_denom: base_denom.clone(),
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

        // Open uusd:asset0000 position
        let msg = ExecuteMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(1000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: None,
        };
        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                amount: Uint128(1000000u128),
                denom: "uusd".to_string(),
            }],
        );
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // Open asset0000:asset0001 position
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            msg: to_binary(&Cw20HookMsg::OpenPosition {
                asset_info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0001"),
                },
                collateral_ratio: Decimal::percent(150),
                short_params: None,
            })
            .unwrap(),
            sender: "addr0000".to_string(),
            amount: Uint128(1000000u128),
        });
        let env = mock_env_with_block_time(1000);
        let info = mock_info("asset0000", &[]);
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // register migration
        let msg = ExecuteMsg::RegisterMigration {
            asset_token: "asset0000".to_string(),
            end_price: Decimal::percent(100),
        };
        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // cannot open a position with deprecated collateral
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            msg: to_binary(&Cw20HookMsg::OpenPosition {
                asset_info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0001"),
                },
                collateral_ratio: Decimal::percent(150),
                short_params: None,
            })
            .unwrap(),
            sender: "addr0000".to_string(),
            amount: Uint128(1000000u128),
        });
        let env = mock_env_with_block_time(1000);
        let info = mock_info("asset0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "The collateral asset provided is no longer valid")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot open a deprecated asset position
        let msg = ExecuteMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(100u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: None,
        };
        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                amount: Uint128(100u128),
                denom: "uusd".to_string(),
            }],
        );
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Operation is not allowed for the deprecated asset")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot do deposit more collateral to the deprecated asset position
        let msg = ExecuteMsg::Deposit {
            position_idx: Uint128(1u128),
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(1u128),
            },
        };
        let info = mock_info(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(1u128),
            }],
        );
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Operation is not allowed for the deprecated asset")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot deposit more collateral to the deprecated collateral position
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            msg: to_binary(&Cw20HookMsg::Deposit {
                position_idx: Uint128(2u128),
            })
            .unwrap(),
            sender: "addr0000".to_string(),
            amount: Uint128(1000000u128),
        });
        let info = mock_info("asset0000", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "The collateral asset provided is no longer valid")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot mint more the deprecated asset
        let msg = ExecuteMsg::Mint {
            position_idx: Uint128(1u128),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128(1u128),
            },
            short_params: None,
        };
        let info = mock_info("addr0000", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Operation is not allowed for the deprecated asset")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot mint more asset with deprecated collateral
        let msg = ExecuteMsg::Mint {
            position_idx: Uint128(2u128),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0001"),
                },
                amount: Uint128(1u128),
            },
            short_params: None,
        };
        let env = mock_env_with_block_time(1000);
        let info = mock_info("addr0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "The collateral asset provided is no longer valid")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // withdraw 333334 uusd
        let msg = ExecuteMsg::Withdraw {
            position_idx: Uint128(1u128),
            collateral: Some(Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(333334u128),
            }),
        };

        // only owner can withdraw
        let info = mock_info("addr0001", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let info = mock_info("addr0000", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(
            res.messages,
            vec![CosmosMsg::Bank(BankMsg::Send {
                to_address: "addr0000".to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128(333334u128),
                }]
            })]
        );

        let res = query(
            deps.as_ref(),
            mock_env(),
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
                owner: "addr0000".to_string(),
                collateral: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    amount: Uint128::from(666666u128),
                },
                asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                    amount: Uint128::from(666666u128),
                },
                is_short: false,
            }
        );

        // anyone can burn deprecated asset to any position
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "addr0001".to_string(),
            amount: Uint128::from(666666u128),
            msg: to_binary(&Cw20HookMsg::Burn {
                position_idx: Uint128(1u128),
            })
            .unwrap(),
        });
        let info = mock_info("asset0000", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    send: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Burn {
                        amount: Uint128::from(666666u128), // value 666666 -- protocol fee = 6666
                    })
                    .unwrap(),
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: "collector0000".to_string(),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128(6666u128),
                    }]
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: "addr0001".to_string(),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::from(660000u128), // 666666 - 6666 = 660000
                    }],
                }),
            ]
        );

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "burn"),
                attr("position_idx", "1"),
                attr("burn_amount", "666666asset0000"),
                attr("protocol_fee", "6666uusd"),
                attr("refund_collateral_amount", "660000uusd")
            ]
        );

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Positions {
                owner_addr: None,
                asset_token: Some("asset0000".to_string()),
                start_after: None,
                limit: None,
                order_by: None,
            },
        )
        .unwrap();
        let positions: PositionsResponse = from_binary(&res).unwrap();
        assert_eq!(positions, PositionsResponse { positions: vec![] });
    }

    #[test]
    fn burn_migrated_asset_position() {
        let mut deps = mock_dependencies(&[]);
        deps.querier.with_token_balances(&[(
            &"asset0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
        )]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
            (&"asset0001".to_string(), &Decimal::percent(50)),
        ]);
        deps.querier.with_collateral_infos(&[(
            &"asset0000".to_string(),
            &Decimal::percent(100),
            &Decimal::one(),
            &false,
        )]);

        let base_denom = "uusd".to_string();

        let msg = InstantiateMsg {
            owner: "owner0000".to_string(),
            oracle: "oracle0000".to_string(),
            collector: "collector0000".to_string(),
            collateral_oracle: "collateraloracle0000".to_string(),
            staking: "staking0000".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            lock: "lock0000".to_string(),
            base_denom: base_denom.clone(),
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

        // Open asset0000:asset0001 position
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            msg: to_binary(&Cw20HookMsg::OpenPosition {
                asset_info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0001"),
                },
                collateral_ratio: Decimal::percent(150),
                short_params: None,
            })
            .unwrap(),
            sender: "addr0000".to_string(),
            amount: Uint128(1000000u128),
        });
        let env = mock_env_with_block_time(1000);
        let info = mock_info("asset0000", &[]);
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // register migration
        let msg = ExecuteMsg::RegisterMigration {
            asset_token: "asset0001".to_string(),
            end_price: Decimal::percent(50),
        };
        let info = mock_info("owner0000", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // withdraw 333334 uusd
        let msg = ExecuteMsg::Withdraw {
            position_idx: Uint128(1u128),
            collateral: Some(Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128(333334u128),
            }),
        };

        // only owner can withdraw
        let info = mock_info("addr0001", &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let env = mock_env_with_block_time(1000);
        let info = mock_info("addr0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(
            res.messages,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0000".to_string(),
                send: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0000".to_string(),
                    amount: Uint128::from(333334u128),
                })
                .unwrap(),
            })]
        );

        let res = query(
            deps.as_ref(),
            mock_env(),
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
                owner: "addr0000".to_string(),
                collateral: Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0000"),
                    },
                    amount: Uint128::from(666666u128),
                },
                asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("asset0001"),
                    },
                    amount: Uint128::from(1333333u128),
                },
                is_short: false,
            }
        );

        // anyone can burn deprecated asset to any position
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "addr0001".to_string(),
            amount: Uint128::from(1333333u128),
            msg: to_binary(&Cw20HookMsg::Burn {
                position_idx: Uint128(1u128),
            })
            .unwrap(),
        });
        let env = mock_env_with_block_time(1000);
        let info = mock_info("asset0001", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0001".to_string(),
                    send: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Burn {
                        amount: Uint128::from(1333333u128), // asset value in collateral 1333333 *
                    }) // 0.5 = 666666 -- protocol fee 6666
                    .unwrap(),
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    send: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "collector0000".to_string(),
                        amount: Uint128::from(6666u128),
                    })
                    .unwrap(),
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    send: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "addr0001".to_string(),
                        amount: Uint128::from(659999u128), // rounding
                    })
                    .unwrap(),
                }),
            ]
        );

        assert_eq!(
            res.attributes,
            vec![
                attr("action", "burn"),
                attr("position_idx", "1"),
                attr("burn_amount", "1333333asset0001"),
                attr("protocol_fee", "6666asset0000"),
                attr("refund_collateral_amount", "659999asset0000") // rounding
            ]
        );

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Positions {
                owner_addr: None,
                asset_token: Some("asset0000".to_string()),
                start_after: None,
                limit: None,
                order_by: None,
            },
        )
        .unwrap();
        let positions: PositionsResponse = from_binary(&res).unwrap();
        assert_eq!(positions, PositionsResponse { positions: vec![] });
    }

    #[test]
    fn revoked_collateral() {
        let mut deps = mock_dependencies(&[]);
        deps.querier.with_token_balances(&[(
            &"asset000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
        )]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (&"asset0000".to_string(), &Decimal::percent(100)),
        ]);
        deps.querier.with_collateral_infos(&[(
            &"uluna".to_string(),
            &Decimal::percent(10),
            &Decimal::percent(50),
            &false,
        )]);

        let base_denom = "uusd".to_string();

        let msg = InstantiateMsg {
            owner: "owner0000".to_string(),
            oracle: "oracle0000".to_string(),
            collector: "collector0000".to_string(),
            collateral_oracle: "collateraloracle0000".to_string(),
            staking: "staking0000".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            lock: "lock0000".to_string(),
            base_denom: base_denom.clone(),
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

        // Open uluna:asset0000 position
        let msg = ExecuteMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                amount: Uint128(1000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: Addr::unchecked("asset0000"),
            },
            collateral_ratio: Decimal::percent(200),
            short_params: None,
        };
        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                amount: Uint128(1000u128),
                denom: "uluna".to_string(),
            }],
        );
        let _res = execute(deps.as_mut(), env, info, msg.clone()).unwrap();

        // collateral is revoked
        deps.querier.with_collateral_infos(&[(
            &"uluna".to_string(),
            &Decimal::percent(100),
            &Decimal::percent(50),
            &true,
        )]);

        // open position fails
        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                amount: Uint128(1000u128),
                denom: "uluna".to_string(),
            }],
        );
        let res = execute(deps.as_mut(), env, info, msg.clone()).unwrap_err();
        assert_eq!(
            res,
            StdError::generic_err("The collateral asset provided is no longer valid")
        );

        // minting to previously open position fails
        let msg = ExecuteMsg::Mint {
            position_idx: Uint128(1u128),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: Addr::unchecked("asset0000"),
                },
                amount: Uint128(1u128),
            },
            short_params: None,
        };
        let env = mock_env_with_block_time(1000);
        let info = mock_info("addr0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg.clone()).unwrap_err();
        assert_eq!(
            res,
            StdError::generic_err("The collateral asset provided is no longer valid")
        );

        // deposit fails
        let msg = ExecuteMsg::Deposit {
            position_idx: Uint128(1u128),
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                amount: Uint128(2u128),
            },
        };
        let env = mock_env_with_block_time(1000);
        let info = mock_info(
            "addr0000",
            &[Coin {
                amount: Uint128(2u128),
                denom: "uluna".to_string(),
            }],
        );
        let res = execute(deps.as_mut(), env, info, msg.clone()).unwrap_err();
        assert_eq!(
            res,
            StdError::generic_err("The collateral asset provided is no longer valid")
        );

        // burn against revoked collateral is enabled
        // fail attempt, only owner can burn
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "addr0001".to_string(),
            amount: Uint128::from(10u128),
            msg: to_binary(&Cw20HookMsg::Burn {
                position_idx: Uint128(1u128),
            })
            .unwrap(),
        });
        let env = mock_env_with_block_time(1000);
        let info = mock_info("asset0000", &[]);
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, StdError::generic_err("unauthorized"));
        // sucessful attempt
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "addr0000".to_string(),
            amount: Uint128::from(10u128),
            msg: to_binary(&Cw20HookMsg::Burn {
                position_idx: Uint128(1u128),
            })
            .unwrap(),
        });
        let env = mock_env_with_block_time(1000);
        let info = mock_info("asset0000", &[]);
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // owner can withdraw revoked collateral
        let msg = ExecuteMsg::Withdraw {
            position_idx: Uint128(1u128),
            collateral: Some(Asset {
                info: AssetInfo::NativeToken {
                    denom: "uluna".to_string(),
                },
                amount: Uint128(2u128),
            }),
        };
        let env = mock_env_with_block_time(1000);
        let info = mock_info("addr0000", &[]);
        let _res = execute(deps.as_mut(), env, info, msg.clone()).unwrap();
    }
}
