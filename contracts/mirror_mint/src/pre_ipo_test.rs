#[cfg(test)]
mod tests {
    use crate::contract::{handle, init, query};
    use crate::mock_querier::mock_dependencies;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{
        from_binary, log, to_binary, BlockInfo, Coin, CosmosMsg, Decimal, Env, HumanAddr, StdError,
        Uint128, WasmMsg, WasmQuery,
    };
    use cw20::Cw20ReceiveMsg;
    use mirror_protocol::collateral_oracle::{HandleMsg::RegisterCollateralAsset, SourceType};
    use mirror_protocol::mint::{
        AssetConfigResponse, Cw20HookMsg, HandleMsg, IPOParams, InitMsg, QueryMsg,
    };
    use mirror_protocol::oracle::QueryMsg::Price;
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
    fn pre_ipo_assets() {
        let mut deps = mock_dependencies(20, &[]);
        deps.querier.with_oracle_price(&[
            (&"uusd".to_string(), &Decimal::one()),
            (
                &"preIPOAsset0000".to_string(),
                &Decimal::from_ratio(10u128, 1u128),
            ),
        ]);
        deps.querier.with_oracle_feeders(&[(
            &HumanAddr::from("preIPOAsset0000"),
            &HumanAddr::from("feeder0000"),
        )]);

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
        let creator_env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, creator_env.clone(), msg).unwrap();

        // register preIPO asset with mint_end parameter (10 blocks)
        let mint_end = creator_env.clone().block.time + 10u64;
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("preIPOAsset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(1000),
            ipo_params: Some(IPOParams {
                mint_end,
                min_collateral_ratio_after_ipo: Decimal::percent(150),
                pre_ipo_price: Decimal::percent(100),
            }),
        };
        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(res.messages.len(), 0); // not registered as collateral

        ///////////////////
        // Minting phase
        ///////////////////
        let mut current_time = creator_env.block.time + 1;

        // open position successfully at creation_time + 1
        let msg = HandleMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(2000000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("preIPOAsset0000"),
            },
            collateral_ratio: Decimal::percent(2000),
            short_params: None,
        };
        let env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(2000000000u128),
            }],
            current_time,
        );
        let res = handle(&mut deps, env.clone(), msg).unwrap();
        assert_eq!(
            res.log,
            vec![
                log("action", "open_position"),
                log("position_idx", "1"),
                log("mint_amount", "100000000preIPOAsset0000"), // 2000% cr with pre_ipo_price=1
                log("collateral_amount", "2000000000uusd"),
                log("is_short", "false"),
            ]
        );

        // mint successfully at creation_time + 1
        let msg = HandleMsg::Mint {
            position_idx: Uint128(1u128),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("preIPOAsset0000"),
                },
                amount: Uint128(2000000u128),
            },
            short_params: None,
        };
        let _res = handle(&mut deps, env.clone(), msg).unwrap();

        // burn successfully at creation_time + 1
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr0000"),
            amount: Uint128::from(1000000u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Burn {
                    position_idx: Uint128(1u128),
                })
                .unwrap(),
            ),
        });
        let env = mock_env_with_block_time("preIPOAsset0000", &[], current_time);
        let _res = handle(&mut deps, env, msg).unwrap();

        ///////////////////
        // Trading phase
        ///////////////////
        current_time = creator_env.block.time + 11; // > mint_end

        // open position disabled
        let msg = HandleMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(1000000000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("preIPOAsset0000"),
            },
            collateral_ratio: Decimal::percent(10000),
            short_params: None,
        };
        let env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(1000000000u128),
            }],
            current_time,
        );
        let res = handle(&mut deps, env.clone(), msg).unwrap_err();
        assert_eq!(
            res,
            StdError::generic_err(format!(
                "The minting period for this asset ended at time {}",
                mint_end
            ))
        );

        // mint disabled
        let msg = HandleMsg::Mint {
            position_idx: Uint128(1u128),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("preIPOAsset0000"),
                },
                amount: Uint128(2000000u128),
            },
            short_params: None,
        };
        let res = handle(&mut deps, env.clone(), msg).unwrap_err();
        assert_eq!(
            res,
            StdError::generic_err(format!(
                "The minting period for this asset ended at time {}",
                mint_end
            ))
        );

        // burn disabled
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr0000"),
            amount: Uint128::from(1000000u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Burn {
                    position_idx: Uint128(1u128),
                })
                .unwrap(),
            ),
        });
        let env = mock_env_with_block_time("preIPOAsset0000", &[], current_time);
        let res = handle(&mut deps, env, msg).unwrap_err();
        assert_eq!(
            res,
            StdError::generic_err(format!(
            "Burning is disabled for assets with limitied minting time. Mint period ended at time {}",
            mint_end
        ))
        );

        ///////////////////
        // IPO/Migration
        ///////////////////
        current_time = creator_env.block.time + 20;

        // register migration initiated by the feeder
        let msg = HandleMsg::TriggerIPO {
            asset_token: HumanAddr::from("preIPOAsset0000"),
        };

        // unauthorized attempt
        let env = mock_env("owner", &[]);
        let res = handle(&mut deps, env, msg.clone()).unwrap_err();
        assert_eq!(res, StdError::unauthorized());

        // succesfull attempt
        let env = mock_env_with_block_time("feeder0000", &[], current_time);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            res.log,
            vec![
                log("action", "trigger_ipo"),
                log("asset_token", "preIPOAsset0000"),
            ]
        );
        assert_eq!(
            res.messages,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("collateraloracle0000"),
                send: vec![],
                msg: to_binary(&RegisterCollateralAsset {
                    asset: AssetInfo::Token {
                        contract_addr: HumanAddr::from("preIPOAsset0000"),
                    },
                    multiplier: Decimal::one(),
                    price_source: SourceType::TerraOracle {
                        terra_oracle_query: to_binary(&WasmQuery::Smart {
                            contract_addr: HumanAddr::from("oracle0000"),
                            msg: to_binary(&Price {
                                base_asset: "uusd".to_string(),
                                quote_asset: "preIPOAsset0000".to_string(),
                            })
                            .unwrap()
                        })
                        .unwrap(),
                    },
                })
                .unwrap(),
            })]
        );

        let res = query(
            &deps,
            QueryMsg::AssetConfig {
                asset_token: HumanAddr::from("preIPOAsset0000"),
            },
        )
        .unwrap();
        let asset_config_res: AssetConfigResponse = from_binary(&res).unwrap();
        // traditional asset configuration, feeder feeds real price
        assert_eq!(
            asset_config_res,
            AssetConfigResponse {
                token: HumanAddr::from("preIPOAsset0000"),
                auction_discount: Decimal::percent(20),
                min_collateral_ratio: Decimal::percent(150),
                end_price: None,
                ipo_params: None,
            }
        );

        // open position as a postIPO asset
        let msg = HandleMsg::OpenPosition {
            collateral: Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128(9000u128),
            },
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("preIPOAsset0000"),
            },
            collateral_ratio: Decimal::percent(150), // new minCR
            short_params: None,
        };
        let env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(9000u128),
            }],
            1000u64,
        );
        let res = handle(&mut deps, env.clone(), msg).unwrap();
        assert_eq!(
            res.log,
            vec![
                log("action", "open_position"),
                log("position_idx", "2"),
                log("mint_amount", "599preIPOAsset0000"), // 150% cr with oracle_price=10
                log("collateral_amount", "9000uusd"),
                log("is_short", "false"),
            ]
        );
    }
}
