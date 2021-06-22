use crate::contract::{handle, init, query};
use cosmwasm_std::testing::{mock_dependencies, mock_env};
use cosmwasm_std::{
    from_binary, Decimal, HandleResponse, HandleResult, HumanAddr, InitResponse, StdError,
};
use mirror_protocol::common::OrderBy;
use mirror_protocol::oracle::{
    ConfigResponse, FeederResponse, HandleMsg, InitMsg, PriceResponse, PricesResponse,
    PricesResponseElem, QueryMsg,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr::from("owner0000"),
        base_asset: "base0000".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let res: InitResponse = init(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&mut deps, QueryMsg::Config {}).unwrap();
    let value: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0000", value.owner.as_str());
    assert_eq!("base0000", value.base_asset);
}

#[test]
fn update_owner() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        base_asset: "base0000".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();
    // update owner
    let env = mock_env("owner0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("owner0001".to_string())),
    };

    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let query_result = query(&mut deps, QueryMsg::Config {}).unwrap();
    let value: ConfigResponse = from_binary(&query_result).unwrap();
    assert_eq!("owner0001", value.owner.as_str());
    assert_eq!("base0000", value.base_asset);

    // Unauthorzied err
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig { owner: None };

    let res: HandleResult = handle(&mut deps, env, msg);
    match res.unwrap_err() {
        StdError::Unauthorized { .. } => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn update_price() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        base_asset: "base0000".to_string(),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res: InitResponse = init(&mut deps, env, msg).unwrap();

    // register asset
    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("mAAPL"),
        feeder: HumanAddr::from("addr0000"),
    };

    let env = mock_env("addr0000", &[]);
    let res: HandleResult = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("mAAPL"),
        feeder: HumanAddr::from("addr0001"),
    };

    let env = mock_env("owner0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // try update an asset already exists
    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("mAAPL"),
        feeder: HumanAddr::from("addr0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res: HandleResponse = handle(&mut deps, env, msg).unwrap();

    // update price
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::FeedPrice {
        prices: vec![(
            HumanAddr::from("mAAPL"),
            Decimal::from_ratio(12u128, 10u128),
        )],
    };

    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let query_result = query(
        &mut deps,
        QueryMsg::Price {
            base_asset: "mAAPL".to_string(),
            quote_asset: "base0000".to_string(),
        },
    )
    .unwrap();
    let value: PriceResponse = from_binary(&query_result).unwrap();
    assert_eq!("1.2", format!("{}", value.rate));

    // Unauthorzied err
    let env = mock_env("addr0001", &[]);
    let msg = HandleMsg::FeedPrice {
        prices: vec![(
            HumanAddr::from("mAAPL"),
            Decimal::from_ratio(12u128, 10u128),
        )],
    };

    let res: HandleResult = handle(&mut deps, env, msg);
    match res.unwrap_err() {
        StdError::Unauthorized { .. } => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn feed_price() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner: HumanAddr("owner0000".to_string()),
        base_asset: "base0000".to_string(),
    };

    let env = mock_env("addr0000", &[]);
    let _res = init(&mut deps, env, msg).unwrap();

    // update price
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::FeedPrice {
        prices: vec![(
            HumanAddr::from("mAAPL"),
            Decimal::from_ratio(12u128, 10u128),
        )],
    };

    let _res = handle(&mut deps, env, msg).unwrap_err();

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("mAAPL"),
        feeder: HumanAddr::from("addr0000"),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => {}
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("mAAPL"),
        feeder: HumanAddr::from("addr0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();
    let msg = HandleMsg::RegisterAsset {
        asset_token: HumanAddr::from("mGOGL"),
        feeder: HumanAddr::from("addr0000"),
    };

    let env = mock_env("owner0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let value: FeederResponse = from_binary(
        &query(
            &deps,
            QueryMsg::Feeder {
                asset_token: HumanAddr::from("mAAPL"),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value,
        FeederResponse {
            asset_token: HumanAddr::from("mAAPL"),
            feeder: HumanAddr::from("addr0000"),
        }
    );

    let value: PriceResponse = from_binary(
        &query(
            &deps,
            QueryMsg::Price {
                base_asset: "mAAPL".to_string(),
                quote_asset: "base0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value,
        PriceResponse {
            rate: Decimal::zero(),
            last_updated_base: 0u64,
            last_updated_quote: u64::MAX,
        }
    );

    let msg = HandleMsg::FeedPrice {
        prices: vec![
            (
                HumanAddr::from("mAAPL"),
                Decimal::from_ratio(12u128, 10u128),
            ),
            (
                HumanAddr::from("mGOGL"),
                Decimal::from_ratio(22u128, 10u128),
            ),
        ],
    };
    let env = mock_env("addr0000", &[]);
    let _res = handle(&mut deps, env.clone(), msg).unwrap();
    let value: PriceResponse = from_binary(
        &query(
            &deps,
            QueryMsg::Price {
                base_asset: "mAAPL".to_string(),
                quote_asset: "base0000".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value,
        PriceResponse {
            rate: Decimal::from_ratio(12u128, 10u128),
            last_updated_base: env.block.time,
            last_updated_quote: u64::MAX,
        }
    );

    let value: PricesResponse = from_binary(
        &query(
            &deps,
            QueryMsg::Prices {
                start_after: None,
                limit: None,
                order_by: Some(OrderBy::Asc),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value,
        PricesResponse {
            prices: vec![
                PricesResponseElem {
                    asset_token: HumanAddr::from("mAAPL"),
                    price: Decimal::from_ratio(12u128, 10u128),
                    last_updated_time: env.block.time,
                },
                PricesResponseElem {
                    asset_token: HumanAddr::from("mGOGL"),
                    price: Decimal::from_ratio(22u128, 10u128),
                    last_updated_time: env.block.time,
                }
            ],
        }
    );

    // Unautorized try
    let env = mock_env("addr0001", &[]);
    let msg = HandleMsg::FeedPrice {
        prices: vec![(
            HumanAddr::from("mAAPL"),
            Decimal::from_ratio(12u128, 10u128),
        )],
    };

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}
