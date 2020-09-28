use cosmwasm_std::{
    from_binary, log, to_binary, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, HumanAddr,
    StdError, Uint128, WasmMsg,
};

use crate::contract::{handle, init, query};

use crate::msg::{
    AssetConfigResponse, ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, PositionResponse,
    PositionsResponse, QueryMsg,
};

use crate::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use terraswap::{Asset, AssetInfo};

static TOKEN_CODE_ID: u64 = 10u64;
#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0000", config.owner.as_str());
    assert_eq!("uusd", config.base_asset_info.to_string());
    assert_eq!("oracle0000", config.oracle.as_str());
    assert_eq!(TOKEN_CODE_ID, config.token_code_id);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    // update owner
    let env = mock_env("owner0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("owner0001".to_string())),
        token_code_id: Some(100u64),
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0001", config.owner.as_str());
    assert_eq!(100u64, config.token_code_id);

    // Unauthorzied err
    let env = mock_env("owner0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        token_code_id: None,
    };

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn register_asset() {
    let mut deps = mock_dependencies(20, &[]);

    let base_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: base_asset_info.clone(),
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(150),
    };

    let env = mock_env("owner0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(res.messages, vec![],);

    let res = query(
        &deps,
        QueryMsg::AssetConfig {
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        },
    )
    .unwrap();
    let asset_config: AssetConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        asset_config,
        AssetConfigResponse {
            token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
        }
    );

    // must be failed with the already registered token error
    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(150),
    };
    let env = mock_env("owner0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "Asset was already registered"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // must be failed with unauthorized error
    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(150),
    };
    let env = mock_env("owner0001", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn update_asset() {
    let mut deps = mock_dependencies(20, &[]);

    let base_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: base_asset_info.clone(),
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(150),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::UpdateAsset {
        asset_info: AssetInfo::Token {
            contract_addr: HumanAddr::from("asset0000"),
        },
        auction_discount: Some(Decimal::percent(30)),
        min_collateral_ratio: Some(Decimal::percent(200)),
    };
    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let res = query(
        &deps,
        QueryMsg::AssetConfig {
            asset_info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        },
    )
    .unwrap();
    let asset_config: AssetConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        asset_config,
        AssetConfigResponse {
            token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(30),
            min_collateral_ratio: Decimal::percent(200),
        }
    );

    let msg = HandleMsg::UpdateAsset {
        asset_info: AssetInfo::Token {
            contract_addr: HumanAddr::from("asset0000"),
        },
        auction_discount: Some(Decimal::percent(30)),
        min_collateral_ratio: Some(Decimal::percent(200)),
    };
    let env = mock_env("owner0001", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn open_position() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_oracle_price(&[
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::percent(100),
        ),
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::percent(50),
        ),
    ]);

    let base_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: base_asset_info.clone(),
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(150),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // must be failed; collateral ratio is too low
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
        collateral_ratio: Decimal::percent(140),
    };
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(1000000u128),
        }],
        1000,
    );
    let res = handle(&mut deps, env.clone(), msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(
            msg,
            "Can not open a position with low collateral ratio than minimum"
        ),
        _ => panic!("DO NOT ENTER ERROR"),
    }

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
    };
    let res = handle(&mut deps, env.clone(), msg).unwrap();

    assert_eq!(
        res.log,
        vec![
            log("action", "open_position"),
            log("position_idx", "1"),
            log("mint_amount", "666666asset0000"),
            log("collateral_amount", "1000000uusd"),
        ]
    );

    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128(666666u128),
            })
            .unwrap(),
        })]
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
        }
    );

    // can query positions
    let res = query(
        &deps,
        QueryMsg::Positions {
            owner_addr: HumanAddr::from("addr0000"),
            limit: None,
            start_after: None,
        },
    )
    .unwrap();
    let positions: PositionsResponse = from_binary(&res).unwrap();
    assert_eq!(
        positions,
        PositionsResponse {
            positions: vec![PositionResponse {
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
            }],
        }
    );

    // Cannot directly deposit token
    let msg = HandleMsg::OpenPosition {
        collateral: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            },
            amount: Uint128(1000000u128),
        },
        asset_info: AssetInfo::Token {
            contract_addr: HumanAddr::from("asset0000"),
        },
        collateral_ratio: Decimal::percent(150),
    };
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => (),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        msg: Some(
            to_binary(&Cw20HookMsg::OpenPosition {
                asset_info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                collateral_ratio: Decimal::percent(150),
            })
            .unwrap(),
        ),
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
    });
    let env = mock_env_with_block_time("asset0001", &[], 1000);
    let res = handle(&mut deps, env, msg).unwrap();

    assert_eq!(
        res.log,
        vec![
            log("action", "open_position"),
            log("position_idx", "2"),
            log("mint_amount", "333333asset0000"),
            log("collateral_amount", "1000000asset0001"),
        ]
    );

    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128(333333u128),
            })
            .unwrap(),
        })]
    );

    let res = query(
        &deps,
        QueryMsg::Position {
            position_idx: Uint128(2u128),
        },
    )
    .unwrap();
    let position: PositionResponse = from_binary(&res).unwrap();
    assert_eq!(
        position,
        PositionResponse {
            idx: Uint128(2u128),
            owner: HumanAddr::from("addr0000"),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                amount: Uint128(333333u128),
            },
            collateral: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0001"),
                },
                amount: Uint128(1000000u128),
            },
        }
    );

    // can query positions
    let res = query(
        &deps,
        QueryMsg::Positions {
            owner_addr: HumanAddr::from("addr0000"),
            limit: None,
            start_after: Some(Uint128(1u128)),
        },
    )
    .unwrap();
    let positions: PositionsResponse = from_binary(&res).unwrap();
    assert_eq!(
        positions,
        PositionsResponse {
            positions: vec![PositionResponse {
                idx: Uint128(2u128),
                owner: HumanAddr::from("addr0000"),
                asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0000"),
                    },
                    amount: Uint128(333333u128),
                },
                collateral: Asset {
                    info: AssetInfo::Token {
                        contract_addr: HumanAddr::from("asset0001"),
                    },
                    amount: Uint128(1000000u128),
                },
            }],
        }
    );
}

