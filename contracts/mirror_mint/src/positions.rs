use cosmwasm_std::{
    log, to_binary, Api, CosmosMsg, Decimal, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    LogAttribute, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::{
    asserts::{
        assert_asset, assert_burn_period, assert_collateral, assert_migrated_asset,
        assert_mint_period, assert_revoked_collateral,
    },
    math::{decimal_division, decimal_subtraction, reverse_decimal, decimal_multiplication},
    querier::{load_asset_price, load_collateral_info},
    state::{
        create_position, is_short_position, read_asset_config, read_config, read_position,
        read_position_idx, read_positions, read_positions_with_asset_indexer,
        read_positions_with_user_indexer, remove_position, store_position, store_position_idx,
        store_short_position, AssetConfig, Config, Position,
    },
};

use cw20::Cw20HandleMsg;
use mirror_protocol::{
    common::OrderBy,
    lock::HandleMsg as LockHandleMsg,
    mint::{NextPositionIdxResponse, PositionResponse, PositionsResponse, ShortParams},
    staking::HandleMsg as StakingHandleMsg,
};
use terraswap::{
    asset::{Asset, AssetInfo, AssetInfoRaw, AssetRaw},
    pair::Cw20HookMsg as PairCw20HookMsg,
    querier::query_pair_info,
};

pub fn open_position<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    collateral: Asset,
    asset_info: AssetInfo,
    collateral_ratio: Decimal,
    short_params: Option<ShortParams>,
) -> StdResult<HandleResponse> {
    let config: Config = read_config(&deps.storage)?;
    if collateral.amount.is_zero() {
        return Err(StdError::generic_err("Wrong collateral"));
    }

    // assert the collateral is listed and has not been migrated/revoked
    let collateral_info_raw: AssetInfoRaw = collateral.info.to_raw(&deps)?;
    let collateral_oracle: HumanAddr = deps.api.human_address(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier) =
        assert_revoked_collateral(load_collateral_info(
            &deps,
            &collateral_oracle,
            &collateral_info_raw,
            Some(env.block.time),
        )?)?;

    // assert asset migrated
    let asset_info_raw: AssetInfoRaw = asset_info.to_raw(&deps)?;
    let asset_token_raw = match asset_info_raw.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    // for assets with limited minting period (preIPO assets), assert minting phase
    assert_mint_period(&env, &asset_config)?;

    if collateral_ratio < asset_config.min_collateral_ratio {
        return Err(StdError::generic_err(
            "Can not open a position with low collateral ratio than minimum",
        ));
    }

    let oracle: HumanAddr = deps.api.human_address(&config.oracle)?;
    let asset_price: Decimal =
        load_asset_price(&deps, &oracle, &asset_info_raw, Some(env.block.time))?;

    let asset_price_in_collateral_asset = decimal_division(collateral_price, asset_price);

    // Calculate effective cr
    let effective_cr = decimal_multiplication(collateral_ratio, collateral_multiplier);

    // Convert collateral to mint amount
    let mint_amount = collateral.amount * asset_price_in_collateral_asset * reverse_decimal(effective_cr);
    if mint_amount.is_zero() {
        return Err(StdError::generic_err("collateral is too small"));
    }

    let position_idx = read_position_idx(&deps.storage)?;
    let asset_info_raw = asset_info.to_raw(&deps)?;

    create_position(
        &mut deps.storage,
        position_idx,
        &Position {
            idx: position_idx,
            owner: deps.api.canonical_address(&sender)?,
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
    let asset_token = deps.api.human_address(&asset_config.token)?;
    let messages: Vec<CosmosMsg> = if let Some(short_params) = short_params {
        is_short = true;
        store_short_position(&mut deps.storage, position_idx)?;

        // need to sell the tokens to terraswap
        // load pair contract address
        let pair_info = query_pair_info(
            &deps,
            &deps.api.human_address(&config.terraswap_factory)?,
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
        let lock_contract = deps.api.human_address(&config.lock)?;
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Mint {
                    recipient: env.contract.address,
                    amount: mint_amount,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: pair_info.contract_addr,
                    amount: mint_amount,
                    msg: Some(to_binary(&PairCw20HookMsg::Swap {
                        belief_price: short_params.belief_price,
                        max_spread: short_params.max_spread,
                        to: Some(lock_contract.clone()),
                    })?),
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lock_contract,
                send: vec![],
                msg: to_binary(&LockHandleMsg::LockPositionFundsHook {
                    position_idx,
                    receiver: sender.clone(),
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.staking)?,
                send: vec![],
                msg: to_binary(&StakingHandleMsg::IncreaseShortToken {
                    asset_token,
                    staker_addr: sender,
                    amount: mint_amount,
                })?,
            }),
        ]
    } else {
        is_short = false;
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token,
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: sender,
                amount: mint_amount,
            })?,
        })]
    };

    store_position_idx(&mut deps.storage, position_idx + Uint128(1u128))?;
    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "open_position"),
            log("position_idx", position_idx.to_string()),
            log(
                "mint_amount",
                mint_amount.to_string() + &asset_info.to_string(),
            ),
            log("collateral_amount", collateral.to_string()),
            log("is_short", is_short),
        ],
        data: None,
    })
}

