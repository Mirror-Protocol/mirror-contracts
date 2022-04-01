use crate::contract::{
    execute, instantiate, query_collateral_info, query_collateral_price, query_config,
};
use crate::testing::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{Decimal, StdError, Uint128};
use mirror_protocol::collateral_oracle::{
    CollateralInfoResponse, CollateralPriceResponse, ExecuteMsg, InstantiateMsg, SourceType,
};
use terraswap::asset::AssetInfo;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let value = query_config(deps.as_ref()).unwrap();
    assert_eq!("owner0000", value.owner.as_str());
    assert_eq!("mint0000", value.mint_contract.as_str());
    assert_eq!("uusd", value.base_denom.as_str());
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // update owner
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("owner0001".to_string()),
        mint_contract: Some("mint0001".to_string()),
        base_denom: Some("uluna".to_string()),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let value = query_config(deps.as_ref()).unwrap();
    assert_eq!("owner0001", value.owner.as_str());
    assert_eq!("mint0001", value.mint_contract.as_str());
    assert_eq!("uluna", value.base_denom.as_str());

    // Unauthorized err
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        mint_contract: None,
        base_denom: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn register_collateral() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (&"mTSLA".to_string(), &Decimal::percent(100)),
    ]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "mTSLA".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::TefiOracle {
            oracle_addr: "mirrorOracle0000".to_string(),
        },
    };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));

    // successfull attempt
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // query collateral info
    let query_res = query_collateral_info(deps.as_ref(), "mTSLA".to_string()).unwrap();
    assert_eq!(
        query_res,
        CollateralInfoResponse {
            asset: "mTSLA".to_string(),
            source_type: "tefi_oracle".to_string(),
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    )
}

#[test]
fn update_collateral() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (&"mTSLA".to_string(), &Decimal::percent(100)),
    ]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "mTSLA".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::TefiOracle {
            oracle_addr: "mirrorOracle0000".to_string(),
        },
    };

    // successfull attempt
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // query collateral info
    let query_res = query_collateral_info(deps.as_ref(), "mTSLA".to_string()).unwrap();
    assert_eq!(
        query_res,
        CollateralInfoResponse {
            asset: "mTSLA".to_string(),
            source_type: "tefi_oracle".to_string(),
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );

    // update collateral query
    let msg = ExecuteMsg::UpdateCollateralPriceSource {
        asset: AssetInfo::Token {
            contract_addr: "mTSLA".to_string(),
        },
        price_source: SourceType::FixedPrice {
            price: Decimal::zero(),
        },
    };

    // unauthorized attempt
    let info = mock_info("factory0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));

    // successfull attempt
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // query the updated collateral
    let query_res = query_collateral_info(deps.as_ref(), "mTSLA".to_string()).unwrap();
    assert_eq!(
        query_res,
        CollateralInfoResponse {
            asset: "mTSLA".to_string(),
            source_type: "fixed_price".to_string(),
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );

    // update collateral premium - invalid msg
    let msg = ExecuteMsg::UpdateCollateralMultiplier {
        asset: AssetInfo::Token {
            contract_addr: "mTSLA".to_string(),
        },
        multiplier: Decimal::zero(),
    };

    // invalid multiplier
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("Multiplier must be bigger than 0")
    );

    // update collateral premium - valid msg
    let msg = ExecuteMsg::UpdateCollateralMultiplier {
        asset: AssetInfo::Token {
            contract_addr: "mTSLA".to_string(),
        },
        multiplier: Decimal::percent(120),
    };

    // unauthorized attempt
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));

    // successfull attempt
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // query the updated collateral
    let query_res = query_collateral_info(deps.as_ref(), "mTSLA".to_string()).unwrap();
    assert_eq!(
        query_res,
        CollateralInfoResponse {
            asset: "mTSLA".to_string(),
            source_type: "fixed_price".to_string(),
            multiplier: Decimal::percent(120),
            is_revoked: false,
        }
    )
}

