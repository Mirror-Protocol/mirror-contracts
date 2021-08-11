use cosmwasm_std::{
    attr, to_binary, Addr, Attribute, CosmosMsg, Decimal, Deps, DepsMut, Env, Response, StdError,
    StdResult, Uint128, WasmMsg,
};

use crate::{
    asserts::{
        assert_asset, assert_burn_period, assert_collateral, assert_migrated_asset,
        assert_mint_period, assert_pre_ipo_collateral, assert_revoked_collateral,
    },
    math::{decimal_division, decimal_multiplication, decimal_subtraction, reverse_decimal},
    querier::{load_asset_price, load_collateral_info},
    state::{
        create_position, is_short_position, read_asset_config, read_config, read_position,
        read_position_idx, read_positions, read_positions_with_asset_indexer,
        read_positions_with_user_indexer, remove_position, store_position, store_position_idx,
        store_short_position, AssetConfig, Config, Position,
    },
};

use cw20::Cw20ExecuteMsg;
use mirror_protocol::{
    common::OrderBy,
    lock::ExecuteMsg as LockExecuteMsg,
    mint::{NextPositionIdxResponse, PositionResponse, PositionsResponse, ShortParams},
    staking::ExecuteMsg as StakingExecuteMsg,
};
use terraswap::{
    asset::{Asset, AssetInfo, AssetInfoRaw, AssetRaw},
    pair::Cw20HookMsg as PairCw20HookMsg,
    querier::query_pair_info,
};

pub fn open_position(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    collateral: Asset,
    asset_info: AssetInfo,
    collateral_ratio: Decimal,
    short_params: Option<ShortParams>,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    if collateral.amount.is_zero() {
        return Err(StdError::generic_err("Wrong collateral"));
    }

    // assert the collateral is listed and has not been migrated/revoked
    let collateral_info_raw: AssetInfoRaw = collateral.info.to_raw(deps.api)?;
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier) =
        assert_revoked_collateral(load_collateral_info(
            deps.as_ref(),
            collateral_oracle,
            &collateral_info_raw,
            Some(env.block.time.nanos() / 1_000_000_000),
            Some(env.block.height),
        )?)?;

    // assert asset migrated
    let asset_info_raw: AssetInfoRaw = asset_info.to_raw(deps.api)?;
    let asset_token_raw = match asset_info_raw.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    // for assets with limited minting period (preIPO assets), assert minting phase as well as pre-ipo collateral
    assert_mint_period(&env, &asset_config)?;
    assert_pre_ipo_collateral(config.base_denom.clone(), &asset_config, &collateral.info)?;

    if collateral_ratio
        < decimal_multiplication(asset_config.min_collateral_ratio, collateral_multiplier)
    {
        return Err(StdError::generic_err(
            "Can not open a position with low collateral ratio than minimum",
        ));
    }

    let oracle: Addr = deps.api.addr_humanize(&config.oracle)?;
    let asset_price: Decimal = load_asset_price(
        deps.as_ref(),
        oracle,
        &asset_info_raw,
        Some(env.block.time.nanos() / 1_000_000_000),
    )?;

    let asset_price_in_collateral_asset = decimal_division(collateral_price, asset_price);

    // Convert collateral to mint amount
    let mint_amount =
        collateral.amount * asset_price_in_collateral_asset * reverse_decimal(collateral_ratio);
    if mint_amount.is_zero() {
        return Err(StdError::generic_err("collateral is too small"));
    }

    let position_idx = read_position_idx(deps.storage)?;
    let asset_info_raw = asset_info.to_raw(deps.api)?;

    create_position(
        deps.storage,
        position_idx,
        &Position {
            idx: position_idx,
            owner: deps.api.addr_canonicalize(sender.as_str())?,
            collateral: AssetRaw {
                amount: collateral.amount,
                info: collateral_info_raw,
            },
            asset: AssetRaw {
                amount: mint_amount,
                info: asset_info_raw,
            },
        },
    )?;

    // If the short_params exists, the position is
    // flagged as short position. so if want to make short position,
    // the one must pass at least empty {} as short_params
    let is_short: bool;
    let asset_token = deps.api.addr_humanize(&asset_config.token)?.to_string();
    let messages: Vec<CosmosMsg> = if let Some(short_params) = short_params {
        is_short = true;
        store_short_position(deps.storage, position_idx)?;

        // need to sell the tokens to terraswap
        // load pair contract address
        let pair_info = query_pair_info(
            &deps.querier,
            deps.api.addr_humanize(&config.terraswap_factory)?,
            &[
                AssetInfo::NativeToken {
                    denom: config.base_denom,
                },
                asset_info.clone(),
            ],
        )?;

        // 1. Mint token to itself
        // 2. Swap token and send to lock contract
        // 3. Call lock hook on lock contract
        // 4. Increase short token in staking contract
        let lock_contract = deps.api.addr_humanize(&config.lock)?.to_string();
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: env.contract.address.to_string(),
                    amount: mint_amount,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: pair_info.contract_addr.to_string(),
                    amount: mint_amount,
                    msg: to_binary(&PairCw20HookMsg::Swap {
                        belief_price: short_params.belief_price,
                        max_spread: short_params.max_spread,
                        to: Some(lock_contract.clone()),
                    })?,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lock_contract,
                funds: vec![],
                msg: to_binary(&LockExecuteMsg::LockPositionFundsHook {
                    position_idx,
                    receiver: sender.to_string(),
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.staking)?.to_string(),
                funds: vec![],
                msg: to_binary(&StakingExecuteMsg::IncreaseShortToken {
                    asset_token,
                    staker_addr: sender.to_string(),
                    amount: mint_amount,
                })?,
            }),
        ]
    } else {
        is_short = false;
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token,
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: sender.to_string(),
                amount: mint_amount,
            })?,
        })]
    };

    store_position_idx(deps.storage, position_idx + Uint128::from(1u128))?;
    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "open_position"),
            attr("position_idx", position_idx.to_string()),
            attr(
                "mint_amount",
                mint_amount.to_string() + &asset_info.to_string(),
            ),
            attr("collateral_amount", collateral.to_string()),
            attr("is_short", is_short.to_string()),
        ])
        .add_messages(messages))
}