pub fn deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    sender: HumanAddr,
    position_idx: Uint128,
    collateral: Asset,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let mut position: Position = read_position(&deps.storage, position_idx)?;
    let position_owner = deps.api.human_address(&position.owner)?;
    if sender != position_owner {
        return Err(StdError::unauthorized());
    }

    // Check the given collateral has same asset info
    // with position's collateral token
    // also Check the collateral amount is non-zero
    assert_collateral(deps, &position, &collateral)?;

    // assert the collateral is listed and has not been migrated/revoked
    let collateral_oracle: HumanAddr = deps.api.human_address(&config.collateral_oracle)?;
    assert_revoked_collateral(load_collateral_info(
        &deps,
        &collateral_oracle,
        &position.collateral.info,
        None,
    )?)?;

    // assert asset migrated
    match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => {
            assert_migrated_asset(&read_asset_config(&deps.storage, &contract_addr)?)?
        }
        _ => panic!("DO NOT ENTER HERE"),
    };

    // Increase collateral amount
    position.collateral.amount += collateral.amount;
    store_position(&mut deps.storage, position_idx, &position)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "deposit"),
            log("position_idx", position_idx.to_string()),
            log("deposit_amount", collateral.to_string()),
        ],
        data: None,
    })
}

pub fn withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    position_idx: Uint128,
    collateral: Asset,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let mut position: Position = read_position(&deps.storage, position_idx)?;
    let position_owner = deps.api.human_address(&position.owner)?;
    if env.message.sender != position_owner {
        return Err(StdError::unauthorized());
    }

    // Check the given collateral has same asset info
    // with position's collateral token
    // also Check the collateral amount is non-zero
    assert_collateral(deps, &position, &collateral)?;

    if position.collateral.amount < collateral.amount {
        return Err(StdError::generic_err(
            "Cannot withdraw more than you provide",
        ));
    }

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    let oracle: HumanAddr = deps.api.human_address(&config.oracle)?;
    let asset_price: Decimal =
        load_asset_price(&deps, &oracle, &position.asset.info, Some(env.block.time))?;

    // Fetch collateral info from collateral oracle
    let collateral_oracle: HumanAddr = deps.api.human_address(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier, _collateral_is_revoked) = load_collateral_info(
        &deps,
        &collateral_oracle,
        &position.collateral.info,
        Some(env.block.time),
    )?;

    // Compute new collateral amount
    let collateral_amount: Uint128 = (position.collateral.amount - collateral.amount)?;

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
        if is_short_position(&deps.storage, position_idx)? {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.lock)?,
                send: vec![],
                msg: to_binary(&LockHandleMsg::ReleasePositionFunds { position_idx })?,
            }));
        }
        remove_position(&mut deps.storage, position_idx)?;
    } else {
        store_position(&mut deps.storage, position_idx, &position)?;
    }

    let mut collateral = collateral;

    // Deduct protocol fee
    let protocol_fee = Asset {
        info: collateral.info.clone(),
        amount: collateral.amount * config.protocol_fee_rate,
    };

    collateral.amount = (collateral.amount - protocol_fee.amount)?;

    // Compute tax amount
    let tax_amount = collateral.compute_tax(&deps)?;

    if !protocol_fee.amount.is_zero() {
        messages.push(protocol_fee.clone().into_msg(
            &deps,
            env.contract.address.clone(),
            deps.api.human_address(&config.collector)?,
        )?);
    }

    Ok(HandleResponse {
        messages: vec![
            vec![collateral.clone().into_msg(
                &deps,
                env.contract.address,
                position_owner,
            )?],
            messages,
        ]
        .concat(),
        log: vec![
            log("action", "withdraw"),
            log("position_idx", position_idx.to_string()),
            log("withdraw_amount", collateral.to_string()),
            log(
                "tax_amount",
                tax_amount.to_string() + &collateral.info.to_string(),
            ),
            log("protocol_fee", protocol_fee.to_string()),
        ],
        data: None,
    })
}

