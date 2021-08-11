use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{
    attr, from_binary, to_binary, BankMsg, Coin, CosmosMsg, Decimal, StdError, SubMsg, Uint128,
    WasmMsg,
};

use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies;

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use mirror_protocol::common::OrderBy;
use mirror_protocol::limit_order::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, LastOrderIdResponse, OrderResponse, OrdersResponse,
    QueryMsg,
};
use terraswap::asset::{Asset, AssetInfo};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {};
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
}

#[test]
fn submit_order() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {};
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: "mAAPL".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: "mAAPL".to_string(),
            },
        },
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "must provide native token"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: "mAAPL".to_string(),
            },
        },
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(
            msg,
            "Native token balance mismatch between the argument and the transferred"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: "mAAPL".to_string(),
            },
        },
    };

    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "submit_order"),
            attr("order_id", 1.to_string()),
            attr("bidder_addr", "addr0000"),
            attr("offer_asset", "1000000uusd"),
            attr("ask_asset", "1000000mAAPL"),
        ]
    );

    assert_eq!(
        from_binary::<LastOrderIdResponse>(
            &query(deps.as_ref(), mock_env(), QueryMsg::LastOrderId {}).unwrap()
        )
        .unwrap(),
        LastOrderIdResponse { last_order_id: 1 }
    );

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::new(1000000u128),
        msg: to_binary(&Cw20HookMsg::SubmitOrder {
            ask_asset: Asset {
                amount: Uint128::from(1000000u128),
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
            },
        })
        .unwrap(),
    });

    let info = mock_info("mAAPL", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "submit_order"),
            attr("order_id", 2.to_string()),
            attr("bidder_addr", "addr0000"),
            attr("offer_asset", "1000000mAAPL"),
            attr("ask_asset", "1000000uusd"),
        ]
    );
    assert_eq!(
        from_binary::<LastOrderIdResponse>(
            &query(deps.as_ref(), mock_env(), QueryMsg::LastOrderId {}).unwrap()
        )
        .unwrap(),
        LastOrderIdResponse { last_order_id: 2 }
    );
}

#[test]
fn cancel_order_native_token() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128::new(1000000u128))],
    );

    let msg = InstantiateMsg {};
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: "mAAPL".to_string(),
            },
        },
    };

    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::CancelOrder { order_id: 1 };

    // failed verfication failed
    let wrong_info = mock_info("addr0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), wrong_info, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "cancel_order"),
            attr("order_id", 1.to_string()),
            attr("bidder_refund", "1000000uusd")
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "addr0000".to_string(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(990099u128),
            }]
        }))]
    );

    // failed no order exists
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone());
    assert_eq!(true, res.is_err());
}

#[test]
fn cancel_order_token() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128::new(1000000u128))],
    );

    let msg = InstantiateMsg {};
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::new(1000000u128),
        msg: to_binary(&Cw20HookMsg::SubmitOrder {
            ask_asset: Asset {
                amount: Uint128::from(1000000u128),
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
            },
        })
        .unwrap(),
    });

    let info = mock_info("mAAPL", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::CancelOrder { order_id: 1 };

    // failed verfication failed
    let wrong_info = mock_info("addr0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), wrong_info, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "cancel_order"),
            attr("order_id", 1.to_string()),
            attr("bidder_refund", "1000000mAAPL")
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "mAAPL".to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                amount: Uint128::new(1000000u128),
                recipient: "addr0000".to_string(),
            })
            .unwrap(),
        }))]
    );

    // failed no order exists
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone());
    assert_eq!(true, res.is_err());
}

#[test]
fn execute_order_native_token() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[
            (&"uusd".to_string(), &Uint128::new(1000000u128)),
            (&"ukrw".to_string(), &Uint128::new(1000000u128)),
        ],
    );

    let msg = InstantiateMsg {};
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        },
    };

    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // assertion; native asset balance
    let msg = ExecuteMsg::ExecuteOrder {
        execute_asset: Asset {
            amount: Uint128::new(500000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        order_id: 1u64,
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(
            msg,
            "Native token balance mismatch between the argument and the transferred"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // cannot execute order with other asset
    let info = mock_info(
        "addr0001",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(500000u128),
        }],
    );
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "invalid asset given"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // partial execute
    let info = mock_info(
        "addr0001",
        &[Coin {
            denom: "ukrw".to_string(),
            amount: Uint128::from(500000u128),
        }],
    );
    let msg = ExecuteMsg::ExecuteOrder {
        execute_asset: Asset {
            amount: Uint128::new(500000u128),
            info: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        },
        order_id: 1u64,
    };
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "execute_order"),
            attr("order_id", 1.to_string()),
            attr("executor_receive", "500000uusd"),
            attr("bidder_receive", "500000ukrw"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "addr0001".to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(495049u128)
                }]
            })),
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "addr0000".to_string(),
                amount: vec![Coin {
                    denom: "ukrw".to_string(),
                    amount: Uint128::from(495049u128)
                }]
            })),
        ]
    );

    let resp: OrderResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Order { order_id: 1 }).unwrap())
            .unwrap();
    assert_eq!(resp.filled_ask_amount, Uint128::new(500000u128));
    assert_eq!(resp.filled_offer_amount, Uint128::new(500000u128));

    // fill left amount
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "execute_order"),
            attr("order_id", 1.to_string()),
            attr("executor_receive", "500000uusd"),
            attr("bidder_receive", "500000ukrw"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "addr0001".to_string(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(495049u128)
                }]
            })),
            SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "addr0000".to_string(),
                amount: vec![Coin {
                    denom: "ukrw".to_string(),
                    amount: Uint128::from(495049u128)
                }]
            })),
        ]
    );

    assert_eq!(
        true,
        query(deps.as_ref(), mock_env(), QueryMsg::Order { order_id: 1 }).is_err()
    );
}