pub fn deposit(
    deps: DepsMut,
    sender: Addr,
    position_idx: Uint128,
    collateral: Asset,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let mut position: Position = read_position(deps.storage, position_idx)?;
    let position_owner = deps.api.addr_humanize(&position.owner)?;
    if sender != position_owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    // Check the given collateral has same asset info
    // with position's collateral token
    // also Check the collateral amount is non-zero
    assert_collateral(deps.as_ref(), &position, &collateral)?;

    // assert the collateral is listed and has not been migrated/revoked
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    assert_revoked_collateral(load_collateral_info(
        deps.as_ref(),
        collateral_oracle,
        &position.collateral.info,
        None,
        None,
    )?)?;

    // assert asset migrated
    match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => {
            assert_migrated_asset(&read_asset_config(deps.storage, &contract_addr)?)?
        }
        _ => panic!("DO NOT ENTER HERE"),
    };

    // Increase collateral amount
    position.collateral.amount += collateral.amount;
    store_position(deps.storage, position_idx, &position)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "deposit"),
        attr("position_idx", position_idx.to_string()),
        attr("deposit_amount", collateral.to_string()),
    ]))
}

pub fn withdraw(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    position_idx: Uint128,
    collateral: Option<Asset>,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let mut position: Position = read_position(deps.storage, position_idx)?;
    let position_owner = deps.api.addr_humanize(&position.owner)?;
    if sender != position_owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    // if collateral is not provided, withraw all collateral
    let collateral: Asset = if let Some(collateral) = collateral {
        // Check the given collateral has same asset info
        // with position's collateral token
        // also Check the collateral amount is non-zero
        assert_collateral(deps.as_ref(), &position, &collateral)?;

        if position.collateral.amount < collateral.amount {
            return Err(StdError::generic_err(
                "Cannot withdraw more than you provide",
            ));
        }

        collateral
    } else {
        position.collateral.to_normal(deps.api)?
    };

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;
    let oracle: Addr = deps.api.addr_humanize(&config.oracle)?;
    let asset_price: Decimal = load_asset_price(
        deps.as_ref(),
        oracle,
        &position.asset.info,
        Some(env.block.time.nanos() / 1_000_000_000),
    )?;

    // Fetch collateral info from collateral oracle
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier, _collateral_is_revoked) = load_collateral_info(
        deps.as_ref(),
        collateral_oracle,
        &position.collateral.info,
        Some(env.block.time.nanos() / 1_000_000_000),
        Some(env.block.height),
    )?;

    // Compute new collateral amount
    let collateral_amount: Uint128 = position.collateral.amount.checked_sub(collateral.amount)?;

    // Convert asset to collateral unit
    let asset_value_in_collateral_asset: Uint128 =
        position.asset.amount * decimal_division(asset_price, collateral_price);

    // Check minimum collateral ratio is satisfied
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio * collateral_multiplier
        > collateral_amount
    {
        return Err(StdError::generic_err(
            "Cannot withdraw collateral over than minimum collateral ratio",
        ));
    }

    let mut messages: Vec<CosmosMsg> = vec![];

    position.collateral.amount = collateral_amount;
    if position.collateral.amount == Uint128::zero() && position.asset.amount == Uint128::zero() {
        // if it is a short position, release locked funds
        if is_short_position(deps.storage, position_idx)? {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.lock)?.to_string(),
                funds: vec![],
                msg: to_binary(&LockExecuteMsg::ReleasePositionFunds { position_idx })?,
            }));
        }
        remove_position(deps.storage, position_idx)?;
    } else {
        store_position(deps.storage, position_idx, &position)?;
    }

    // Compute tax amount
    let tax_amount = collateral.compute_tax(&deps.querier)?;

    Ok(Response::new()
        .add_messages(
            vec![
                vec![collateral.clone().into_msg(&deps.querier, position_owner)?],
                messages,
            ]
            .concat(),
        )
        .add_attributes(vec![
            attr("action", "withdraw"),
            attr("position_idx", position_idx.to_string()),
            attr("withdraw_amount", collateral.to_string()),
            attr(
                "tax_amount",
                tax_amount.to_string() + &collateral.info.to_string(),
            ),
        ]))
}