#[test]
fn deposit() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_oracle_price(&[
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::percent(100),
        ),
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::percent(50),
        ),
    ]);

    let base_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: base_asset_info.clone(),
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(150),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // open uusd-asset0000 position
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

    // open asset0001-asset0000 position
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        msg: Some(
            to_binary(&Cw20HookMsg::OpenPosition {
                asset_info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                collateral_ratio: Decimal::percent(150),
            })
            .unwrap(),
        ),
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
    });
    let env = mock_env_with_block_time("asset0001", &[], 1000);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Deposit {
        position_idx: Uint128(1u128),
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128(1000000u128),
        },
    };
    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(1000000u128),
        }],
    );
    let _res = handle(&mut deps, env, msg).unwrap();
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
                amount: Uint128(2000000u128),
            },
        }
    );

    // unauthorized failed; must be executed from token contract
    let msg = HandleMsg::Deposit {
        position_idx: Uint128(2u128),
        collateral: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            },
            amount: Uint128(1000000u128),
        },
    };
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("Must return unauthorized error"),
    }

    // deposit other token asset
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Deposit {
                position_idx: Uint128(2u128),
            })
            .unwrap(),
        ),
    });

    let env = mock_env("asset0001", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let res = query(
        &deps,
        QueryMsg::Position {
            position_idx: Uint128(2u128),
        },
    )
    .unwrap();

    let position: PositionResponse = from_binary(&res).unwrap();
    assert_eq!(
        position,
        PositionResponse {
            idx: Uint128(2u128),
            owner: HumanAddr::from("addr0000"),
            asset: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                amount: Uint128(333333u128),
            },
            collateral: Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0001"),
                },
                amount: Uint128(2000000u128),
            },
        }
    );
}