pub fn mint<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    position_idx: Uint128,
    asset: Asset,
    short_params: Option<ShortParams>,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let mint_amount = asset.amount;
    let sender = env.message.sender.clone();

    let mut position: Position = read_position(&deps.storage, position_idx)?;
    let position_owner = deps.api.human_address(&position.owner)?;
    if sender != position_owner {
        return Err(StdError::unauthorized());
    }

    assert_asset(&deps, &position, &asset)?;

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    // assert the asset migrated
    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    // assert the collateral is listed and has not been migrated/revoked
    let collateral_oracle: HumanAddr = deps.api.human_address(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier) =
        assert_revoked_collateral(load_collateral_info(
            &deps,
            &collateral_oracle,
            &position.collateral.info,
            Some(env.block.time),
        )?)?;

    // for assets with limited minting period (preIPO assets), assert minting phase
    assert_mint_period(&env, &asset_config)?;

    let oracle: HumanAddr = deps.api.human_address(&config.oracle)?;
    let asset_price: Decimal =
        load_asset_price(&deps, &oracle, &position.asset.info, Some(env.block.time))?;

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
    store_position(&mut deps.storage, position_idx, &position)?;

    let asset_token = deps.api.human_address(&asset_config.token)?;

    // If the position is flagged as short position.
    // immediately sell the token to terraswap
    // and execute staking short token
    let messages: Vec<CosmosMsg> = if is_short_position(&deps.storage, position_idx)? {
        // need to sell the tokens to terraswap
        // load pair contract address
        let pair_info = query_pair_info(
            &deps,
            &deps.api.human_address(&config.terraswap_factory)?,
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
        let lock_contract = deps.api.human_address(&config.lock)?;
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Mint {
                    recipient: env.contract.address,
                    amount: mint_amount,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: pair_info.contract_addr,
                    amount: mint_amount,
                    msg: Some(to_binary(
                        &(if let Some(short_params) = short_params {
                            PairCw20HookMsg::Swap {
                                belief_price: short_params.belief_price,
                                max_spread: short_params.max_spread,
                                to: Some(lock_contract.clone()),
                            }
                        } else {
                            PairCw20HookMsg::Swap {
                                belief_price: None,
                                max_spread: None,
                                to: Some(lock_contract.clone()),
                            }
                        }),
                    )?),
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: lock_contract,
                send: vec![],
                msg: to_binary(&LockHandleMsg::LockPositionFundsHook {
                    position_idx,
                    receiver: position_owner.clone(),
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.staking)?,
                send: vec![],
                msg: to_binary(&StakingHandleMsg::IncreaseShortToken {
                    asset_token,
                    staker_addr: position_owner,
                    amount: mint_amount,
                })?,
            }),
        ]
    } else {
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&asset_config.token)?,
            msg: to_binary(&Cw20HandleMsg::Mint {
                amount: mint_amount,
                recipient: position_owner,
            })?,
            send: vec![],
        })]
    };

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "mint"),
            log("position_idx", position_idx.to_string()),
            log("mint_amount", asset.to_string()),
        ],
        data: None,
    })
}