pub fn mint(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    position_idx: Uint128,
    asset: Asset,
    short_params: Option<ShortParams>,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let mint_amount = asset.amount;

    let mut position: Position = read_position(deps.storage, position_idx)?;
    let position_owner = deps.api.addr_humanize(&position.owner)?;
    if sender != position_owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    assert_asset(deps.as_ref(), &position, &asset)?;

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    // assert the asset migrated
    let asset_config: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    // assert the collateral is listed and has not been migrated/revoked
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier) =
        assert_revoked_collateral(load_collateral_info(
            deps.as_ref(),
            collateral_oracle,
            &position.collateral.info,
            Some(env.block.time.nanos() / 1_000_000_000),
            Some(env.block.height),
        )?)?;

    // for assets with limited minting period (preIPO assets), assert minting phase
    assert_mint_period(&env, &asset_config)?;

    let oracle: Addr = deps.api.addr_humanize(&config.oracle)?;
    let asset_price: Decimal = load_asset_price(
        deps.as_ref(),
        oracle,
        &position.asset.info,
        Some(env.block.time.nanos() / 1_000_000_000),
    )?;

    // Compute new asset amount
    let asset_amount: Uint128 = mint_amount + position.asset.amount;

    // Convert asset to collateral unit
    let asset_value_in_collateral_asset: Uint128 =
        asset_amount * decimal_division(asset_price, collateral_price);

    // Check minimum collateral ratio is satisfied
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio * collateral_multiplier
        > position.collateral.amount
    {
        return Err(StdError::generic_err(
            "Cannot mint asset over than min collateral ratio",
        ));
    }

    position.asset.amount += mint_amount;
    store_position(deps.storage, position_idx, &position)?;

    let asset_token = deps.api.addr_humanize(&asset_config.token)?;

    // If the position is flagged as short position.
    // immediately sell the token to terraswap
    // and execute staking short token
    let messages: Vec<CosmosMsg> = if is_short_position(deps.storage, position_idx)? {
        // need to sell the tokens to terraswap
        // load pair contract address
        let pair_info = query_pair_info(
            &deps.querier,
            deps.api.addr_humanize(&config.terraswap_factory)?,
            &[
                AssetInfo::NativeToken {
                    denom: config.base_denom,
                },
                asset.info.clone(),
            ],
        )?;

        // 1. Mint token to itself
        // 2. Swap token and send to lock contract
        // 3. Call lock hook on lock contract
        // 4. Increase short token in staking contract
        let lock_contract = deps.api.addr_humanize(&config.lock)?;
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Mint {
                    recipient: env.contract.address.to_string(),
                    amount: mint_amount,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: pair_info.contract_addr.to_string(),
                    amount: mint_amount,
                    msg: to_binary(
                        &(if let Some(short_params) = short_params {
                            PairCw20HookMsg::Swap {
                                belief_price: short_params.belief_price,
                                max_spread: short_params.max_spread,
                                to: Some(lock_contract.to_string()),
                            }
                        } else {
                            PairCw20HookMsg::Swap {
                                belief_price: None,
                                max_spread: None,
                                to: Some(lock_contract.to_string()),
                            }
                        }),
                    )?,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lock_contract.to_string(),
                funds: vec![],
                msg: to_binary(&LockExecuteMsg::LockPositionFundsHook {
                    position_idx,
                    receiver: position_owner.to_string(),
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.staking)?.to_string(),
                funds: vec![],
                msg: to_binary(&StakingExecuteMsg::IncreaseShortToken {
                    asset_token: asset_token.to_string(),
                    staker_addr: position_owner.to_string(),
                    amount: mint_amount,
                })?,
            }),
        ]
    } else {
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&asset_config.token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                amount: mint_amount,
                recipient: position_owner.to_string(),
            })?,
            funds: vec![],
        })]
    };

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "mint"),
            attr("position_idx", position_idx.to_string()),
            attr("mint_amount", asset.to_string()),
        ])
        .add_messages(messages))
}