#[test]
fn mint() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_oracle_price(&[
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(100u64, 1u64),
        ),
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(50u64, 1u64),
        ),
    ]);

    let base_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: base_asset_info.clone(),
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(150),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // open uusd-asset0000 position
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

    // open asset0001-asset0000 position
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        msg: Some(
            to_binary(&Cw20HookMsg::OpenPosition {
                asset_info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                collateral_ratio: Decimal::percent(150),
            })
            .unwrap(),
        ),
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
    });
    let env = mock_env_with_block_time("asset0001", &[], 1000);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Deposit {
        position_idx: Uint128(1u128),
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128(1000000u128),
        },
    };
    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(1000000u128),
        }],
    );
    let _res = handle(&mut deps, env, msg).unwrap();

    // deposit other token asset
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Deposit {
                position_idx: Uint128(2u128),
            })
            .unwrap(),
        ),
    });

    let env = mock_env("asset0001", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // failed to mint; due to min_collateral_ratio
    // price 100, collateral 1000000, min_collateral_ratio 150%
    // x * price * min_collateral_ratio < collateral
    // x < collateral/(price*min_collateral_ratio) = 10000 / 1.5
    let msg = HandleMsg::Mint {
        position_idx: Uint128(1u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            amount: Uint128(6668u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot mint asset over than min collateral ratio")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // successfully mint within the min_collateral_ratio
    let msg = HandleMsg::Mint {
        position_idx: Uint128(1u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            amount: Uint128(6667u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Mint {
                amount: Uint128(6667u128),
                recipient: HumanAddr::from("addr0000"),
            })
            .unwrap(),
            send: vec![],
        })]
    );
    assert_eq!(
        res.log,
        vec![
            log("action", "mint"),
            log("position_idx", "1"),
            log("mint_amount", "6667asset0000")
        ]
    );

    // mint with other token; failed due to min collateral ratio
    let msg = HandleMsg::Mint {
        position_idx: Uint128(2u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            amount: Uint128(333334u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot mint asset over than min collateral ratio")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // mint with other token;
    let msg = HandleMsg::Mint {
        position_idx: Uint128(2u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            amount: Uint128(333333u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Mint {
                amount: Uint128(333333u128),
                recipient: HumanAddr::from("addr0000"),
            })
            .unwrap(),
            send: vec![],
        })]
    );
    assert_eq!(
        res.log,
        vec![
            log("action", "mint"),
            log("position_idx", "2"),
            log("mint_amount", "333333asset0000")
        ]
    );
}

#[test]
fn burn() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_oracle_price(&[
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(100u64, 1u64),
        ),
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(50u64, 1u64),
        ),
    ]);

    let base_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: base_asset_info.clone(),
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(150),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // open uusd-asset0000 position
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

    // open asset0001-asset0000 position
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        msg: Some(
            to_binary(&Cw20HookMsg::OpenPosition {
                asset_info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                collateral_ratio: Decimal::percent(150),
            })
            .unwrap(),
        ),
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
    });
    let env = mock_env_with_block_time("asset0001", &[], 1000);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Deposit {
        position_idx: Uint128(1u128),
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128(1000000u128),
        },
    };
    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(1000000u128),
        }],
    );
    let _res = handle(&mut deps, env, msg).unwrap();

    // deposit other token asset
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Deposit {
                position_idx: Uint128(2u128),
            })
            .unwrap(),
        ),
    });

    let env = mock_env("asset0001", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Mint {
        position_idx: Uint128(1u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            amount: Uint128(6667u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let _res = handle(&mut deps, env, msg).unwrap();

    // failed to burn more than the position amount
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(13334u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Burn {
                position_idx: Uint128(1u128),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "Cannot burn asset more than you mint"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(13333u128),
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
        res.log,
        vec![
            log("action", "burn"),
            log("position_idx", "1"),
            log("burn_amount", "13333asset0000"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Burn {
                amount: Uint128(13333u128),
            })
            .unwrap(),
            send: vec![],
        })]
    );

    // mint other asset
    let msg = HandleMsg::Mint {
        position_idx: Uint128(2u128),
        asset: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            amount: Uint128(333333u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let _res = handle(&mut deps, env, msg).unwrap();

    // failed to burn more than the position amount
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(666667u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Burn {
                position_idx: Uint128(2u128),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "Cannot burn asset more than you mint"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(666666u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Burn {
                position_idx: Uint128(2u128),
            })
            .unwrap(),
        ),
    });
    let env = mock_env("asset0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "burn"),
            log("position_idx", "2"),
            log("burn_amount", "666666asset0000"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Burn {
                amount: Uint128(666666u128),
            })
            .unwrap(),
            send: vec![],
        })]
    );
}

#[test]
fn withdraw() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );

    deps.querier.with_oracle_price(&[
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(100u64, 1u64),
        ),
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(50u64, 1u64),
        ),
    ]);

    let base_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: base_asset_info.clone(),
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(150),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // open uusd-asset0000 position
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

    // open asset0001-asset0000 position
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        msg: Some(
            to_binary(&Cw20HookMsg::OpenPosition {
                asset_info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                collateral_ratio: Decimal::percent(150),
            })
            .unwrap(),
        ),
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
    });
    let env = mock_env_with_block_time("asset0001", &[], 1000);
    let _res = handle(&mut deps, env, msg).unwrap();

    // cannot withdraw more than (100 collateral token == 1 token)
    // due to min collateral ratio
    let msg = HandleMsg::Withdraw {
        position_idx: Uint128(1u128),
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128(101u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(
            msg,
            "Cannot withdraw collateral over than minimum collateral ratio"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::Withdraw {
        position_idx: Uint128(1u128),
        collateral: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: Uint128(100u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "withdraw"),
            log("position_idx", "1"),
            log("withdraw_amount", "100uusd"),
            log("tax_amount", "1uusd"),
        ]
    );

    // cannot withdraw more than (2 collateral token == 1 token)
    // due to min collateral ratio
    let msg = HandleMsg::Withdraw {
        position_idx: Uint128(2u128),
        collateral: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            },
            amount: Uint128(2u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(
            msg,
            "Cannot withdraw collateral over than minimum collateral ratio"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::Withdraw {
        position_idx: Uint128(2u128),
        collateral: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            },
            amount: Uint128(1u128),
        },
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "withdraw"),
            log("position_idx", "2"),
            log("withdraw_amount", "1asset0001"),
            log("tax_amount", "0asset0001"),
        ]
    );
}

