use crate::asset::{Asset, AssetInfo};
use crate::mock_querier::mock_dependencies;
use crate::querier::{
    load_balance, load_liquidity_token, load_pair_contract, load_supply, load_token_balance,
};
use cosmwasm_std::testing::MOCK_CONTRACT_ADDR;
use cosmwasm_std::{to_binary, BankMsg, Coin, CosmosMsg, Decimal, HumanAddr, Uint128, WasmMsg};
use cw20::Cw20HandleMsg;

#[test]
fn token_balance_querier() {
    let mut deps = mock_dependencies(20, &[]);

    deps.querier.with_token_balances(&[(
        &HumanAddr::from("liquidity0000"),
        &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(123u128))],
    )]);

    assert_eq!(
        Uint128(123u128),
        load_token_balance(
            &deps,
            &HumanAddr::from("liquidity0000"),
            &HumanAddr::from(MOCK_CONTRACT_ADDR),
        )
        .unwrap()
    );
}

#[test]
fn balance_querier() {
    let deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(200u128),
        }],
    );

    assert_eq!(
        load_balance(
            &deps,
            &HumanAddr::from(MOCK_CONTRACT_ADDR),
            "uusd".to_string()
        )
        .unwrap(),
        Uint128(200u128)
    );
}

#[test]
fn supply_querier() {
    let mut deps = mock_dependencies(20, &[]);

    deps.querier.with_token_balances(&[(
        &HumanAddr::from("liquidity0000"),
        &[
            (&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(123u128)),
            (&HumanAddr::from("addr00000"), &Uint128(123u128)),
            (&HumanAddr::from("addr00001"), &Uint128(123u128)),
            (&HumanAddr::from("addr00002"), &Uint128(123u128)),
        ],
    )]);

    assert_eq!(
        load_supply(&deps, &HumanAddr::from("liquidity0000")).unwrap(),
        Uint128(492u128)
    )
}

#[test]
fn test_asset_info() {
    let token_info: AssetInfo = AssetInfo::Token {
        contract_addr: HumanAddr::from("asset0000"),
    };
    let native_token_info: AssetInfo = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    assert_eq!(false, token_info.equal(&native_token_info));

    assert_eq!(
        false,
        token_info.equal(&AssetInfo::Token {
            contract_addr: HumanAddr::from("asset0001"),
        })
    );

    assert_eq!(
        true,
        token_info.equal(&AssetInfo::Token {
            contract_addr: HumanAddr::from("asset0000"),
        })
    );

    assert_eq!(true, native_token_info.is_native_token());
    assert_eq!(false, token_info.is_native_token());

    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(123),
        }],
    );
    deps.querier.with_token_balances(&[(
        &HumanAddr::from("asset0000"),
        &[
            (&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(123u128)),
            (&HumanAddr::from("addr00000"), &Uint128(123u128)),
            (&HumanAddr::from("addr00001"), &Uint128(123u128)),
            (&HumanAddr::from("addr00002"), &Uint128(123u128)),
        ],
    )]);

    assert_eq!(
        token_info
            .load_pool(&deps, &HumanAddr::from(MOCK_CONTRACT_ADDR))
            .unwrap(),
        Uint128(123u128)
    );
    assert_eq!(
        native_token_info
            .load_pool(&deps, &HumanAddr::from(MOCK_CONTRACT_ADDR))
            .unwrap(),
        Uint128(123u128)
    );
}

#[test]
fn test_asset() {
    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(123),
        }],
    );

    deps.querier.with_token_balances(&[(
        &HumanAddr::from("asset0000"),
        &[
            (&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(123u128)),
            (&HumanAddr::from("addr00000"), &Uint128(123u128)),
            (&HumanAddr::from("addr00001"), &Uint128(123u128)),
            (&HumanAddr::from("addr00002"), &Uint128(123u128)),
        ],
    )]);

    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128(1000000u128))],
    );

    let token_asset = Asset {
        amount: Uint128(123123u128),
        info: AssetInfo::Token {
            contract_addr: HumanAddr::from("asset0000"),
        },
    };

    let native_token_asset = Asset {
        amount: Uint128(123123u128),
        info: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    };

    assert_eq!(token_asset.compute_tax(&deps).unwrap(), Uint128::zero());
    assert_eq!(
        native_token_asset.compute_tax(&deps).unwrap(),
        Uint128(1220u128)
    );

    assert_eq!(
        native_token_asset.deduct_tax(&deps).unwrap(),
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128(121903u128),
        }
    );

    assert_eq!(
        token_asset
            .into_msg(
                &deps,
                HumanAddr::from("asset0000"),
                HumanAddr::from("addr0000")
            )
            .unwrap(),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128(123123u128),
            })
            .unwrap(),
            send: vec![],
        })
    );

    assert_eq!(
        native_token_asset
            .into_msg(
                &deps,
                HumanAddr::from(MOCK_CONTRACT_ADDR),
                HumanAddr::from("addr0000")
            )
            .unwrap(),
        CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("addr0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128(121903u128),
            }]
        })
    );
}

#[test]
fn query_terraswap_pair_contract() {
    let mut deps = mock_dependencies(20, &[]);

    deps.querier.with_terraswap_pairs(&[(
        &"asset0000\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}uusd".to_string(),
        &HumanAddr::from("pair0000"),
    )]);

    let pair_contract = load_pair_contract(
        &deps,
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        &[
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        ],
    )
    .unwrap();

    assert_eq!(pair_contract, HumanAddr::from("pair0000"),);
}

#[test]
fn query_terraswap_lp_token() {
    let mut deps = mock_dependencies(20, &[]);
    deps.querier
        .with_terraswap_pair_lp_token(&[(&HumanAddr::from("pair0000"), &HumanAddr::from("LP0000"))]);

    let liquidity_token = load_liquidity_token(&deps, &HumanAddr::from("pair0000")).unwrap();
    assert_eq!(liquidity_token, HumanAddr::from("LP0000"));
}