#[test]
fn execute_order_token() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[
            (&"uusd".to_string(), &Uint128::new(1000000u128)),
            (&"ukrw".to_string(), &Uint128::new(1000000u128)),
        ],
    );

    let msg = InstantiateMsg {};
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::SubmitOrder {
            ask_asset: Asset {
                amount: Uint128::from(1000000u128),
                info: AssetInfo::Token {
                    contract_addr: "token0001".to_string(),
                },
            },
        })
        .unwrap(),
    });

    let info = mock_info("token0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // cannot execute order with other asset
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(500000u128),
        msg: to_binary(&Cw20HookMsg::ExecuteOrder { order_id: 1u64 }).unwrap(),
    });

    let info = mock_info("token0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "invalid asset given"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // partial execute
    let info = mock_info("token0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "execute_order"),
            attr("order_id", 1.to_string()),
            attr("executor_receive", "500000token0000"),
            attr("bidder_receive", "500000token0001"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "token0000".to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0001".to_string(),
                    amount: Uint128::from(500000u128)
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "token0001".to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0000".to_string(),
                    amount: Uint128::from(500000u128)
                })
                .unwrap(),
            })),
        ]
    );

    let resp: OrderResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Order { order_id: 1 }).unwrap())
            .unwrap();
    assert_eq!(resp.filled_ask_amount, Uint128::new(500000u128));
    assert_eq!(resp.filled_offer_amount, Uint128::new(500000u128));

    // fill left amount
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "execute_order"),
            attr("order_id", 1.to_string()),
            attr("executor_receive", "500000token0000"),
            attr("bidder_receive", "500000token0001"),
        ]
    );
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "token0000".to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0001".to_string(),
                    amount: Uint128::from(500000u128)
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "token0001".to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "addr0000".to_string(),
                    amount: Uint128::from(500000u128)
                })
                .unwrap(),
            })),
        ]
    );

    assert_eq!(
        true,
        query(deps.as_ref(), mock_env(), QueryMsg::Order { order_id: 1 }).is_err()
    );
}

#[test]
fn orders_querier() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_tax(
        Decimal::percent(1),
        &[
            (&"uusd".to_string(), &Uint128::new(1000000u128)),
            (&"ukrw".to_string(), &Uint128::new(1000000u128)),
        ],
    );

    let msg = InstantiateMsg {};
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::SubmitOrder {
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        },
    };

    let info = mock_info(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
    );
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(1000000u128),
        msg: to_binary(&Cw20HookMsg::SubmitOrder {
            ask_asset: Asset {
                amount: Uint128::from(1000000u128),
                info: AssetInfo::Token {
                    contract_addr: "token0001".to_string(),
                },
            },
        })
        .unwrap(),
    });

    let info = mock_info("token0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let order_1 = OrderResponse {
        order_id: 1u64,
        bidder_addr: "addr0000".to_string(),
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::NativeToken {
                denom: "ukrw".to_string(),
            },
        },
        filled_offer_amount: Uint128::zero(),
        filled_ask_amount: Uint128::zero(),
    };

    let order_2 = OrderResponse {
        order_id: 2u64,
        bidder_addr: "addr0000".to_string(),
        offer_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: "token0000".to_string(),
            },
        },
        ask_asset: Asset {
            amount: Uint128::from(1000000u128),
            info: AssetInfo::Token {
                contract_addr: "token0001".to_string(),
            },
        },
        filled_offer_amount: Uint128::zero(),
        filled_ask_amount: Uint128::zero(),
    };

    assert_eq!(
        OrdersResponse {
            orders: vec![order_1.clone(), order_2.clone(),],
        },
        from_binary::<OrdersResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::Orders {
                    bidder_addr: Some("addr0000".to_string()),
                    start_after: None,
                    limit: None,
                    order_by: Some(OrderBy::Asc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    assert_eq!(
        OrdersResponse {
            orders: vec![order_1.clone(), order_2.clone(),],
        },
        from_binary::<OrdersResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::Orders {
                    bidder_addr: None,
                    start_after: None,
                    limit: None,
                    order_by: Some(OrderBy::Asc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    // DESC test
    assert_eq!(
        OrdersResponse {
            orders: vec![order_2.clone(), order_1.clone(),],
        },
        from_binary::<OrdersResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::Orders {
                    bidder_addr: None,
                    start_after: None,
                    limit: None,
                    order_by: Some(OrderBy::Desc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    // different bidder
    assert_eq!(
        OrdersResponse { orders: vec![] },
        from_binary::<OrdersResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::Orders {
                    bidder_addr: Some("addr0001".to_string()),
                    start_after: None,
                    limit: None,
                    order_by: None,
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    // start after DESC
    assert_eq!(
        OrdersResponse {
            orders: vec![order_1.clone()],
        },
        from_binary::<OrdersResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::Orders {
                    bidder_addr: None,
                    start_after: Some(2u64),
                    limit: None,
                    order_by: Some(OrderBy::Desc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );

    // start after ASC
    assert_eq!(
        OrdersResponse {
            orders: vec![order_2.clone()],
        },
        from_binary::<OrdersResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::Orders {
                    bidder_addr: None,
                    start_after: Some(1u64),
                    limit: None,
                    order_by: Some(OrderBy::Asc),
                }
            )
            .unwrap()
        )
        .unwrap()
    );
}
