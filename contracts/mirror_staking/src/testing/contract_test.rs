use crate::contract::{execute, instantiate, query};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{attr, from_binary, Addr, Decimal, StdError, Uint128};
use mirror_protocol::staking::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, PoolInfoResponse, QueryMsg,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        mirror_token: "reward".to_string(),
        mint_contract: "mint".to_string(),
        oracle_contract: "oracle".to_string(),
        terraswap_factory: "terraswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        premium_min_update_interval: 3600,
        short_reward_contract: "short_reward".to_string(),
    };

    let info = mock_info("addr", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        ConfigResponse {
            owner: "owner".to_string(),
            mirror_token: "reward".to_string(),
            mint_contract: "mint".to_string(),
            oracle_contract: "oracle".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            base_denom: "uusd".to_string(),
            premium_min_update_interval: 3600,
            short_reward_contract: Addr::unchecked("short_reward").to_string(),
        },
        config
    );
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        mirror_token: "reward".to_string(),
        mint_contract: "mint".to_string(),
        oracle_contract: "oracle".to_string(),
        terraswap_factory: "terraswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        premium_min_update_interval: 3600,
        short_reward_contract: Addr::unchecked("short_reward").to_string(),
    };

    let info = mock_info("addr", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // update owner
    let info = mock_info("owner", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("owner2".to_string()),
        premium_min_update_interval: Some(7200),
        short_reward_contract: Some(Addr::unchecked("new_short_reward").to_string()),
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        ConfigResponse {
            owner: "owner2".to_string(),
            mirror_token: "reward".to_string(),
            mint_contract: "mint".to_string(),
            oracle_contract: "oracle".to_string(),
            terraswap_factory: "terraswap_factory".to_string(),
            base_denom: "uusd".to_string(),
            premium_min_update_interval: 7200,
            short_reward_contract: Addr::unchecked("new_short_reward").to_string(),
        },
        config
    );

    // unauthorized err
    let info = mock_info("owner", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        premium_min_update_interval: Some(7200),
        short_reward_contract: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn test_register() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        mirror_token: "reward".to_string(),
        mint_contract: "mint".to_string(),
        oracle_contract: "oracle".to_string(),
        terraswap_factory: "terraswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        premium_min_update_interval: 3600,
        short_reward_contract: Addr::unchecked("short_reward").to_string(),
    };

    let info = mock_info("addr", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset".to_string(),
        staking_token: "staking".to_string(),
    };

    // failed with unauthorized error
    let info = mock_info("addr", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info("owner", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "register_asset"),
            attr("asset_token", "asset"),
        ]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PoolInfo {
            asset_token: "asset".to_string(),
        },
    )
    .unwrap();
    let pool_info: PoolInfoResponse = from_binary(&res).unwrap();
    assert_eq!(
        pool_info,
        PoolInfoResponse {
            asset_token: "asset".to_string(),
            staking_token: "staking".to_string(),
            total_bond_amount: Uint128::zero(),
            total_short_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::zero(),
            short_reward_weight: Decimal::zero(),
            premium_updated_time: 0,
        }
    );
}

