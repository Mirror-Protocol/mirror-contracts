#[cfg(test)]
mod tests {
    use crate::contract::{handle, init, query};
    use crate::mock_querier::mock_dependencies;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{from_binary, Decimal, HumanAddr, StdError};
    use mirror_protocol::mint::{
        AssetConfigResponse, ConfigResponse, HandleMsg, InitMsg, QueryMsg,
    };

    static TOKEN_CODE_ID: u64 = 10u64;

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);
        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            staking: HumanAddr::from("staking0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };
        let env = mock_env("addr0000", &[]);
        // we can just call .unwrap() to assert this was a success
        let _res = init(&mut deps, env.clone(), msg).unwrap();
        // it worked, let's query the state
        let res = query(&deps, QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("owner0000", config.owner.as_str());
        assert_eq!("uusd", config.base_denom.to_string());
        assert_eq!("oracle0000", config.oracle.as_str());
        assert_eq!("staking0000", config.staking.as_str());
        assert_eq!("collector0000", config.collector.as_str());
        assert_eq!("terraswap_factory", config.terraswap_factory.as_str());
        assert_eq!(TOKEN_CODE_ID, config.token_code_id);
        assert_eq!(Decimal::percent(1), config.protocol_fee_rate);
    }
    #[test]
    fn update_config() {
        let mut deps = mock_dependencies(20, &[]);
        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            staking: HumanAddr::from("staking0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: "uusd".to_string(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };
        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env.clone(), msg).unwrap();
        // update owner
        let env = mock_env("owner0000", &[]);
        let msg = HandleMsg::UpdateConfig {
            owner: Some(HumanAddr("owner0001".to_string())),
            oracle: None,
            collector: None,
            terraswap_factory: None,
            token_code_id: Some(100u64),
            protocol_fee_rate: None,
        };
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());
        // it worked, let's query the state
        let res = query(&deps, QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("owner0001", config.owner.as_str());
        assert_eq!(100u64, config.token_code_id);
        // Unauthorized err
        let env = mock_env("owner0000", &[]);
        let msg = HandleMsg::UpdateConfig {
            owner: None,
            oracle: None,
            collector: None,
            terraswap_factory: None,
            token_code_id: None,
            protocol_fee_rate: None,
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
        let base_denom = "uusd".to_string();
        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            staking: HumanAddr::from("staking0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: base_denom.clone(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };
        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(res.messages, vec![],);
        let res = query(
            &deps,
            QueryMsg::AssetConfig {
                asset_token: HumanAddr::from("asset0000"),
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
                end_price: None,
                mint_end: None,
                min_collateral_ratio_after_migration: None,
            }
        );
        // must be failed with the already registered token error
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            mint_end: None,
            min_collateral_ratio_after_migration: None,
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
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0001", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::Unauthorized { .. } => {}
            _ => panic!("DO NOT ENTER HERE"),
        }
        // must be failed with unauthorized error
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(150),
            min_collateral_ratio: Decimal::percent(150),
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "auction_discount must be smaller than 1")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }
        // must be failed with unauthorized error
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(50),
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "min_collateral_ratio must be bigger than 1")
            }
            _ => panic!("DO NOT ENTER HERE"),
        }
    }
    #[test]
    fn update_asset() {
        let mut deps = mock_dependencies(20, &[]);
        let base_denom = "uusd".to_string();
        let msg = InitMsg {
            owner: HumanAddr::from("owner0000"),
            oracle: HumanAddr::from("oracle0000"),
            collector: HumanAddr::from("collector0000"),
            staking: HumanAddr::from("staking0000"),
            terraswap_factory: HumanAddr::from("terraswap_factory"),
            base_denom: base_denom.clone(),
            token_code_id: TOKEN_CODE_ID,
            protocol_fee_rate: Decimal::percent(1),
        };
        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();
        let msg = HandleMsg::RegisterAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Decimal::percent(20),
            min_collateral_ratio: Decimal::percent(150),
            mint_end: None,
            min_collateral_ratio_after_migration: None,
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();
        let msg = HandleMsg::UpdateAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Some(Decimal::percent(30)),
            min_collateral_ratio: Some(Decimal::percent(200)),
        };
        let env = mock_env("owner0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();
        let res = query(
            &deps,
            QueryMsg::AssetConfig {
                asset_token: HumanAddr::from("asset0000"),
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
                end_price: None,
                mint_end: None,
                min_collateral_ratio_after_migration: None,
            }
        );
        let msg = HandleMsg::UpdateAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Some(Decimal::percent(130)),
            min_collateral_ratio: Some(Decimal::percent(150)),
        };
        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "auction_discount must be smaller than 1")
            }
            _ => panic!("Must return unauthorized error"),
        }
        let msg = HandleMsg::UpdateAsset {
            asset_token: HumanAddr::from("asset0000"),
            auction_discount: Some(Decimal::percent(30)),
            min_collateral_ratio: Some(Decimal::percent(50)),
        };
        let env = mock_env("owner0000", &[]);
        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => {
                assert_eq!(msg, "min_collateral_ratio must be bigger than 1")
            }
            _ => panic!("Must return unauthorized error"),
        }
        let msg = HandleMsg::UpdateAsset {
            asset_token: HumanAddr::from("asset0000"),
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
}
