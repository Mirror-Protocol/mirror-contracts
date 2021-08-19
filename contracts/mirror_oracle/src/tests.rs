use crate::contract::{execute, instantiate, query};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_binary, Decimal, StdError};
use mirror_protocol::common::OrderBy;
use mirror_protocol::oracle::{
    ConfigResponse, ExecuteMsg, FeederResponse, InstantiateMsg, PriceResponse, PricesResponse,
    PricesResponseElem, QueryMsg,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        base_asset: "base0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0000", config.owner);
    assert_eq!("base0000", config.base_asset);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        base_asset: "base0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // update owner
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("owner0001".to_string()),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();

    assert_eq!("owner0001", config.owner);
    assert_eq!("base0000", config.base_asset);

    // Unauthorized err
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::UpdateConfig { owner: None };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn update_price() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        base_asset: "base0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // register asset
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "mAAPL".to_string(),
        feeder: "addr0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "mAAPL".to_string(),
        feeder: "addr0001".to_string(),
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // try update an asset already exists
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "mAAPL".to_string(),
        feeder: "addr0000".to_string(),
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // update price
    let msg = ExecuteMsg::FeedPrice {
        prices: vec![("mAAPL".to_string(), Decimal::from_ratio(12u128, 10u128))],
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let query_result = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Price {
            base_asset: "mAAPL".to_string(),
            quote_asset: "base0000".to_string(),
        },
    )
    .unwrap();
    let value: PriceResponse = from_binary(&query_result).unwrap();
    assert_eq!("1.2", format!("{}", value.rate));

    // Unauthorzied err
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::FeedPrice {
        prices: vec![("mAAPL".to_string(), Decimal::from_ratio(12u128, 10u128))],
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn feed_price() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        base_asset: "base0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // update price
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::FeedPrice {
        prices: vec![("mAAPL".to_string(), Decimal::from_ratio(12u128, 10u128))],
    };

    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "mAAPL".to_string(),
        feeder: "addr0000".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "mAAPL".to_string(),
        feeder: "addr0000".to_string(),
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "mGOGL".to_string(),
        feeder: "addr0000".to_string(),
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Feeder {
            asset_token: "mAAPL".to_string(),
        },
    )
    .unwrap();
    let feeder_res: FeederResponse = from_binary(&res).unwrap();

    assert_eq!(
        feeder_res,
        FeederResponse {
            asset_token: "mAAPL".to_string(),
            feeder: "addr0000".to_string(),
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Price {
            base_asset: "mAAPL".to_string(),
            quote_asset: "base0000".to_string(),
        },
    )
    .unwrap();
    let price_res: PriceResponse = from_binary(&res).unwrap();

    assert_eq!(
        price_res,
        PriceResponse {
            rate: Decimal::zero(),
            last_updated_base: 0u64,
            last_updated_quote: u64::MAX,
        }
    );

    let msg = ExecuteMsg::FeedPrice {
        prices: vec![
            ("mAAPL".to_string(), Decimal::from_ratio(12u128, 10u128)),
            ("mGOGL".to_string(), Decimal::from_ratio(22u128, 10u128)),
        ],
    };
    let info = mock_info("addr0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let env = mock_env();
    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::Price {
            base_asset: "mAAPL".to_string(),
            quote_asset: "base0000".to_string(),
        },
    )
    .unwrap();
    let price_res: PriceResponse = from_binary(&res).unwrap();

    assert_eq!(
        price_res,
        PriceResponse {
            rate: Decimal::from_ratio(12u128, 10u128),
            last_updated_base: env.block.time.seconds(),
            last_updated_quote: u64::MAX,
        }
    );

    let env = mock_env();
    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::Prices {
            start_after: None,
            limit: None,
            order_by: Some(OrderBy::Asc),
        },
    )
    .unwrap();
    let prices_res: PricesResponse = from_binary(&res).unwrap();

    assert_eq!(
        prices_res,
        PricesResponse {
            prices: vec![
                PricesResponseElem {
                    asset_token: "mAAPL".to_string(),
                    price: Decimal::from_ratio(12u128, 10u128),
                    last_updated_time: env.block.time.seconds(),
                },
                PricesResponseElem {
                    asset_token: "mGOGL".to_string(),
                    price: Decimal::from_ratio(22u128, 10u128),
                    last_updated_time: env.block.time.seconds(),
                }
            ],
        }
    );

    // Unautorized try
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::FeedPrice {
        prices: vec![("mAAPL".to_string(), Decimal::from_ratio(12u128, 10u128))],
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}