#[test]
fn auction() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::percent(5u64),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );

    deps.querier.with_oracle_price(&[
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(100u64, 1u64),
        ),
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(50u64, 1u64),
        ),
    ]);

    let base_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        oracle: HumanAddr::from("oracle0000"),
        base_asset_info: base_asset_info.clone(),
        token_code_id: TOKEN_CODE_ID,
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("asset0000"),
        auction_discount: Decimal::percent(20),
        min_collateral_ratio: Decimal::percent(130),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // open uusd-asset0000 position
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

    // open asset0001-asset0000 position
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        msg: Some(
            to_binary(&Cw20HookMsg::OpenPosition {
                asset_info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                collateral_ratio: Decimal::percent(150),
            })
            .unwrap(),
        ),
        sender: HumanAddr::from("addr0000"),
        amount: Uint128(1000000u128),
    });
    let env = mock_env_with_block_time("asset0001", &[], 1000);
    let _res = handle(&mut deps, env, msg).unwrap();

    deps.querier.with_oracle_price(&[
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(115u64, 1u64),
        ),
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(50u64, 1u64),
        ),
    ]);

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0001"),
        amount: Uint128(1u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Auction {
                position_idx: Uint128(1u128),
            })
            .unwrap(),
        ),
    });

    let env = mock_env_with_block_time("asset0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot liquidate a safely collateralized position")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0001"),
        amount: Uint128(1u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Auction {
                position_idx: Uint128(1u128),
            })
            .unwrap(),
        ),
    });

    let env = mock_env_with_block_time("asset0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot liquidate a safely collateralized position")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    deps.querier.with_oracle_price(&[
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(116u64, 1u64),
        ),
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(50u64, 1u64),
        ),
    ]);

    // auction failed; liquidation amont is bigger than position amount
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0001"),
        amount: Uint128(6667u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Auction {
                position_idx: Uint128(1u128),
            })
            .unwrap(),
        ),
    });

    let env = mock_env_with_block_time("asset0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot liquidate more than the position amount")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // auction failed; liquidation amont is bigger than position amount
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0001"),
        amount: Uint128(333334u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Auction {
                position_idx: Uint128(2u128),
            })
            .unwrap(),
        ),
    });

    let env = mock_env_with_block_time("asset0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "Cannot liquidate more than the position amount")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // auction success
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0001"),
        amount: Uint128(6666u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Auction {
                position_idx: Uint128(1u128),
            })
            .unwrap(),
        ),
    });

    let env = mock_env_with_block_time("asset0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                to_address: HumanAddr::from("addr0000"),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128(31838u128) // Tax (5%) 33430 * 1 / 1.05 -> 31838
                }],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("asset0000"),
                msg: to_binary(&Cw20HandleMsg::Burn {
                    amount: Uint128(6666u128),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                to_address: HumanAddr::from("addr0001"),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128(920542u128) // Tax (5%) 966570 * 1 / 1.05 -> 920542
                }],
            })
        ],
    );
    assert_eq!(
        res.log,
        vec![
            log("action", "auction"),
            log("owner", "addr0000"),
            log("return_collateral_amount", "966570uusd"),
            log("liquidated_amount", "6666asset0000"),
            log("tax_amount", "46028uusd"),
        ]
    );

    // If the price goes too high, the return collateral amount
    // must be capped to positions's collateral amount
    deps.querier.with_oracle_price(&[
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(200u64, 1u64),
        ),
        (
            &AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0001"),
            }
            .to_raw(&deps)
            .unwrap(),
            &Decimal::from_ratio(50u64, 1u64),
        ),
    ]);

    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0001"),
        amount: Uint128(210000u128),
        msg: Some(
            to_binary(&Cw20HookMsg::Auction {
                position_idx: Uint128(2u128),
            })
            .unwrap(),
        ),
    });

    let env = mock_env_with_block_time("asset0000", &[], 1000u64);
    let res = handle(&mut deps, env, msg).unwrap();
    // cap to collateral amount
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("asset0000"),
                msg: to_binary(&Cw20HandleMsg::Burn {
                    amount: Uint128(210000u128),
                })
                .unwrap(),
                send: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("asset0001"),
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from("addr0001"),
                    amount: Uint128(1000000u128),
                })
                .unwrap(),
                send: vec![],
            })
        ],
    );
    assert_eq!(
        res.log,
        vec![
            log("action", "auction"),
            log("owner", "addr0000"),
            log("return_collateral_amount", "1000000asset0001"),
            log("liquidated_amount", "210000asset0000"),
            log("tax_amount", "0asset0001"),
        ]
    );
}

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
