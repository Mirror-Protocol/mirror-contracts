use cosmwasm_std::{
    to_binary, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, HumanAddr, StdError, Uint128,
    WasmMsg,
};

use crate::contract::{handle, init, query_asset, query_config, query_position};

use crate::msg::{
    ConfigAssetResponse, ConfigGeneralResponse, HandleMsg, InitMsg, PositionResponse,
};

use crate::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cw20::Cw20HandleMsg;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        auction_discount: Decimal::percent(10),
        auction_threshold_rate: Decimal::percent(85),
        mint_capacity: Decimal::percent(70),
        asset_oracle: HumanAddr::from("oracle0000"),
        asset_token: HumanAddr::from("asset0000"),
        asset_symbol: "mAPPL".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let res = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let config: ConfigGeneralResponse = query_config(&deps).unwrap();
    assert_eq!("addr0000", config.owner.as_str());
    assert_eq!("uusd", config.collateral_denom.as_str());
    assert_eq!(Decimal::percent(10), config.auction_discount);
    assert_eq!(Decimal::percent(85), config.auction_threshold_rate);
    assert_eq!(Decimal::percent(70), config.mint_capacity);

    let asset: ConfigAssetResponse = query_asset(&deps).unwrap();
    assert_eq!("oracle0000", asset.oracle.as_str());
    assert_eq!("asset0000", asset.token.as_str());
    assert_eq!("mAPPL", asset.symbol.as_str());
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        auction_discount: Decimal::percent(10),
        auction_threshold_rate: Decimal::percent(85),
        mint_capacity: Decimal::percent(70),
        asset_oracle: HumanAddr::from("oracle0000"),
        asset_token: HumanAddr::from("asset0000"),
        asset_symbol: "mAPPL".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // update owner
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("addr0001".to_string())),
        auction_discount: None,
        auction_threshold_rate: None,
        mint_capacity: None,
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let value = query_config(&deps).unwrap();
    assert_eq!("addr0001", value.owner.as_str());
    assert_eq!(Decimal::percent(10), value.auction_discount);
    assert_eq!(Decimal::percent(85), value.auction_threshold_rate);
    assert_eq!(Decimal::percent(70), value.mint_capacity);

    // update left items
    let env = mock_env("addr0001", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        auction_discount: Some(Decimal::percent(20)),
        auction_threshold_rate: Some(Decimal::percent(75)),
        mint_capacity: Some(Decimal::percent(80)),
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let value = query_config(&deps).unwrap();
    assert_eq!("addr0001", value.owner.as_str());
    assert_eq!(Decimal::percent(20), value.auction_discount);
    assert_eq!(Decimal::percent(75), value.auction_threshold_rate);
    assert_eq!(Decimal::percent(80), value.mint_capacity);

    // Unauthorzied err
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        auction_discount: None,
        auction_threshold_rate: None,
        mint_capacity: None,
    };

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn mint() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier
        .with_oracle_price(&[(&HumanAddr("oracle0000".to_string()), &Decimal::one())]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        auction_discount: Decimal::percent(10),
        auction_threshold_rate: Decimal::percent(85),
        mint_capacity: Decimal::percent(70),
        asset_oracle: HumanAddr::from("oracle0000"),
        asset_token: HumanAddr::from("asset0000"),
        asset_symbol: "mAPPL".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // fail mint request due to price is too old
    let msg = HandleMsg::Mint {};
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1000000u128),
        }],
        1031,
    );

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Price is too old"),
        _ => panic!("Must return generic error"),
    }

    // fail the mint request due to swap amount is too small
    let msg = HandleMsg::Mint {};
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(1u128),
        }],
        1000,
    );

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Mint amount is too small"),
        _ => panic!("Must return generic error"),
    }

    // successfully mint
    let msg = HandleMsg::Mint {};
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10u128),
        }],
        1030,
    );

    let res = handle(&mut deps, env, msg).unwrap();
    let msg = res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr("asset0000".to_string()),
            msg: to_binary(&Cw20HandleMsg::Mint {
                amount: Uint128::from(7u128),
                recipient: HumanAddr("addr0000".to_string()),
            })
            .unwrap(),
            send: vec![],
        })
    );

    let position_res: PositionResponse =
        query_position(&deps, HumanAddr("addr0000".to_string())).unwrap();
    assert_eq!(7u128, position_res.asset_amount.u128());
    assert_eq!(10u128, position_res.collateral_amount.u128());
    assert_eq!(false, position_res.is_auction_open);

    // collateral 10, asset 7, price 1, auction begin at capacity == 80%;
    // change price to 1.3; then the auction will begin
    let prices: &[(&HumanAddr, &Decimal)] = &[(
        &HumanAddr("oracle0000".to_string()),
        &Decimal::percent(130u64),
    )];

    deps.querier.with_oracle_price(prices);
    let position_res: PositionResponse =
        query_position(&deps, HumanAddr("addr0000".to_string())).unwrap();
    assert_eq!(true, position_res.is_auction_open);

    // put more collateral to close auction
    let msg = HandleMsg::Mint {};
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10u128),
        }],
        1000,
    );

    // mint more tokens to meet capacity 70%
    let res = handle(&mut deps, env, msg).unwrap();
    let msg = res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr("asset0000".to_string()),
            msg: to_binary(&Cw20HandleMsg::Mint {
                amount: Uint128::from(3u128),
                recipient: HumanAddr("addr0000".to_string()),
            })
            .unwrap(),
            send: vec![],
        })
    );

    let position_res: PositionResponse =
        query_position(&deps, HumanAddr("addr0000".to_string())).unwrap();
    assert_eq!(false, position_res.is_auction_open);
}

