use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{
    attr, from_binary, to_binary, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, StdError,
    SubMsg, Timestamp, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use mirror_protocol::common::OrderBy;
use mirror_protocol::mint::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, PositionResponse, PositionsResponse, QueryMsg,
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
fn open_position() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (&"asset0000".to_string(), &Decimal::percent(100)),
        (&"asset0001".to_string(), &Decimal::percent(50)),
    ]);
    deps.querier.with_collateral_infos(&[(
        &"asset0001".to_string(),
        &Decimal::percent(50),
        &Decimal::percent(200), // 2 collateral_multiplier
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

    // open position with unknown collateral
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        msg: to_binary(&Cw20HookMsg::OpenPosition {
            asset_info: AssetInfo::Token {
                contract_addr: "asset9999".to_string(),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: None,
        })
        .unwrap(),
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset9999", &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap_err(); // expect error

    // must fail; collateral ratio is too low
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
        collateral_ratio: Decimal::percent(140),
        short_params: None,
    };
    let env = mock_env_with_block_time(1000);
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(
            msg,
            "Can not open a position with low collateral ratio than minimum"
        ),
        _ => panic!("DO NOT ENTER ERROR"),
    }

    // successful attempt
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
        short_params: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "open_position"),
            attr("position_idx", "1"),
            attr("mint_amount", "666666asset0000"),
            attr("collateral_amount", "1000000uusd"),
            attr("is_short", "false"),
        ]
    );

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "asset0000".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(666666u128),
            })
            .unwrap(),
        }))]
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
            is_short: false,
        }
    );

    // can query positions
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Positions {
            owner_addr: Some("addr0000".to_string()),
            asset_token: None,
            limit: None,
            start_after: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let positions: PositionsResponse = from_binary(&res).unwrap();
    assert_eq!(
        positions,
        PositionsResponse {
            positions: vec![PositionResponse {
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
                is_short: false,
            }],
        }
    );

    // Cannot directly deposit token
    let msg = ExecuteMsg::OpenPosition {
        collateral: Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0001".to_string(),
            },
            amount: Uint128::from(1000000u128),
        },
        asset_info: AssetInfo::Token {
            contract_addr: "asset0000".to_string(),
        },
        collateral_ratio: Decimal::percent(150),
        short_params: None,
    };
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        msg: to_binary(&Cw20HookMsg::OpenPosition {
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(300), // 15 * 2 (multiplier)
            short_params: None,
        })
        .unwrap(),
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset0001", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "open_position"),
            attr("position_idx", "2"),
            attr("mint_amount", "166666asset0000"), // 1000000 * 0.5 (price to asset) * 0.5 multiplier / 1.5 (mcr)
            attr("collateral_amount", "1000000asset0001"),
            attr("is_short", "false"),
        ]
    );

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "asset0000".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(166666u128),
            })
            .unwrap(),
        }))]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Position {
            position_idx: Uint128::from(2u128),
        },
    )
    .unwrap();
    let position: PositionResponse = from_binary(&res).unwrap();
    assert_eq!(
        position,
        PositionResponse {
            idx: Uint128::from(2u128),
            owner: "addr0000".to_string(),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: "asset0000".to_string(),
                },
                amount: Uint128::from(166666u128),
            },
            collateral: Asset {
                info: AssetInfo::Token {
                    contract_addr: "asset0001".to_string(),
                },
                amount: Uint128::from(1000000u128),
            },
            is_short: false,
        }
    );

    // can query positions
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Positions {
            owner_addr: Some("addr0000".to_string()),
            asset_token: None,
            limit: None,
            start_after: None,
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let positions: PositionsResponse = from_binary(&res).unwrap();
    assert_eq!(
        positions,
        PositionsResponse {
            positions: vec![
                PositionResponse {
                    idx: Uint128::from(2u128),
                    owner: "addr0000".to_string(),
                    asset: Asset {
                        info: AssetInfo::Token {
                            contract_addr: "asset0000".to_string(),
                        },
                        amount: Uint128::from(166666u128),
                    },
                    collateral: Asset {
                        info: AssetInfo::Token {
                            contract_addr: "asset0001".to_string(),
                        },
                        amount: Uint128::from(1000000u128),
                    },
                    is_short: false,
                },
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
                    is_short: false,
                }
            ],
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Positions {
            owner_addr: Some("addr0000".to_string()),
            asset_token: None,
            limit: None,
            start_after: Some(Uint128::from(2u128)),
            order_by: Some(OrderBy::Desc),
        },
    )
    .unwrap();
    let positions: PositionsResponse = from_binary(&res).unwrap();
    assert_eq!(
        positions,
        PositionsResponse {
            positions: vec![PositionResponse {
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
                is_short: false,
            }],
        }
    );
}

