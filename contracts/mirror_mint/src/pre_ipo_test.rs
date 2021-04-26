#[cfg(test)]
mod tests {
    use crate::contract::{handle, init, query};
    use crate::mock_querier::mock_dependencies;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{
        from_binary, log, to_binary, BlockInfo, Coin, Decimal, Env, HumanAddr, StdError, Uint128,
    };
    use cw20::Cw20ReceiveMsg;
    use mirror_protocol::mint::{AssetConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, QueryMsg};
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

        let base_denom = "uusd".to_string();

        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            staking: HumanAddr::from("staking0000"),
            collateral_oracle: HumanAddr::from("collateraloracle0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: base_denom.clone(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };
        let creator_env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, creator_env.clone(), msg).unwrap();

        // register preIPO asset with mint_end parameter (10 blocks)
        let mint_end = creator_env.clone().block.height + 10u64;
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("preIPOAsset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(1000),
            mint_end: Some(mint_end),
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        ///////////////////
        // Minting phase
        ///////////////////
        let mut current_height = creator_env.block.height + 1;

        // open position successfully at creation_height + 1
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
        let mut env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(1000000000u128),
            }],
            1000u64,
        );
        env.block.height = current_height;
        let _res = handle(&mut deps, env.clone(), msg).unwrap();

        // mint successfully at creation_height + 1
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

        // burn successfully at creation_height + 1
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
        let mut env = mock_env("preIPOAsset0000", &[]);
        env.block.height = current_height;
        let _res = handle(&mut deps, env, msg).unwrap();

        ///////////////////
        // Trading phase
        ///////////////////
        current_height = creator_env.block.height + 11; // > mint_end

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
        let mut env = mock_env_with_block_time(
            "addr0000",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128(1000000000u128),
            }],
            1000u64,
        );
        env.block.height = current_height;
        let res = handle(&mut deps, env.clone(), msg).unwrap_err();
        assert_eq!(
            res,
            StdError::generic_err(format!(
                "The minting period for this asset ended at height {}",
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
                "The minting period for this asset ended at height {}",
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
        let mut env = mock_env("preIPOAsset0000", &[]);
        env.block.height = current_height;
        let res = handle(&mut deps, env, msg).unwrap_err();
        assert_eq!(
            res,
            StdError::generic_err(format!(
            "Burning is disabled for assets with limitied minting time. Mint period ended at {}",
            mint_end
        ))
        );

        ///////////////////
        // IPO/Migration
        ///////////////////
        current_height = creator_env.block.height + 20;

        // register migration initiated by the feeder
        let msg = HandleMsg::RegisterMigration {
            asset_token: HumanAddr::from("preIPOAsset0000"),
            end_price: Decimal::percent(50), // first IPO price
        };
        let mut env = mock_env("owner0000", &[]);
        env.block.height = current_height;
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(
            res.log,
            vec![
                log("action", "migrate_asset"),
                log("asset_token", "preIPOAsset0000"),
                log("end_price", "0.5"),
            ]
        );

        let res = query(
            &deps,
            QueryMsg::AssetConfig {
                asset_token: HumanAddr::from("preIPOAsset0000"),
            },
        )
        .unwrap();
        let asset_config_res: AssetConfigResponse = from_binary(&res).unwrap();
        assert_eq!(
            asset_config_res,
            AssetConfigResponse {
                token: HumanAddr::from("preIPOAsset0000"),
                auction_discount: Decimal::percent(20),
                min_collateral_ratio: Decimal::percent(100),
                end_price: Some(Decimal::percent(50)),
                mint_end: None,
                min_collateral_ratio_after_migration: None,
            }
        );

        // anyone can burn the preIPO asset at the first IPO price
        let msg = HandleMsg::Receive(Cw20ReceiveMsg {
            sender: HumanAddr::from("addr0001"),
            amount: Uint128::from(133u128),
            msg: Some(
                to_binary(&Cw20HookMsg::Burn {
                    position_idx: Uint128(1u128),
                })
                .unwrap(),
            ),
        });
        let mut env = mock_env_with_block_time("preIPOAsset0000", &[], 1000);
        env.block.height = current_height + 1;
        let _res = handle(&mut deps, env, msg).unwrap();
    }
}