pub fn burn<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    position_idx: Uint128,
    asset: Asset,
) -> StdResult<HandleResponse> {
    let burn_amount = asset.amount;

    let config: Config = read_config(&deps.storage)?;
    let mut position: Position = read_position(&deps.storage, position_idx)?;
    let position_owner = deps.api.human_address(&position.owner)?;
    let collateral_info: AssetInfo = position.collateral.info.to_normal(&deps)?;

    // Check the asset has same token with position asset
    // also Check burn amount is non-zero
    assert_asset(deps, &position, &asset)?;

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    if position.asset.amount < burn_amount {
        return Err(StdError::generic_err(
            "Cannot burn asset more than you mint",
        ));
    }

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut logs: Vec<LogAttribute> = vec![];

    // If mint_end is different than None, it is a pre-IPO asset.
    // In that case, burning should be disabled after the minting period is over.
    // Burning is enabled again after asset migration (IPO event), when mint_end is reset to None
    assert_burn_period(&env, &asset_config)?;

    // Check if it is a short position
    let is_short_position: bool = is_short_position(&deps.storage, position_idx)?;

    // If the collateral is default denom asset and the asset is deprecated,
    // anyone can execute burn the asset to any position without permission
    let mut close_position: bool = false;
    if asset_config.end_price.is_some() {
        let oracle: HumanAddr = deps.api.human_address(&config.oracle)?;
        let asset_price: Decimal =
            load_asset_price(&deps, &oracle, &position.asset.info, Some(env.block.time))?;

        // fetch collateral info from collateral oracle
        let collateral_oracle: HumanAddr = deps.api.human_address(&config.collateral_oracle)?;
        let (collateral_price, _collateral_multiplier, _collateral_is_revoked) =
            load_collateral_info(
                &deps,
                &collateral_oracle,
                &position.collateral.info,
                Some(env.block.time),
            )?;

        let collateral_price_in_asset = decimal_division(asset_price, collateral_price);

        // Burn deprecated asset to receive collaterals back
        let conversion_rate =
            Decimal::from_ratio(position.collateral.amount, position.asset.amount);
        let refund_collateral = Asset {
            info: collateral_info,
            amount: std::cmp::min(
                burn_amount * collateral_price_in_asset,
                burn_amount * conversion_rate,
            ),
        };

        position.asset.amount = (position.asset.amount - burn_amount).unwrap();
        position.collateral.amount =
            (position.collateral.amount - refund_collateral.amount).unwrap();

        // due to rounding, include 1
        if position.collateral.amount <= Uint128(1u128) && position.asset.amount == Uint128::zero()
        {
            close_position = true;
            remove_position(&mut deps.storage, position_idx)?;
        } else {
            store_position(&mut deps.storage, position_idx, &position)?;
        }

        // Refund collateral msg
        messages.push(
            refund_collateral
                .clone()
                .into_msg(&deps, env.contract.address, sender)?,
        );

        logs.push(log(
            "refund_collateral_amount",
            refund_collateral.to_string(),
        ));
    } else {
        if sender != position_owner {
            return Err(StdError::unauthorized());
        }

        // Update asset amount
        position.asset.amount = (position.asset.amount - burn_amount).unwrap();
        store_position(&mut deps.storage, position_idx, &position)?;
    }

    // If the position is flagged as short position.
    // decrease short token amount from the staking contract
    let asset_token = deps.api.human_address(&asset_config.token)?;
    if is_short_position {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.staking)?,
            msg: to_binary(&StakingHandleMsg::DecreaseShortToken {
                asset_token: asset_token.clone(),
                staker_addr: position_owner,
                amount: burn_amount,
            })?,
            send: vec![],
        }));
        if close_position {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.lock)?,
                send: vec![],
                msg: to_binary(&LockHandleMsg::ReleasePositionFunds { position_idx })?,
            }));
        }
    }

    Ok(HandleResponse {
        messages: vec![
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token,
                msg: to_binary(&Cw20HandleMsg::Burn {
                    amount: burn_amount,
                })?,
                send: vec![],
            })],
            messages,
        ]
        .concat(),
        log: vec![
            vec![
                log("action", "burn"),
                log("position_idx", position_idx.to_string()),
                log("burn_amount", asset.to_string()),
            ],
            logs,
        ]
        .concat(),
        data: None,
    })
}