#[test]
fn burn() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::permille(5),
        &[(&"uusd".to_string(), &Uint128(1u128))],
    );
    deps.querier
        .with_oracle_price(&[(&HumanAddr("oracle0000".to_string()), &Decimal::one())]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        auction_discount: Decimal::percent(10),
        auction_threshold_rate: Decimal::percent(85),
        mint_capacity: Decimal::percent(70),
        asset_oracle: HumanAddr::from("oracle0000"),
        asset_token: HumanAddr::from("asset0000"),
        asset_symbol: "mAPPL".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // failed to burn
    let msg = HandleMsg::Burn {
        amount: Uint128::from(10u128),
    };

    let env = mock_env_with_block_time("addr0000", &[], 1000);
    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Burn amount is bigger than the position amount")
        }
        _ => panic!("Must return generic error"),
    }

    // mint
    let msg = HandleMsg::Mint {};
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128),
        }],
        1030,
    );

    let _res = handle(&mut deps, env, msg).unwrap();

    // successfully burn 50 APPL,
    // then 20 APPL left. the 72 collateral token will be refund
    let msg = HandleMsg::Burn {
        amount: Uint128::from(50u128),
    };

    let env = mock_env_with_block_time("addr0000", &[], 1000);
    let res = handle(&mut deps, env, msg).unwrap();
    let msg = res.messages.get(1).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr("addr0000".to_string()),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(72u128)
            }],
        })
    );

    let position: PositionResponse =
        query_position(&deps, HumanAddr("addr0000".to_string())).unwrap();
    assert_eq!(28u128, position.collateral_amount.u128());
    assert_eq!(20u128, position.asset_amount.u128());

    // increase oracle price then, the collateral must be stay
    let prices: &[(&HumanAddr, &Decimal)] = &[(
        &HumanAddr("oracle0000".to_string()),
        &Decimal::percent(130u64),
    )];

    deps.querier.with_oracle_price(prices);

    let msg = HandleMsg::Burn {
        amount: Uint128::from(1u128),
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(1, res.messages.len());

    let position: PositionResponse =
        query_position(&deps, HumanAddr("addr0000".to_string())).unwrap();
    assert_eq!(28u128, position.collateral_amount.u128());
    assert_eq!(19u128, position.asset_amount.u128());

    // burn all
    let msg = HandleMsg::Burn {
        amount: Uint128::from(19u128),
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000);
    let res = handle(&mut deps, env, msg).unwrap();
    let msg = res.messages.get(1).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr("addr0000".to_string()),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(28u128)
            }],
        })
    );

    let position: PositionResponse =
        query_position(&deps, HumanAddr("addr0000".to_string())).unwrap();
    assert_eq!(0u128, position.collateral_amount.u128());
    assert_eq!(0u128, position.asset_amount.u128());
}

#[test]
fn auction() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier.with_tax(
        Decimal::permille(5),
        &[(&"uusd".to_string(), &Uint128(1u128))],
    );
    deps.querier
        .with_oracle_price(&[(&HumanAddr("oracle0000".to_string()), &Decimal::one())]);

    let msg = InitMsg {
        collateral_denom: "uusd".to_string(),
        auction_discount: Decimal::percent(10),
        auction_threshold_rate: Decimal::percent(85),
        mint_capacity: Decimal::percent(70),
        asset_oracle: HumanAddr::from("oracle0000"),
        asset_token: HumanAddr::from("asset0000"),
        asset_symbol: "mAPPL".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // mint
    let msg = HandleMsg::Mint {};
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128),
        }],
        1000,
    );

    let _res = handle(&mut deps, env, msg).unwrap();

    // increase oracle price then to make the auction open
    let prices: &[(&HumanAddr, &Decimal)] =
        &[(&HumanAddr("oracle0000".to_string()), &Decimal::percent(130))];

    deps.querier.with_oracle_price(prices);

    let msg = HandleMsg::Auction {
        owner: HumanAddr::from("addr0000"),
        amount: Uint128::from(50u128),
    };

    let env = mock_env_with_block_time("addr0001", &[], 1000);

    // discount price = 1.43; 50 * 1.43 = 71 uusd will be returned
    let res = handle(&mut deps, env, msg).unwrap();
    let msg = res.messages.get(1).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr("addr0001".to_string()),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(71u128)
            }],
        })
    );

    let position: PositionResponse =
        query_position(&deps, HumanAddr("addr0000".to_string())).unwrap();
    assert_eq!(29u128, position.collateral_amount.u128());
    assert_eq!(20u128, position.asset_amount.u128());

    // auction all
    let msg = HandleMsg::Auction {
        owner: HumanAddr::from("addr0000"),
        amount: Uint128::from(20u128),
    };

    let env = mock_env_with_block_time("addr0001", &[], 1000);

    // discount price = 1.43; 20 * 1.43 = 28 uusd will be returned
    let res = handle(&mut deps, env, msg).unwrap();
    // refund to position owner
    let msg = res.messages.get(0).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr("addr0000".to_string()),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(1u128)
            }],
        })
    );
    // send funds to auction winner
    let msg = res.messages.get(2).expect("no message");
    assert_eq!(
        msg,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr("addr0001".to_string()),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(28u128)
            }],
        })
    );

    let position: PositionResponse =
        query_position(&deps, HumanAddr("addr0000".to_string())).unwrap();
    assert_eq!(0u128, position.collateral_amount.u128());
    assert_eq!(0u128, position.asset_amount.u128());
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