#[test]
fn deposit() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (&"asset0000".to_string(), &Decimal::percent(100)),
        (&"asset0001".to_string(), &Decimal::percent(50)),
    ]);
    deps.querier.with_collateral_infos(&[(
        &"asset0001".to_string(),
        &Decimal::percent(50),
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

    // open uusd-asset0000 position
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
        short_params: None,
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

    // open asset0001-asset0000 position
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        msg: to_binary(&Cw20HookMsg::OpenPosition {
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: None,
        })
        .unwrap(),
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset0001", &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::Deposit {
        position_idx: Uint128::from(1u128),
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(1000000u128),
        },
    };
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
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
                amount: Uint128::from(2000000u128),
            },
            is_short: false,
        }
    );

    // unauthorized failed; must be executed from token contract
    let msg = ExecuteMsg::Deposit {
        position_idx: Uint128::from(2u128),
        collateral: Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0001".to_string(),
            },
            amount: Uint128::from(1000000u128),
        },
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }

    // deposit other token asset
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Deposit {
            position_idx: Uint128::from(2u128),
        })
        .unwrap(),
    });

    let info = mock_info("asset0001", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Position {
            position_idx: Uint128::from(2u128),
        },
    )
    .unwrap();

    let position: PositionResponse = from_binary(&res).unwrap();
    assert_eq!(
        position,
        PositionResponse {
            idx: Uint128::from(2u128),
            owner: "addr0000".to_string(),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: "asset0000".to_string(),
                },
                amount: Uint128::from(333333u128),
            },
            collateral: Asset {
                info: AssetInfo::Token {
                    contract_addr: "asset0001".to_string(),
                },
                amount: Uint128::from(2000000u128),
            },
            is_short: false,
        }
    );
}

#[test]
fn mint() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(100u128, 1u128),
        ),
        (
            &"asset0001".to_string(),
            &Decimal::from_ratio(50u128, 1u128),
        ),
    ]);
    deps.querier.with_collateral_infos(&[(
        &"asset0001".to_string(),
        &Decimal::from_ratio(50u128, 1u128),
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

    // open uusd-asset0000 position
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
        short_params: None,
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

    // open asset0001-asset0000 position
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        msg: to_binary(&Cw20HookMsg::OpenPosition {
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: None,
        })
        .unwrap(),
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset0001", &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::Deposit {
        position_idx: Uint128::from(1u128),
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(1000000u128),
        },
    };
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // deposit other token asset
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Deposit {
            position_idx: Uint128::from(2u128),
        })
        .unwrap(),
    });

    let info = mock_info("asset0001", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // failed to mint; due to min_collateral_ratio
    // price 100, collateral 1000000, min_collateral_ratio 150%
    // x * price * min_collateral_ratio < collateral
    // x < collateral/(price*min_collateral_ratio) = 10000 / 1.5
    let msg = ExecuteMsg::Mint {
        position_idx: Uint128::from(1u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            amount: Uint128::from(6668u128),
        },
        short_params: None,
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot mint asset over than min collateral ratio")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // successfully mint within the min_collateral_ratio
    let msg = ExecuteMsg::Mint {
        position_idx: Uint128::from(1u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            amount: Uint128::from(6667u128),
        },
        short_params: None,
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "asset0000".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                amount: Uint128::from(6667u128),
                recipient: "addr0000".to_string(),
            })
            .unwrap(),
        }))]
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "mint"),
            attr("position_idx", "1"),
            attr("mint_amount", "6667asset0000")
        ]
    );

    // mint with other token; failed due to min collateral ratio
    let msg = ExecuteMsg::Mint {
        position_idx: Uint128::from(2u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            amount: Uint128::from(333334u128),
        },
        short_params: None,
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot mint asset over than min collateral ratio")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // mint with other token;
    let msg = ExecuteMsg::Mint {
        position_idx: Uint128::from(2u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            amount: Uint128::from(333333u128),
        },
        short_params: None,
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "asset0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                amount: Uint128::from(333333u128),
                recipient: "addr0000".to_string(),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "mint"),
            attr("position_idx", "2"),
            attr("mint_amount", "333333asset0000")
        ]
    );
}

