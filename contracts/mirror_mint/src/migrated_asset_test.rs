#[cfg(test)]
mod tests {
    use crate::contract::{handle, init, query};
    use crate::mock_querier::mock_dependencies;
    use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{
        from_binary, log, to_binary, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, HumanAddr,
        StdError, Uint128, WasmMsg,
    };
    use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
    use mirror_protocol::mint::{
        AssetConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, PositionResponse, PositionsResponse,
        QueryMsg,
    };
    use terraswap::asset::{Asset, AssetInfo};

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
    fn register_migration() {
        let mut deps = mock_dependencies(20, &[]);
        deps.querier.with_token_balances(&[(
            &HumanAddr::from("asset0000"),
            &[(
                &HumanAddr::from(MOCK_CONTRACT_ADDR),
                &Uint128::from(1000000u128),
            )],
        )]);

        let base_denom = "uusd".to_string();

        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            staking: HumanAddr::from("staking0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
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
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };

        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterMigration {
            asset_token: HumanAddr::from("asset0000"),
            end_price: Decimal::from_ratio(2u128, 1u128),
        };
        let env = mock_env("addr0000", &[]);
        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = HandleMsg::RegisterMigration {
            asset_token: HumanAddr::from("asset0001"),
            end_price: Decimal::from_ratio(2u128, 1u128),
        };
        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "no asset data stored"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = HandleMsg::RegisterMigration {
            asset_token: HumanAddr::from("asset0000"),
            end_price: Decimal::percent(50),
        };
        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            res.log,
            vec![
                log("action", "migrate_asset"),
                log("asset_token", "asset0000"),
                log("end_price", "0.5"),
            ]
        );