pub fn auction<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    position_idx: Uint128,
    asset: Asset,
) -> StdResult<HandleResponse> {
    let config: Config = read_config(&deps.storage)?;
    let mut position: Position = read_position(&deps.storage, position_idx)?;
    let position_owner = deps.api.human_address(&position.owner)?;
    assert_asset(&deps, &position, &asset)?;

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    let asset_token = deps.api.human_address(&asset_config.token)?;
    let collateral_info = position.collateral.info.to_normal(&deps)?;

    if asset.amount > position.asset.amount {
        return Err(StdError::generic_err(
            "Cannot liquidate more than the position amount".to_string(),
        ));
    }

    let oracle: HumanAddr = deps.api.human_address(&config.oracle)?;
    let asset_price: Decimal =
        load_asset_price(&deps, &oracle, &position.asset.info, Some(env.block.time))?;

    // fetch collateral info from collateral oracle
    let collateral_oracle: HumanAddr = deps.api.human_address(&config.collateral_oracle)?;
    let (collateral_price, collateral_multiplier, _collateral_is_revoked) = load_collateral_info(
        &deps,
        &collateral_oracle,
        &position.collateral.info,
        Some(env.block.time),
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
    // left asset amount will be refunds to executor.
    let (return_collateral_amount, refund_asset_amount) =
        if asset_value_in_collateral_asset > position.collateral.amount {
            // refunds left asset to position liquidator
            let refund_asset_amount =
                (asset_value_in_collateral_asset - position.collateral.amount).unwrap()
                    * reverse_decimal(discounted_price);

            let refund_asset: Asset = Asset {
                info: asset.info.clone(),
                amount: refund_asset_amount,
            };

            messages.push(refund_asset.into_msg(
                &deps,
                env.contract.address.clone(),
                sender.clone(),
            )?);

            (position.collateral.amount, refund_asset_amount)
        } else {
            (asset_value_in_collateral_asset, Uint128::zero())
        };

    let liquidated_asset_amount = (asset.amount - refund_asset_amount).unwrap();
    let left_asset_amount = (position.asset.amount - liquidated_asset_amount).unwrap();
    let left_collateral_amount = (position.collateral.amount - return_collateral_amount).unwrap();

    // Check if it is a short position
    let is_short_position: bool = is_short_position(&deps.storage, position_idx)?;

    let mut close_position: bool = false;
    if left_collateral_amount.is_zero() {
        // all collaterals are sold out
        close_position = true;
        remove_position(&mut deps.storage, position_idx)?;
    } else if left_asset_amount.is_zero() {
        // all assets are paid
        close_position = true;
        remove_position(&mut deps.storage, position_idx)?;

        // refunds left collaterals to position owner
        let refund_collateral: Asset = Asset {
            info: collateral_info.clone(),
            amount: left_collateral_amount,
        };

        messages.push(refund_collateral.into_msg(
            &deps,
            env.contract.address.clone(),
            position_owner.clone(),
        )?);
    } else {
        position.collateral.amount = left_collateral_amount;
        position.asset.amount = left_asset_amount;

        store_position(&mut deps.storage, position_idx, &position)?;
    }

    // token burn message
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: asset_token.clone(),
        msg: to_binary(&Cw20HandleMsg::Burn {
            amount: liquidated_asset_amount,
        })?,
        send: vec![],
    }));

    // Deduct protocol fee
    let protocol_fee = return_collateral_amount * config.protocol_fee_rate;
    let return_collateral_amount = (return_collateral_amount - protocol_fee).unwrap();

    // return collateral to liquidation initiator(sender)
    let return_collateral_asset = Asset {
        info: collateral_info.clone(),
        amount: return_collateral_amount,
    };
    let tax_amount = return_collateral_asset.compute_tax(&deps)?;
    messages.push(return_collateral_asset.into_msg(&deps, env.contract.address.clone(), sender)?);

    // protocol fee sent to collector
    let protocol_fee_asset = Asset {
        info: collateral_info.clone(),
        amount: protocol_fee,
    };

    if !protocol_fee_asset.amount.is_zero() {
        messages.push(protocol_fee_asset.into_msg(
            &deps,
            env.contract.address,
            deps.api.human_address(&config.collector)?,
        )?);
    }

    // If the position is flagged as short position.
    // decrease short token amount from the staking contract
    if is_short_position {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.staking)?,
            msg: to_binary(&StakingHandleMsg::DecreaseShortToken {
                asset_token: asset_token,
                staker_addr: position_owner.clone(),
                amount: liquidated_asset_amount,
            })?,
            send: vec![],
        }));
        if close_position {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.lock)?,
                send: vec![],
                msg: to_binary(&LockHandleMsg::ReleasePositionFunds { position_idx })?,
            }));
        }
    }

    let collateral_info_str = collateral_info.to_string();
    let asset_info_str = asset.info.to_string();
    Ok(HandleResponse {
        log: vec![
            log("action", "auction"),
            log("position_idx", position_idx.to_string()),
            log("owner", position_owner.as_str()),
            log(
                "return_collateral_amount",
                return_collateral_amount.to_string() + &collateral_info_str,
            ),
            log(
                "liquidated_amount",
                liquidated_asset_amount.to_string() + &asset_info_str,
            ),
            log("tax_amount", tax_amount.to_string() + &collateral_info_str),
            log(
                "protocol_fee",
                protocol_fee.to_string() + &collateral_info_str,
            ),
        ],
        messages,
        data: None,
    })
}