#[test]
fn get_oracle_price() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_oracle_price(&[
        (&"uusd".to_string(), &Decimal::one()),
        (&"mTSLA".to_string(), &Decimal::percent(100)),
    ]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "mTSLA".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::TefiOracle {
            oracle_addr: "mirrorOracle0000".to_string(),
        },
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    // attempt to query price
    let query_res =
        query_collateral_price(deps.as_ref(), mock_env(), "mTSLA".to_string(), None).unwrap();
    assert_eq!(
        query_res,
        CollateralPriceResponse {
            asset: "mTSLA".to_string(),
            rate: Decimal::percent(100),
            last_updated: 1000u64,
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );
}

#[test]
fn get_amm_pair_price() {
    let mut deps = mock_dependencies(&[]);
    deps.querier.with_terraswap_pools(&[
        (
            &"ustancpair0000".to_string(),
            (
                &"uusd".to_string(),
                &Uint128::from(1u128),
                &"anc0000".to_string(),
                &Uint128::from(100u128),
            ),
        ),
        (
            &"lunablunapair0000".to_string(),
            (
                &"uluna".to_string(),
                &Uint128::from(18u128),
                &"bluna0000".to_string(),
                &Uint128::from(2u128),
            ),
        ),
    ]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "anc0000".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::AmmPair {
            pair_addr: "ustancpair0000".to_string(),
            intermediate_denom: None,
        },
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // attempt to query price
    let query_res =
        query_collateral_price(deps.as_ref(), mock_env(), "anc0000".to_string(), None).unwrap();
    assert_eq!(
        query_res,
        CollateralPriceResponse {
            asset: "anc0000".to_string(),
            rate: Decimal::from_ratio(1u128, 100u128),
            last_updated: u64::MAX,
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );

    // register collateral with intermediate denom
    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "bluna0000".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::AmmPair {
            pair_addr: "lunablunapair0000".to_string(),
            intermediate_denom: Some("uluna".to_string()),
        },
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // attempt to query price
    let query_res =
        query_collateral_price(deps.as_ref(), mock_env(), "bluna0000".to_string(), None).unwrap();
    assert_eq!(
        query_res,
        CollateralPriceResponse {
            asset: "bluna0000".to_string(),
            rate: Decimal::from_ratio(45u128, 1u128), // 9 / 1 * 5 / 1
            last_updated: u64::MAX,
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );
}

#[test]
fn get_fixed_price() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "aUST".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::FixedPrice {
            price: Decimal::from_ratio(1u128, 2u128),
        },
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // attempt to query price
    let query_res =
        query_collateral_price(deps.as_ref(), mock_env(), "aUST".to_string(), None).unwrap();
    assert_eq!(
        query_res,
        CollateralPriceResponse {
            asset: "aUST".to_string(),
            rate: Decimal::from_ratio(1u128, 2u128),
            last_updated: u64::MAX,
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );
}

#[test]
fn get_anchor_market_price() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "aust0000".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::AnchorMarket {
            anchor_market_addr: "anchormarket0000".to_string(),
        },
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // attempt to query price
    let query_res =
        query_collateral_price(deps.as_ref(), mock_env(), "aust0000".to_string(), None).unwrap();
    assert_eq!(
        query_res,
        CollateralPriceResponse {
            asset: "aust0000".to_string(),
            rate: Decimal::from_ratio(10u128, 3u128),
            last_updated: u64::MAX,
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );
}

#[test]
fn get_lunax_price() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "lunax0000".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::Lunax {
            staking_contract_addr: "lunaxstaking0000".to_string(),
        },
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // attempt to query price
    let query_res =
        query_collateral_price(deps.as_ref(), mock_env(), "lunax0000".to_string(), None).unwrap();
    assert_eq!(
        query_res,
        CollateralPriceResponse {
            asset: "lunax0000".to_string(),
            rate: Decimal::from_ratio(55u128, 10u128), // exchange rate = 1.1 i.e 1 ulunax = 1.1 uluna and 1 uluna = 5 uusd
            last_updated: u64::MAX,
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );
}

#[test]
fn get_native_price() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::Native {
            native_denom: "uluna".to_string(),
        },
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // attempt to query price
    let query_res =
        query_collateral_price(deps.as_ref(), mock_env(), "uluna".to_string(), None).unwrap();
    assert_eq!(
        query_res,
        CollateralPriceResponse {
            asset: "uluna".to_string(),
            rate: Decimal::from_ratio(5u128, 1u128),
            last_updated: u64::MAX,
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );
}

#[test]
fn revoke_collateral() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        mint_contract: "mint0000".to_string(),
        base_denom: "uusd".to_string(),
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "aUST".to_string(),
        },
        multiplier: Decimal::percent(100),
        price_source: SourceType::FixedPrice {
            price: Decimal::one(),
        },
    };

    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // attempt to query price
    let query_res =
        query_collateral_price(deps.as_ref(), mock_env(), "aUST".to_string(), None).unwrap();
    assert_eq!(
        query_res,
        CollateralPriceResponse {
            asset: "aUST".to_string(),
            rate: Decimal::one(),
            last_updated: u64::MAX,
            multiplier: Decimal::percent(100),
            is_revoked: false,
        }
    );

    // revoke the asset
    let msg = ExecuteMsg::RevokeCollateralAsset {
        asset: AssetInfo::Token {
            contract_addr: "aUST".to_string(),
        },
    };

    // unauthorized attempt
    let info = mock_info("factory0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));

    // successfull attempt
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // query the revoked collateral
    let query_res = query_collateral_info(deps.as_ref(), "aUST".to_string()).unwrap();
    assert_eq!(
        query_res,
        CollateralInfoResponse {
            asset: "aUST".to_string(),
            source_type: "fixed_price".to_string(),
            multiplier: Decimal::percent(100),
            is_revoked: true,
        }
    );

    // attempt to query price of revoked asset
    let query_res =
        query_collateral_price(deps.as_ref(), mock_env(), "aUST".to_string(), None).unwrap();
    assert_eq!(
        query_res,
        CollateralPriceResponse {
            asset: "aUST".to_string(),
            rate: Decimal::one(),
            last_updated: u64::MAX,
            multiplier: Decimal::percent(100),
            is_revoked: true,
        }
    );
}
