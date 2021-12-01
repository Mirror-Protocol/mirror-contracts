use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{
    attr, from_binary, to_binary, BlockInfo, Coin, CosmosMsg, Decimal, Env, StdError, SubMsg,
    Timestamp, Uint128, WasmMsg,
};
use cw20::Cw20ReceiveMsg;
use mirror_protocol::collateral_oracle::{ExecuteMsg::RegisterCollateralAsset, SourceType};
use mirror_protocol::mint::{
    AssetConfigResponse, Cw20HookMsg, ExecuteMsg, IPOParams, InstantiateMsg, QueryMsg,
};
use terraswap::asset::{Asset, AssetInfo};

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
fn pre_ipo_assets() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (
            &"preIPOAsset0000".to_string(),
            &Decimal::from_ratio(10u128, 1u128),
        ),
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
    let creator_env = mock_env();
    let creator_info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), creator_env.clone(), creator_info, msg).unwrap();

    // register preIPO asset with mint_end parameter (10 blocks)
    let mint_end = creator_env.block.time.plus_seconds(10u64).seconds();
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "preIPOAsset0000".to_string(),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(1000),
        ipo_params: Some(IPOParams {
            mint_end,
            min_collateral_ratio_after_ipo: Decimal::percent(150),
            pre_ipo_price: Decimal::percent(100),
            trigger_addr: "ipotrigger0000".to_string(),
        }),
    };

    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(res.messages.len(), 0); // not registered as collateral

    ///////////////////
    // Minting phase
    ///////////////////
    let mut current_time = creator_env.block.time.plus_seconds(1).seconds();

    // open position successfully at creation_height + 1
    let msg = ExecuteMsg::OpenPosition {
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(2000000000u128),
        },
        asset_info: AssetInfo::Token {
            contract_addr: "preIPOAsset0000".to_string(),
        },
        collateral_ratio: Decimal::percent(2000),
        short_params: None,
    };

    let env = mock_env_with_block_time(current_time);
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(2000000000u128),
        }],
    );
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "open_position"),
            attr("position_idx", "1"),
            attr("mint_amount", "100000000preIPOAsset0000"), // 2000% cr with pre_ipo_price=1
            attr("collateral_amount", "2000000000uusd"),
            attr("is_short", "false"),
        ]
    );

    // mint successfully at creation_time + 1
    let msg = ExecuteMsg::Mint {
        position_idx: Uint128::from(1u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: "preIPOAsset0000".to_string(),
            },
            amount: Uint128::from(2000000u128),
        },
        short_params: None,
    };
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // burn successfully at creation_time + 1
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Burn {
            position_idx: Uint128::from(1u128),
        })
        .unwrap(),
    });

    let env = mock_env_with_block_time(current_time);
    let info = mock_info("preIPOAsset0000", &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    ///////////////////
    // Trading phase
    ///////////////////
    current_time = creator_env.block.time.plus_seconds(11).seconds(); // > mint_end

    // open position disabled
    let msg = ExecuteMsg::OpenPosition {
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(1000000000u128),
        },
        asset_info: AssetInfo::Token {
            contract_addr: "preIPOAsset0000".to_string(),
        },
        collateral_ratio: Decimal::percent(10000),
        short_params: None,
    };

    let env = mock_env_with_block_time(current_time);
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000000u128),
        }],
    );

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err(format!(
            "The minting period for this asset ended at time {}",
            mint_end
        ))
    );

    // mint disabled
    let msg = ExecuteMsg::Mint {
        position_idx: Uint128::from(1u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: "preIPOAsset0000".to_string(),
            },
            amount: Uint128::from(2000000u128),
        },
        short_params: None,
    };
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err(format!(
            "The minting period for this asset ended at time {}",
            mint_end
        ))
    );

    // burn disabled
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Burn {
            position_idx: Uint128::from(1u128),
        })
        .unwrap(),
    });

    let env = mock_env_with_block_time(current_time);
    let info = mock_info("preIPOAsset0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
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
    current_time = creator_env.block.time.plus_seconds(20).seconds();

    // register migration initiated by the trigger address
    let msg = ExecuteMsg::TriggerIPO {
        asset_token: "preIPOAsset0000".to_string(),
    };

    // unauthorized attempt
    let info = mock_info("owner", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));

    // succesfull attempt
    let env = mock_env_with_block_time(current_time);
    let info = mock_info("ipotrigger0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "trigger_ipo"),
            attr("asset_token", "preIPOAsset0000"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "collateraloracle0000".to_string(),
            funds: vec![],
            msg: to_binary(&RegisterCollateralAsset {
                asset: AssetInfo::Token {
                    contract_addr: "preIPOAsset0000".to_string(),
                },
                multiplier: Decimal::one(),
                price_source: SourceType::TeFiOracle {
                    oracle_addr: "oracle0000".to_string(),
                },
            })
            .unwrap(),
        }))]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::AssetConfig {
            asset_token: "preIPOAsset0000".to_string(),
        },
    )
    .unwrap();
    let asset_config_res: AssetConfigResponse = from_binary(&res).unwrap();
    // traditional asset configuration, price is obtained from the oracle
    assert_eq!(
        asset_config_res,
        AssetConfigResponse {
            token: "preIPOAsset0000".to_string(),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            end_price: None,
            ipo_params: None,
        }
    );

    // open position as a postIPO asset
    let msg = ExecuteMsg::OpenPosition {
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(9000u128),
        },
        asset_info: AssetInfo::Token {
            contract_addr: "preIPOAsset0000".to_string(),
        },
        collateral_ratio: Decimal::percent(150), // new minCR
        short_params: None,
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(9000u128),
        }],
    );
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "open_position"),
            attr("position_idx", "2"),
            attr("mint_amount", "599preIPOAsset0000"), // 150% cr with oracle_price=10
            attr("collateral_amount", "9000uusd"),
            attr("is_short", "false"),
        ]
    );
}