#[test]
fn burn() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(100u128, 1u128),
        ),
        (
            &"asset0001".to_string(),
            &Decimal::from_ratio(50u128, 1u128),
        ),
    ]);
    deps.querier.with_collateral_infos(&[(
        &"asset0001".to_string(),
        &Decimal::from_ratio(50u128, 1u128),
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

    // open uusd-asset0000 position
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
        short_params: None,
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

    // open asset0001-asset0000 position
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        msg: to_binary(&Cw20HookMsg::OpenPosition {
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: None,
        })
        .unwrap(),
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset0001", &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    let msg = ExecuteMsg::Deposit {
        position_idx: Uint128::from(1u128),
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(1000000u128),
        },
    };
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // deposit other token asset
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::Deposit {
            position_idx: Uint128::from(2u128),
        })
        .unwrap(),
    });

    let info = mock_info("asset0001", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Mint {
        position_idx: Uint128::from(1u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            amount: Uint128::from(6667u128),
        },
        short_params: None,
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // failed to burn more than the position amount
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(13334u128),
        msg: to_binary(&Cw20HookMsg::Burn {
            position_idx: Uint128::from(1u128),
        })
        .unwrap(),
    });
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot burn asset more than you mint")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(13333u128),
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
            attr("burn_amount", "13333asset0000"),
            attr("protocol_fee", "13333uusd") // 13333 * 100 (price) * 0.01 (protocol_fee)
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: Uint128::from(13333u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "collector0000".to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(13333u128)
                }],
            })),
        ]
    );

    // mint other asset
    let msg = ExecuteMsg::Mint {
        position_idx: Uint128::from(2u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            amount: Uint128::from(333333u128),
        },
        short_params: None,
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // failed to burn more than the position amount
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(666667u128),
        msg: to_binary(&Cw20HookMsg::Burn {
            position_idx: Uint128::from(2u128),
        })
        .unwrap(),
    });
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot burn asset more than you mint")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(666666u128),
        msg: to_binary(&Cw20HookMsg::Burn {
            position_idx: Uint128::from(2u128),
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
            attr("position_idx", "2"),
            attr("burn_amount", "666666asset0000"),
            attr("protocol_fee", "13333asset0001"), // 666666 * 100 * 0.01 / 50
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: Uint128::from(666666u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0001".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "collector0000".to_string(),
                    amount: Uint128::from(13333u128)
                })
                .unwrap(),
                funds: vec![],
            }))
        ]
    );
}