pub fn burn(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    position_idx: Uint128,
    asset: Asset,
) -> StdResult<Response> {
    let burn_amount = asset.amount;

    let config: Config = read_config(deps.storage)?;
    let mut position: Position = read_position(deps.storage, position_idx)?;
    let position_owner = deps.api.addr_humanize(&position.owner)?;
    let collateral_info: AssetInfo = position.collateral.info.to_normal(deps.api)?;

    // Check the asset has same token with position asset
    // also Check burn amount is non-zero
    assert_asset(deps.as_ref(), &position, &asset)?;

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;
    if position.asset.amount < burn_amount {
        return Err(StdError::generic_err(
            "Cannot burn asset more than you mint",
        ));
    }

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut attributes: Vec<Attribute> = vec![];

    // For preIPO assets, burning should be disabled after the minting period is over.
    // Burning is enabled again after IPO event is triggered, when ipo_params are set to None
    assert_burn_period(&env, &asset_config)?;

    // Check if it is a short position
    let is_short_position: bool = is_short_position(deps.storage, position_idx)?;

    // fetch collateral info from collateral oracle
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    let (collateral_price, _collateral_multiplier, _collateral_is_revoked) = load_collateral_info(
        deps.as_ref(),
        collateral_oracle,
        &position.collateral.info,
        Some(env.block.time.nanos() / 1_000_000_000),
        Some(env.block.height),
    )?;

    // If the collateral is default denom asset and the asset is deprecated,
    // anyone can execute burn the asset to any position without permission
    let mut close_position: bool = false;

    if let Some(end_price) = asset_config.end_price {
        let asset_price: Decimal = end_price;

        let collateral_price_in_asset = decimal_division(asset_price, collateral_price);

        // Burn deprecated asset to receive collaterals back
        let conversion_rate =
            Decimal::from_ratio(position.collateral.amount, position.asset.amount);
        let mut refund_collateral = Asset {
            info: collateral_info.clone(),
            amount: std::cmp::min(
                burn_amount * collateral_price_in_asset,
                burn_amount * conversion_rate,
            ),
        };

        position.asset.amount = position.asset.amount.checked_sub(burn_amount).unwrap();
        position.collateral.amount = position
            .collateral
            .amount
            .checked_sub(refund_collateral.amount)
            .unwrap();

        // due to rounding, include 1
        if position.collateral.amount <= Uint128::from(1u128)
            && position.asset.amount == Uint128::zero()
        {
            close_position = true;
            remove_position(deps.storage, position_idx)?;
        } else {
            store_position(deps.storage, position_idx, &position)?;
        }

        // Subtract protocol fee from refunded collateral
        let protocol_fee = Asset {
            info: collateral_info,
            amount: burn_amount * collateral_price_in_asset * config.protocol_fee_rate,
        };

        if !protocol_fee.amount.is_zero() {
            messages.push(
                protocol_fee
                    .clone()
                    .into_msg(&deps.querier, deps.api.addr_humanize(&config.collector)?)?,
            );
            refund_collateral.amount = refund_collateral
                .amount
                .checked_sub(protocol_fee.amount)
                .unwrap();
        }
        attributes.push(attr("protocol_fee", protocol_fee.to_string()));

        // Refund collateral msg
        messages.push(refund_collateral.clone().into_msg(&deps.querier, sender)?);

        attributes.push(attr(
            "refund_collateral_amount",
            refund_collateral.to_string(),
        ));
    } else {
        if sender != position_owner {
            return Err(StdError::generic_err("unauthorized"));
        }
        let oracle = deps.api.addr_humanize(&config.oracle)?;
        let current_seconds = env.block.time.nanos() / 1_000_000_000u64;
        let asset_price: Decimal = load_asset_price(
            deps.as_ref(),
            oracle,
            &asset.info.to_raw(deps.api)?,
            Some(current_seconds),
        )?;
        let collateral_price_in_asset: Decimal = decimal_division(asset_price, collateral_price);

        // Subtract the protocol fee from the position's collateral
        let protocol_fee = Asset {
            info: collateral_info.clone(),
            amount: burn_amount * collateral_price_in_asset * config.protocol_fee_rate,
        };

        if !protocol_fee.amount.is_zero() {
            messages.push(
                protocol_fee
                    .clone()
                    .into_msg(&deps.querier, deps.api.addr_humanize(&config.collector)?)?,
            );
            position.collateral.amount = position
                .collateral
                .amount
                .checked_sub(protocol_fee.amount)?
        }
        attributes.push(attr("protocol_fee", protocol_fee.to_string()));

        // Update asset amount
        position.asset.amount = position.asset.amount.checked_sub(burn_amount).unwrap();
        store_position(deps.storage, position_idx, &position)?;
    }

    // If the position is flagged as short position.
    // decrease short token amount from the staking contract
    let asset_token = deps.api.addr_humanize(&asset_config.token)?;
    if is_short_position {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.staking)?.to_string(),
            msg: to_binary(&StakingExecuteMsg::DecreaseShortToken {
                asset_token: asset_token.to_string(),
                staker_addr: position_owner.to_string(),
                amount: burn_amount,
            })?,
            funds: vec![],
        }));
        if close_position {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.lock)?.to_string(),
                funds: vec![],
                msg: to_binary(&LockExecuteMsg::ReleasePositionFunds { position_idx })?,
            }));
        }
    }

    Ok(Response::new()
        .add_messages(
            vec![
                vec![CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: asset_token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Burn {
                        amount: burn_amount,
                    })?,
                    funds: vec![],
                })],
                messages,
            ]
            .concat(),
        )
        .add_attributes(
            vec![
                vec![
                    attr("action", "burn"),
                    attr("position_idx", position_idx.to_string()),
                    attr("burn_amount", asset.to_string()),
                ],
                attributes,
            ]
            .concat(),
        ))
}