        let res = query(
            &deps,
            QueryMsg::AssetConfig {
                asset_token: HumanAddr::from("asset0000"),
            },
        )
        .unwrap();
        let asset_config_res: AssetConfigResponse = from_binary(&res).unwrap();
        assert_eq!(
            asset_config_res,
            AssetConfigResponse {
                token: HumanAddr::from("asset0000"),
                auction_discount: Decimal::percent(20),
                min_collateral_ratio: Decimal::percent(100),
                end_price: Some(Decimal::percent(50)),
                mint_end: None,
                min_collateral_ratio_after_migration: None,
            }
        );
    }
    #[test]
    fn migrated_asset() {
        let mut deps = mock_dependencies(20, &[]);
        deps.querier.with_token_balances(&[(
            &HumanAddr::from("asset0000"),
            &[(
                &HumanAddr::from(MOCK_CONTRACT_ADDR),
                &Uint128::from(1000000u128),
            )],
        )]);

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
            terraswap_factory: HumanAddr::from("terraswap_factory"),
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
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0001"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // Open uusd:asset0000 position
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
            short_params: None,
        };
        let env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                amount: Uint128(1000000u128),
                denom: "uusd".to_string(),
            }],
            1000,
        );
        let _res = handle(&mut deps, env, msg).unwrap();

        // Open asset0000:asset0001 position
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            msg: Some(
                to_binary(&Cw20HookMsg::OpenPosition {
                    asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001"),
                    },
                    collateral_ratio: Decimal::percent(150),
                    short_params: None,
                })
                .unwrap(),
            ),
            sender: HumanAddr::from("addr0000"),
            amount: Uint128(1000000u128),
        });
        let env = mock_env_with_block_time("asset0000", &[], 1000);
        let _res = handle(&mut deps, env, msg).unwrap();

        // register migration
        let msg = HandleMsg::RegisterMigration {
            asset_token: HumanAddr::from("asset0000"),
            end_price: Decimal::percent(100),
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // cannot open a position with deprecated collateral
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            msg: Some(
                to_binary(&Cw20HookMsg::OpenPosition {
                    asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001"),
                    },
                    collateral_ratio: Decimal::percent(150),
                    short_params: None,
                })
                .unwrap(),
            ),
            sender: HumanAddr::from("addr0000"),
            amount: Uint128(1000000u128),
        });
        let env = mock_env_with_block_time("asset0000", &[], 1000);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Operation is not allowed for the deprecated asset")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot open a deprecated asset position
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            msg: Some(
                to_binary(&Cw20HookMsg::OpenPosition {
                    asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0000"),
                    },
                    collateral_ratio: Decimal::percent(150),
                    short_params: None,
                })
                .unwrap(),
            ),
            sender: HumanAddr::from("addr0001"),
            amount: Uint128(1000000u128),
        });
        let env = mock_env_with_block_time("asset0000", &[], 1000);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Operation is not allowed for the deprecated asset")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot do deposit more collateral to the deprecated asset position
        let msg = HandleMsg::Deposit {
            position_idx: Uint128(1u128),
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(1u128),
            },
        };
        let env = mock_env(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(1u128),
            }],
        );
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Operation is not allowed for the deprecated asset")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot do deposit more collateral to the deprecated asset position
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            msg: Some(
                to_binary(&Cw20HookMsg::Deposit {
                    position_idx: Uint128(2u128),
                })
                .unwrap(),
            ),
            sender: HumanAddr::from("addr0000"),
            amount: Uint128(1000000u128),
        });
        let env = mock_env("asset0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Operation is not allowed for the deprecated asset")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot mint more the deprecated asset
        let msg = HandleMsg::Mint {
            position_idx: Uint128(1u128),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                amount: Uint128(1u128),
            },
            short_params: None,
        };
        let env = mock_env("addr0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Operation is not allowed for the deprecated asset")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // cannot mint more the asset with deprecated position
        let msg = HandleMsg::Mint {
            position_idx: Uint128(2u128),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0001"),
                },
                amount: Uint128(1u128),
            },
            short_params: None,
        };
        let env = mock_env("addr0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "Operation is not allowed for the deprecated asset")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }

        // withdraw 333334 uusd
        let msg = HandleMsg::Withdraw {
            position_idx: Uint128(1u128),
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(333334u128),
            },
        };

        // only owner can withdraw
        let env = mock_env("addr0001", &[]);
        let res = handle(&mut deps, env, msg.clone()).unwrap_err();
        match res {
            StdError::Unauthorized { .. } => {}
            _ => panic!("DO NOT ENTER HERE"),
        }

        let env = mock_env("addr0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Bank(BankMsg::Send {
                    from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    to_address: HumanAddr::from("addr0000"),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128(330001u128),
                    }]
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    to_address: HumanAddr::from("collector0000"),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128(3333u128),
                    }]
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
                collateral: Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    amount: Uint128::from(666666u128),
                },
                asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0000"),
                    },
                    amount: Uint128::from(666666u128),
                },
                is_short: false,
            }
        );

        // anyone can burn deprecated asset to any position
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr0001"),
            amount: Uint128::from(666666u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Burn {
                    position_idx: Uint128(1u128),
                })
                .unwrap(),
            ),
        });
        let env = mock_env("asset0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Burn {
                        amount: Uint128::from(666666u128),
                    })
                    .unwrap(),
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    to_address: HumanAddr::from("addr0001"),
                    amount: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::from(666666u128),
                    }],
                }),
            ]
        );

        assert_eq!(
            res.log,
            vec![
                log("action", "burn"),
                log("position_idx", "1"),
                log("burn_amount", "666666asset0000"),
                log("refund_collateral_amount", "666666uusd")
            ]
        );

        let res = query(
            &deps,
            QueryMsg::Positions {
                owner_addr: None,
                asset_token: Some(HumanAddr::from("asset0000")),
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
        let mut deps = mock_dependencies(20, &[]);
        deps.querier.with_token_balances(&[(
            &HumanAddr::from("asset0000"),
            &[(
                &HumanAddr::from(MOCK_CONTRACT_ADDR),
                &Uint128::from(1000000u128),
            )],
        )]);

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
            terraswap_factory: HumanAddr::from("terraswap_factory"),
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
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0001"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // Open asset0000:asset0001 position
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            msg: Some(
                to_binary(&Cw20HookMsg::OpenPosition {
                    asset_info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001"),
                    },
                    collateral_ratio: Decimal::percent(150),
                    short_params: None,
                })
                .unwrap(),
            ),
            sender: HumanAddr::from("addr0000"),
            amount: Uint128(1000000u128),
        });
        let env = mock_env_with_block_time("asset0000", &[], 1000);
        let _res = handle(&mut deps, env, msg).unwrap();

        // register migration
        let msg = HandleMsg::RegisterMigration {
            asset_token: HumanAddr::from("asset0001"),
            end_price: Decimal::percent(50),
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        // withdraw 333334 uusd
        let msg = HandleMsg::Withdraw {
            position_idx: Uint128(1u128),
            collateral: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                amount: Uint128(333334u128),
            },
        };

        // only owner can withdraw
        let env = mock_env("addr0001", &[]);
        let res = handle(&mut deps, env, msg.clone()).unwrap_err();
        match res {
            StdError::Unauthorized { .. } => {}
            _ => panic!("DO NOT ENTER HERE"),
        }

        let env = mock_env_with_block_time("addr0000", &[], 1000);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Transfer {
                        recipient: HumanAddr::from("addr0000"),
                        amount: Uint128::from(330001u128),
                    })
                    .unwrap(),
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Transfer {
                        recipient: HumanAddr::from("collector0000"),
                        amount: Uint128::from(3333u128),
                    })
                    .unwrap(),
                }),
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
                collateral: Asset {
                    info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0000"),
                    },
                    amount: Uint128::from(666666u128),
                },
                asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001"),
                    },
                    amount: Uint128::from(1333333u128),
                },
                is_short: false,
            }
        );

        // anyone can burn deprecated asset to any position
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr0001"),
            amount: Uint128::from(1333333u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Burn {
                    position_idx: Uint128(1u128),
                })
                .unwrap(),
            ),
        });
        let env = mock_env_with_block_time("asset0001", &[], 1000);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            res.messages,
            vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0001"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Burn {
                        amount: Uint128::from(1333333u128),
                    })
                    .unwrap(),
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from("asset0000"),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::Transfer {
                        recipient: HumanAddr::from("addr0001"),
                        amount: Uint128::from(666665u128), // rounding
                    })
                    .unwrap(),
                }),
            ]
        );

        assert_eq!(
            res.log,
            vec![
                log("action", "burn"),
                log("position_idx", "1"),
                log("burn_amount", "1333333asset0001"),
                log("refund_collateral_amount", "666665asset0000") // rounding
            ]
        );

        let res = query(
            &deps,
            QueryMsg::Positions {
                owner_addr: None,
                asset_token: Some(HumanAddr::from("asset0000")),
                start_after: None,
                limit: None,
                order_by: None,
            },
        )
        .unwrap();
        let positions: PositionsResponse = from_binary(&res).unwrap();
        assert_eq!(positions, PositionsResponse { positions: vec![] });
    }
}