#[test]
fn withdraw() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(100u128, 1u128),
        ),
        (
            &"asset0001".to_string(),
            &Decimal::from_ratio(50u128, 1u128),
        ),
    ]);
    deps.querier.with_collateral_infos(&[(
        &"asset0001".to_string(),
        &Decimal::from_ratio(50u128, 1u128),
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

    // open uusd-asset0000 position
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
        short_params: None,
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

    // open asset0001-asset0000 position
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        msg: to_binary(&Cw20HookMsg::OpenPosition {
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: None,
        })
        .unwrap(),
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset0001", &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    // cannot withdraw more than (100 collateral token == 1 token)
    // due to min collateral ratio
    let msg = ExecuteMsg::Withdraw {
        position_idx: Uint128::from(1u128),
        collateral: Some(Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(101u128),
        }),
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(
            msg,
            "Cannot withdraw collateral over than minimum collateral ratio"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::Withdraw {
        position_idx: Uint128::from(1u128),
        collateral: Some(Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(100u128),
        }),
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw"),
            attr("position_idx", "1"),
            attr("withdraw_amount", "100uusd"),
            attr("tax_amount", "1uusd"),
        ]
    );

    // cannot withdraw more than (2 collateral token == 1 token)
    // due to min collateral ratio
    let msg = ExecuteMsg::Withdraw {
        position_idx: Uint128::from(2u128),
        collateral: Some(Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0001".to_string(),
            },
            amount: Uint128::from(2u128),
        }),
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(
            msg,
            "Cannot withdraw collateral over than minimum collateral ratio"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::Withdraw {
        position_idx: Uint128::from(2u128),
        collateral: Some(Asset {
            info: AssetInfo::Token {
                contract_addr: "asset0001".to_string(),
            },
            amount: Uint128::from(1u128),
        }),
    };
    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "withdraw"),
            attr("position_idx", "2"),
            attr("withdraw_amount", "1asset0001"),
            attr("tax_amount", "0asset0001"),
        ]
    );
}

#[test]
fn auction() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(5u64),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(100u128, 1u128),
        ),
        (
            &"asset0001".to_string(),
            &Decimal::from_ratio(50u128, 1u128),
        ),
    ]);
    deps.querier.with_collateral_infos(&[
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(100u128, 1u128),
            &Decimal::one(),
            &false,
        ),
        (
            &"asset0001".to_string(),
            &Decimal::from_ratio(50u128, 1u128),
            &Decimal::one(),
            &false,
        ),
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
        min_collateral_ratio: Decimal::percent(130),
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

    // open uusd-asset0000 position
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
        short_params: None,
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

    // open asset0001-asset0000 position
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        msg: to_binary(&Cw20HookMsg::OpenPosition {
            asset_info: AssetInfo::Token {
                contract_addr: "asset0000".to_string(),
            },
            collateral_ratio: Decimal::percent(150),
            short_params: None,
        })
        .unwrap(),
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
    });
    let env = mock_env_with_block_time(1000);
    let info = mock_info("asset0001", &[]);
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(115u128, 1u128),
        ),
        (
            &"asset0001".to_string(),
            &Decimal::from_ratio(50u128, 1u128),
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(1u128),
        msg: to_binary(&Cw20HookMsg::Auction {
            position_idx: Uint128::from(1u128),
        })
        .unwrap(),
    });

    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot liquidate a safely collateralized position")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(1u128),
        msg: to_binary(&Cw20HookMsg::Auction {
            position_idx: Uint128::from(1u128),
        })
        .unwrap(),
    });

    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot liquidate a safely collateralized position")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(116u128, 1u128),
        ),
        (
            &"asset0001".to_string(),
            &Decimal::from_ratio(50u128, 1u128),
        ),
    ]);
    deps.querier.with_collateral_infos(&[
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(116u128, 1u128),
            &Decimal::one(),
            &false,
        ),
        (
            &"asset0001".to_string(),
            &Decimal::from_ratio(50u128, 1u128),
            &Decimal::one(),
            &false,
        ),
    ]);

    // auction failed; liquidation amount is bigger than position amount
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(6667u128),
        msg: to_binary(&Cw20HookMsg::Auction {
            position_idx: Uint128::from(1u128),
        })
        .unwrap(),
    });

    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot liquidate more than the position amount")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // auction failed; liquidation amount is bigger than position amount
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(333334u128),
        msg: to_binary(&Cw20HookMsg::Auction {
            position_idx: Uint128::from(2u128),
        })
        .unwrap(),
    });

    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot liquidate more than the position amount")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // auction success
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(6666u128),
        msg: to_binary(&Cw20HookMsg::Auction {
            position_idx: Uint128::from(1u128),
        })
        .unwrap(),
    });

    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "addr0000".to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(31838u128) // Tax (5%) 33430 * 1 / 1.05 -> 31838
                }],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: Uint128::from(6666u128), // asset value in collateral =
                }) // 6666 * 116 = 773256 -- protocol fee = 7732
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "addr0001".to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    // Origin:          966,570
                    // ProtocolFee(1%): -7,732
                    // Tax(5%):         -45,659
                    amount: Uint128::from(913179u128)
                }],
            })),
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "collector0000".to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    // Origin:  7732
                    // Tax(5%): -369
                    amount: Uint128::from(7363u128)
                }]
            }))
        ],
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "auction"),
            attr("position_idx", "1"),
            attr("owner", "addr0000"),
            attr("return_collateral_amount", "958838uusd"),
            attr("liquidated_amount", "6666asset0000"),
            attr("tax_amount", "45659uusd"),
            attr("protocol_fee", "7732uusd"),
        ]
    );

    // If the price goes too high, the return collateral amount
    // must be capped to position's collateral amount
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (&"asset0000".to_string(), &Decimal::percent(200)),
        (&"asset0001".to_string(), &Decimal::percent(50)),
    ]);
    deps.querier.with_collateral_infos(&[
        (
            &"asset0000".to_string(),
            &Decimal::percent(200),
            &Decimal::one(),
            &false,
        ),
        (
            &"asset0001".to_string(),
            &Decimal::percent(50),
            &Decimal::one(),
            &false,
        ),
    ]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(210000u128),
        msg: to_binary(&Cw20HookMsg::Auction {
            position_idx: Uint128::from(2u128),
        })
        .unwrap(),
    });

    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    // cap to collateral amount
    // required_asset_amount = 1000000 * 50 * 0.8 / 200 = 200000
    // refund_asset_amount = 10000
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0001".to_string(),
                    amount: Uint128::from(10000u128),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: Uint128::from(200000u128), // asset value in collateral = 200000 *
                }) // 200 / 50 = 800000 -- protocol fee = 8000
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0001".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0001".to_string(),
                    amount: Uint128::from(992000u128), // protocol fee = 200000 * 0.01 = 2000
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0001".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "collector0000".to_string(),
                    amount: Uint128::from(8000u128),
                })
                .unwrap(),
                funds: vec![],
            }))
        ],
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "auction"),
            attr("position_idx", "2"),
            attr("owner", "addr0000"),
            attr("return_collateral_amount", "992000asset0001"),
            attr("liquidated_amount", "200000asset0000"),
            attr("tax_amount", "0asset0001"),
            attr("protocol_fee", "8000asset0001"),
        ]
    );
}