pub fn auction(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    position_idx: Uint128,
    asset: Asset,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let mut position: Position = read_position(deps.storage, position_idx)?;
    let position_owner = deps.api.addr_humanize(&position.owner)?;
    assert_asset(deps.as_ref(), &position, &asset)?;

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    let asset_token = deps.api.addr_humanize(&asset_config.token)?;
    let collateral_info = position.collateral.info.to_normal(deps.api)?;

    if asset.amount > position.asset.amount {
        return Err(StdError::generic_err(
            "Cannot liquidate more than the position amount".to_string(),
        ));
    }

    let oracle: Addr = deps.api.addr_humanize(&config.oracle)?;
    let asset_price: Decimal = load_asset_price(
        deps.as_ref(),
        oracle,
        &position.asset.info,
        Some(env.block.time.nanos() / 1_000_000_000),
    )?;

    // fetch collateral info from collateral oracle
    let collateral_oracle: Addr = deps.api.addr_humanize(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier, _collateral_is_revoked) = load_collateral_info(
        deps.as_ref(),
        collateral_oracle,
        &position.collateral.info,
        Some(env.block.time.nanos() / 1_000_000_000),
        Some(env.block.height),
    )?;

    // Compute collateral price in asset unit
    let collateral_price_in_asset: Decimal = decimal_division(asset_price, collateral_price);

    // Check the position is in auction state
    // asset_amount * price_to_collateral * auction_threshold > collateral_amount
    let asset_value_in_collateral_asset: Uint128 =
        position.asset.amount * collateral_price_in_asset;
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio * collateral_multiplier
        < position.collateral.amount
    {
        return Err(StdError::generic_err(
            "Cannot liquidate a safely collateralized position",
        ));
    }

    // Compute discounted price
    let discounted_price: Decimal = decimal_division(
        collateral_price_in_asset,
        decimal_subtraction(Decimal::one(), asset_config.auction_discount),
    );

    // Convert asset value in discounted collateral unit
    let asset_value_in_collateral_asset: Uint128 = asset.amount * discounted_price;

    let mut messages: Vec<CosmosMsg> = vec![];

    // Cap return collateral amount to position collateral amount
    // If the given asset amount exceeds the amount required to liquidate position,
    // left asset amount will be refunded to the executor.
    let (return_collateral_amount, refund_asset_amount) =
        if asset_value_in_collateral_asset > position.collateral.amount {
            // refunds left asset to position liquidator
            let refund_asset_amount = asset_value_in_collateral_asset
                .checked_sub(position.collateral.amount)
                .unwrap()
                * reverse_decimal(discounted_price);

            let refund_asset: Asset = Asset {
                info: asset.info.clone(),
                amount: refund_asset_amount,
            };

            messages.push(refund_asset.into_msg(&deps.querier, sender.clone())?);

            (position.collateral.amount, refund_asset_amount)
        } else {
            (asset_value_in_collateral_asset, Uint128::zero())
        };

    let liquidated_asset_amount = asset.amount.checked_sub(refund_asset_amount).unwrap();
    let left_asset_amount = position
        .asset
        .amount
        .checked_sub(liquidated_asset_amount)
        .unwrap();
    let left_collateral_amount = position
        .collateral
        .amount
        .checked_sub(return_collateral_amount)
        .unwrap();

    // Check if it is a short position
    let is_short_position: bool = is_short_position(deps.storage, position_idx)?;

    let mut close_position: bool = false;
    if left_collateral_amount.is_zero() {
        // all collaterals are sold out
        close_position = true;
        remove_position(deps.storage, position_idx)?;
    } else if left_asset_amount.is_zero() {
        // all assets are paid
        close_position = true;
        remove_position(deps.storage, position_idx)?;

        // refunds left collaterals to position owner
        let refund_collateral: Asset = Asset {
            info: collateral_info.clone(),
            amount: left_collateral_amount,
        };

        messages.push(refund_collateral.into_msg(&deps.querier, position_owner.clone())?);
    } else {
        position.collateral.amount = left_collateral_amount;
        position.asset.amount = left_asset_amount;

        store_position(deps.storage, position_idx, &position)?;
    }

    // token burn message
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: asset_token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Burn {
            amount: liquidated_asset_amount,
        })?,
        funds: vec![],
    }));

    // Deduct protocol fee
    let protocol_fee =
        liquidated_asset_amount * collateral_price_in_asset * config.protocol_fee_rate;
    let return_collateral_amount = return_collateral_amount.checked_sub(protocol_fee).unwrap();

    // return collateral to liquidation initiator(sender)
    let return_collateral_asset = Asset {
        info: collateral_info.clone(),
        amount: return_collateral_amount,
    };
    let tax_amount = return_collateral_asset.compute_tax(&deps.querier)?;
    messages.push(return_collateral_asset.into_msg(&deps.querier, sender)?);

    // protocol fee sent to collector
    let protocol_fee_asset = Asset {
        info: collateral_info.clone(),
        amount: protocol_fee,
    };

    if !protocol_fee_asset.amount.is_zero() {
        messages.push(
            protocol_fee_asset
                .into_msg(&deps.querier, deps.api.addr_humanize(&config.collector)?)?,
        );
    }

    // If the position is flagged as short position.
    // decrease short token amount from the staking contract
    if is_short_position {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.staking)?.to_string(),
            msg: to_binary(&StakingExecuteMsg::DecreaseShortToken {
                asset_token: asset_token.to_string(),
                staker_addr: position_owner.to_string(),
                amount: liquidated_asset_amount,
            })?,
            funds: vec![],
        }));
        if close_position {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.addr_humanize(&config.lock)?.to_string(),
                funds: vec![],
                msg: to_binary(&LockExecuteMsg::ReleasePositionFunds { position_idx })?,
            }));
        }
    }

    let collateral_info_str = collateral_info.to_string();
    let asset_info_str = asset.info.to_string();
    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "auction"),
        attr("position_idx", position_idx.to_string()),
        attr("owner", position_owner.as_str()),
        attr(
            "return_collateral_amount",
            return_collateral_amount.to_string() + &collateral_info_str,
        ),
        attr(
            "liquidated_amount",
            liquidated_asset_amount.to_string() + &asset_info_str,
        ),
        attr("tax_amount", tax_amount.to_string() + &collateral_info_str),
        attr(
            "protocol_fee",
            protocol_fee.to_string() + &collateral_info_str,
        ),
    ]))
}