pub fn query_position<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    position_idx: Uint128,
) -> StdResult<PositionResponse> {
    let position: Position = read_position(&deps.storage, position_idx)?;
    let resp = PositionResponse {
        idx: position.idx,
        owner: deps.api.human_address(&position.owner)?,
        collateral: position.collateral.to_normal(&deps)?,
        asset: position.asset.to_normal(&deps)?,
        is_short: is_short_position(&deps.storage, position.idx)?,
    };

    Ok(resp)
}

pub fn query_positions<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    owner_addr: Option<HumanAddr>,
    asset_token: Option<HumanAddr>,
    start_after: Option<Uint128>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<PositionsResponse> {
    let positions: Vec<Position> = if let Some(owner_addr) = owner_addr {
        read_positions_with_user_indexer(
            &deps.storage,
            &deps.api.canonical_address(&owner_addr)?,
            start_after,
            limit,
            order_by,
        )?
    } else if let Some(asset_token) = asset_token {
        read_positions_with_asset_indexer(
            &deps.storage,
            &deps.api.canonical_address(&asset_token)?,
            start_after,
            limit,
            order_by,
        )?
    } else {
        read_positions(&deps.storage, start_after, limit, order_by)?
    };

    let position_responses: StdResult<Vec<PositionResponse>> = positions
        .iter()
        .map(|position| {
            Ok(PositionResponse {
                idx: position.idx,
                owner: deps.api.human_address(&position.owner)?,
                collateral: position.collateral.to_normal(&deps)?,
                asset: position.asset.to_normal(&deps)?,
                is_short: is_short_position(&deps.storage, position.idx)?,
            })
        })
        .collect();

    Ok(PositionsResponse {
        positions: position_responses?,
    })
}

pub fn query_next_position_idx<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<NextPositionIdxResponse> {
    let idx = read_position_idx(&deps.storage)?;
    let resp = NextPositionIdxResponse {
        next_position_idx: idx,
    };

    Ok(resp)
}