#[test]
fn liquidate_with_low_discount_test() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::zero(),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(1000u128, 1000u128),
        ),
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
        min_collateral_ratio: Decimal::percent(110),
        ipo_params: None,
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // open uusd-asset0000 position
    let msg = ExecuteMsg::OpenPosition {
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128::from(1000u128),
        },
        asset_info: AssetInfo::Token {
            contract_addr: "asset0000".to_string(),
        },
        collateral_ratio: Decimal::percent(110),
        short_params: None,
    };
    let env = mock_env_with_block_time(1000);
    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000u128),
        }],
    );
    let _res = execute(deps.as_mut(), env, info, msg).unwrap();

    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (
            &"asset0000".to_string(),
            &Decimal::from_ratio(1010u128, 1000u128),
        ),
    ]);

    // auction success
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(909u128),
        msg: to_binary(&Cw20HookMsg::Auction {
            position_idx: Uint128::from(1u128),
        })
        .unwrap(),
    });

    let env = mock_env_with_block_time(1000u64);
    let info = mock_info("asset0000", &[]);
    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    amount: Uint128::from(17u128),
                    recipient: "addr0001".to_string(),
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "asset0000".to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn {
                    amount: Uint128::from(892u128), // 892 + 17 = 909
                })
                .unwrap(),
                funds: vec![],
            })),
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "addr0001".to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(991u128)
                }],
            })),
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "collector0000".to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(9u128)
                }]
            }))
        ],
    );
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "auction"),
            attr("position_idx", "1"),
            attr("owner", "addr0000"),
            attr("return_collateral_amount", "991uusd"),
            attr("liquidated_amount", "892asset0000"),
            attr("tax_amount", "0uusd"),
            attr("protocol_fee", "9uusd"),
        ]
    );
}