pub fn query_position(deps: Deps, position_idx: Uint128) -> StdResult<PositionResponse> {
    let position: Position = read_position(deps.storage, position_idx)?;
    let resp = PositionResponse {
        idx: position.idx,
        owner: deps.api.addr_humanize(&position.owner)?.to_string(),
        collateral: position.collateral.to_normal(deps.api)?,
        asset: position.asset.to_normal(deps.api)?,
        is_short: is_short_position(deps.storage, position.idx)?,
    };

    Ok(resp)
}

pub fn query_positions(
    deps: Deps,
    owner_addr: Option<String>,
    asset_token: Option<String>,
    start_after: Option<Uint128>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<PositionsResponse> {
    let positions: Vec<Position> = if let Some(owner_addr) = owner_addr {
        read_positions_with_user_indexer(
            deps.storage,
            &deps.api.addr_canonicalize(&owner_addr)?,
            start_after,
            limit,
            order_by,
        )?
    } else if let Some(asset_token) = asset_token {
        read_positions_with_asset_indexer(
            deps.storage,
            &deps.api.addr_canonicalize(&asset_token)?,
            start_after,
            limit,
            order_by,
        )?
    } else {
        read_positions(deps.storage, start_after, limit, order_by)?
    };

    let position_responses: StdResult<Vec<PositionResponse>> = positions
        .iter()
        .map(|position| {
            Ok(PositionResponse {
                idx: position.idx,
                owner: deps.api.addr_humanize(&position.owner)?.to_string(),
                collateral: position.collateral.to_normal(deps.api)?,
                asset: position.asset.to_normal(deps.api)?,
                is_short: is_short_position(deps.storage, position.idx)?,
            })
        })
        .collect();

    Ok(PositionsResponse {
        positions: position_responses?,
    })
}

pub fn query_next_position_idx(deps: Deps) -> StdResult<NextPositionIdxResponse> {
    let idx = read_position_idx(deps.storage)?;
    let resp = NextPositionIdxResponse {
        next_position_idx: idx,
    };

    Ok(resp)
}
