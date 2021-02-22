use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, InitResponse, LogAttribute, MigrateResponse, MigrateResult, Querier,
    StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::math::{decimal_division, decimal_subtraction, reverse_decimal};
use crate::querier::load_price;
use crate::state::{
    create_position, read_asset_config, read_config, read_position, read_position_idx,
    read_positions, read_positions_with_asset_indexer, read_positions_with_user_indexer,
    remove_position, store_asset_config, store_config, store_position, store_position_idx,
    AssetConfig, Config, Position,
};

use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use mirror_protocol::common::OrderBy;
use mirror_protocol::mint::{
    AssetConfigResponse, ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, MigrateMsg,
    PositionResponse, PositionsResponse, QueryMsg,
};
use terraswap::asset::{Asset, AssetInfo, AssetInfoRaw, AssetRaw};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let config = Config {
        owner: deps.api.canonical_address(&msg.owner)?,
        oracle: deps.api.canonical_address(&msg.oracle)?,
        collector: deps.api.canonical_address(&msg.collector)?,
        base_denom: msg.base_denom,
        token_code_id: msg.token_code_id,
        protocol_fee_rate: msg.protocol_fee_rate,
    };

    store_config(&mut deps.storage, &config)?;
    store_position_idx(&mut deps.storage, Uint128(1u128))?;
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Receive(msg) => receive_cw20(deps, env, msg),
        HandleMsg::UpdateConfig {
            owner,
            oracle,
            collector,
            token_code_id,
            protocol_fee_rate,
        } => try_update_config(
            deps,
            env,
            owner,
            oracle,
            collector,
            token_code_id,
            protocol_fee_rate,
        ),
        HandleMsg::UpdateAsset {
            asset_token,
            auction_discount,
            min_collateral_ratio,
        } => try_update_asset(
            deps,
            env,
            asset_token,
            auction_discount,
            min_collateral_ratio,
        ),
        HandleMsg::RegisterAsset {
            asset_token,
            auction_discount,
            min_collateral_ratio,
        } => try_register_asset(
            deps,
            env,
            asset_token,
            auction_discount,
            min_collateral_ratio,
        ),
        HandleMsg::RegisterMigration {
            asset_token,
            end_price,
        } => try_register_migration(deps, env, asset_token, end_price),
        HandleMsg::OpenPosition {
            collateral,
            asset_info,
            collateral_ratio,
        } => {
            // only native token can be deposited directly
            if !collateral.is_native_token() {
                return Err(StdError::unauthorized());
            }

            // Check the actual deposit happens
            collateral.assert_sent_native_token_balance(&env)?;

            try_open_position(
                deps,
                env.clone(),
                env.message.sender,
                collateral,
                asset_info,
                collateral_ratio,
            )
        }
        HandleMsg::Deposit {
            position_idx,
            collateral,
        } => {
            // only native token can be deposited directly
            if !collateral.is_native_token() {
                return Err(StdError::unauthorized());
            }

            // Check the actual deposit happens
            collateral.assert_sent_native_token_balance(&env)?;

            try_deposit(deps, env.message.sender, position_idx, collateral)
        }
        HandleMsg::Withdraw {
            position_idx,
            collateral,
        } => try_withdraw(deps, env, position_idx, collateral),
        HandleMsg::Mint {
            position_idx,
            asset,
        } => try_mint(deps, env, position_idx, asset),
    }
}

pub fn receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> HandleResult {
    let passed_asset: Asset = Asset {
        info: AssetInfo::Token {
            contract_addr: env.message.sender.clone(),
        },
        amount: cw20_msg.amount,
    };

    if let Some(msg) = cw20_msg.msg {
        match from_binary(&msg)? {
            Cw20HookMsg::OpenPosition {
                asset_info,
                collateral_ratio,
            } => try_open_position(
                deps,
                env,
                cw20_msg.sender,
                passed_asset,
                asset_info,
                collateral_ratio,
            ),
            Cw20HookMsg::Deposit { position_idx } => {
                try_deposit(deps, cw20_msg.sender, position_idx, passed_asset)
            }
            Cw20HookMsg::Burn { position_idx } => {
                try_burn(deps, env, cw20_msg.sender, position_idx, passed_asset)
            }
            Cw20HookMsg::Auction { position_idx } => {
                try_auction(deps, env, cw20_msg.sender, position_idx, passed_asset)
            }
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

pub fn try_update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    oracle: Option<HumanAddr>,
    collector: Option<HumanAddr>,
    token_code_id: Option<u64>,
    protocol_fee_rate: Option<Decimal>,
) -> StdResult<HandleResponse> {
    let mut config: Config = read_config(&deps.storage)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(oracle) = oracle {
        config.oracle = deps.api.canonical_address(&oracle)?;
    }

    if let Some(collector) = collector {
        config.collector = deps.api.canonical_address(&collector)?;
    }

    if let Some(token_code_id) = token_code_id {
        config.token_code_id = token_code_id;
    }

    if let Some(protocol_fee_rate) = protocol_fee_rate {
        config.protocol_fee_rate = protocol_fee_rate;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

pub fn try_update_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    auction_discount: Option<Decimal>,
    min_collateral_ratio: Option<Decimal>,
) -> StdResult<HandleResponse> {
    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let mut asset: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(auction_discount) = auction_discount {
        assert_auction_discount(auction_discount)?;
        asset.auction_discount = auction_discount;
    }

    if let Some(min_collateral_ratio) = min_collateral_ratio {
        assert_min_collateral_ratio(min_collateral_ratio)?;
        asset.min_collateral_ratio = min_collateral_ratio;
    }

    store_asset_config(&mut deps.storage, &asset_token_raw, &asset)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_asset")],
        data: None,
    })
}

pub fn try_register_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    auction_discount: Decimal,
    min_collateral_ratio: Decimal,
) -> StdResult<HandleResponse> {
    assert_auction_discount(auction_discount)?;
    assert_min_collateral_ratio(min_collateral_ratio)?;

    let config: Config = read_config(&deps.storage)?;

    // permission check
    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    if read_asset_config(&deps.storage, &asset_token_raw).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    // Store temp info into base asset store
    store_asset_config(
        &mut deps.storage,
        &asset_token_raw,
        &AssetConfig {
            token: deps.api.canonical_address(&asset_token)?,
            auction_discount,
            min_collateral_ratio,
            end_price: None,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "register"), log("asset_token", asset_token)],
        data: None,
    })
}

pub fn try_register_migration<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    end_price: Decimal,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;

    // update asset config
    store_asset_config(
        &mut deps.storage,
        &asset_token_raw,
        &AssetConfig {
            end_price: Some(end_price),
            min_collateral_ratio: Decimal::percent(100),
            ..asset_config
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "migrate_asset"),
            log("asset_token", asset_token.as_str()),
            log("end_price", end_price.to_string()),
        ],
        data: None,
    })
}

pub fn try_open_position<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    collateral: Asset,
    asset_info: AssetInfo,
    collateral_ratio: Decimal,
) -> StdResult<HandleResponse> {
    if collateral.amount.is_zero() {
        return Err(StdError::generic_err("Wrong collateral"));
    }

    let collateral_info_raw: AssetInfoRaw = collateral.info.to_raw(&deps)?;
    let asset_info_raw: AssetInfoRaw = asset_info.to_raw(&deps)?;
    let asset_token_raw = match asset_info_raw.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    if collateral_ratio < asset_config.min_collateral_ratio {
        return Err(StdError::generic_err(
            "Can not open a position with low collateral ratio than minimum",
        ));
    }

    let config: Config = read_config(&deps.storage)?;
    let oracle: HumanAddr = deps.api.human_address(&config.oracle)?;
    let price: Decimal = load_price(
        &deps,
        &oracle,
        &collateral_info_raw,
        &asset_info_raw,
        Some(env.block.time),
    )?;

    // Convert collateral to asset unit
    let mint_amount = collateral.amount * price * reverse_decimal(collateral_ratio);
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

    store_position_idx(&mut deps.storage, position_idx + Uint128(1u128))?;
    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&asset_config.token)?,
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: sender,
                amount: mint_amount,
            })?,
        })],
        log: vec![
            log("action", "open_position"),
            log("position_idx", position_idx.to_string()),
            log(
                "mint_amount",
                mint_amount.to_string() + &asset_info.to_string(),
            ),
            log("collateral_amount", collateral.to_string()),
        ],
        data: None,
    })
}

pub fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    sender: HumanAddr,
    position_idx: Uint128,
    collateral: Asset,
) -> HandleResult {
    let mut position: Position = read_position(&deps.storage, position_idx)?;
    if position.owner != deps.api.canonical_address(&sender)? {
        return Err(StdError::unauthorized());
    }

    // Check the given collateral has same asset info
    // with position's collateral token
    // also Check the collateral amount is non-zero
    assert_collateral(deps, &position, &collateral)?;

    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config = read_asset_config(&deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

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

pub fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    position_idx: Uint128,
    collateral: Asset,
) -> HandleResult {
    let mut position: Position = read_position(&deps.storage, position_idx)?;
    if position.owner != deps.api.canonical_address(&env.message.sender)? {
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

    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    let oracle: HumanAddr = deps.api.human_address(&config.oracle)?;
    let price: Decimal = load_price(
        &deps,
        &oracle,
        &position.asset.info,
        &position.collateral.info,
        Some(env.block.time),
    )?;

    // Compute new collateral amount
    let collateral_amount: Uint128 = (position.collateral.amount - collateral.amount)?;

    // Convert asset to collateral unit
    let asset_value_in_collateral_asset: Uint128 = position.asset.amount * price;

    // Check minimum collateral ratio is statified
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio > collateral_amount {
        return Err(StdError::generic_err(
            "Cannot withdraw collateral over than minimum collateral ratio",
        ));
    }

    position.collateral.amount = collateral_amount;
    if position.collateral.amount == Uint128::zero() && position.asset.amount == Uint128::zero() {
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

    Ok(HandleResponse {
        messages: vec![
            collateral
                .clone()
                .into_msg(&deps, env.contract.address.clone(), env.message.sender)?,
            protocol_fee.clone().into_msg(
                &deps,
                env.contract.address,
                deps.api.human_address(&config.collector)?,
            )?,
        ],
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

pub fn try_mint<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    position_idx: Uint128,
    asset: Asset,
) -> HandleResult {
    let mut position: Position = read_position(&deps.storage, position_idx)?;
    if position.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    assert_asset(&deps, &position, &asset)?;

    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    let oracle: HumanAddr = deps.api.human_address(&config.oracle)?;
    let price: Decimal = load_price(
        &deps,
        &oracle,
        &position.asset.info,
        &position.collateral.info,
        Some(env.block.time),
    )?;

    // Compute new asset amount
    let asset_amount: Uint128 = asset.amount + position.asset.amount;

    // Convert asset to collateral unit
    let asset_value_in_collateral_asset: Uint128 = asset_amount * price;

    // Check minimum collateral ratio is statified
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio
        > position.collateral.amount
    {
        return Err(StdError::generic_err(
            "Cannot mint asset over than min collateral ratio",
        ));
    }

    position.asset.amount += asset.amount;
    store_position(&mut deps.storage, position_idx, &position)?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&asset_config.token)?,
            msg: to_binary(&Cw20HandleMsg::Mint {
                amount: asset.amount,
                recipient: env.message.sender,
            })?,
            send: vec![],
        })],
        log: vec![
            log("action", "mint"),
            log("position_idx", position_idx.to_string()),
            log("mint_amount", asset.to_string()),
        ],
        data: None,
    })
}

pub fn try_burn<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    position_idx: Uint128,
    asset: Asset,
) -> StdResult<HandleResponse> {
    let mut position: Position = read_position(&deps.storage, position_idx)?;

    // Check the asset has same token with position asset
    // also Check burn amount is non-zero
    assert_asset(deps, &position, &asset)?;
    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    if position.asset.amount < asset.amount {
        return Err(StdError::generic_err(
            "Cannot burn asset more than you mint",
        ));
    }

    // If the asset is in deprecated state, anyone can exeucte burn
    let mut messages: Vec<CosmosMsg> = vec![];
    let mut logs: Vec<LogAttribute> = vec![];
    if let Some(end_price) = asset_config.end_price {
        // Burn deprecated asset to receive collaterals back
        let refund_collateral = Asset {
            info: position.collateral.info.to_normal(&deps)?,
            amount: asset.amount * end_price,
        };

        position.asset.amount = (position.asset.amount - asset.amount).unwrap();
        position.collateral.amount =
            (position.collateral.amount - refund_collateral.amount).unwrap();
        if position.collateral.amount == Uint128::zero() && position.asset.amount == Uint128::zero()
        {
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
        if position.owner != deps.api.canonical_address(&sender)? {
            return Err(StdError::unauthorized());
        }

        // Update asset amount
        position.asset.amount = (position.asset.amount - asset.amount).unwrap();
        store_position(&mut deps.storage, position_idx, &position)?;
    }

    Ok(HandleResponse {
        messages: vec![
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&asset_config.token)?,
                msg: to_binary(&Cw20HandleMsg::Burn {
                    amount: asset.amount,
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

pub fn try_auction<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    position_idx: Uint128,
    asset: Asset,
) -> StdResult<HandleResponse> {
    let mut position: Position = read_position(&deps.storage, position_idx)?;
    assert_asset(&deps, &position, &asset)?;

    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = match position.asset.info.clone() {
        AssetInfoRaw::Token { contract_addr } => contract_addr,
        _ => panic!("DO NOT ENTER HERE"),
    };

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;
    assert_migrated_asset(&asset_config)?;

    let collateral_info = position.collateral.info.to_normal(&deps)?;
    let position_owner = deps.api.human_address(&position.owner)?;

    if asset.amount > position.asset.amount {
        return Err(StdError::generic_err(
            "Cannot liquidate more than the position amount".to_string(),
        ));
    }

    let oracle: HumanAddr = deps.api.human_address(&config.oracle)?;
    let price: Decimal = load_price(
        &deps,
        &oracle,
        &position.asset.info,
        &position.collateral.info,
        Some(env.block.time),
    )?;

    // Check the position is in auction state
    // asset_amount * price_to_collateral * auction_threshold > collateral_amount
    let asset_value_in_collateral_asset: Uint128 = position.asset.amount * price;
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio
        < position.collateral.amount
    {
        return Err(StdError::generic_err(
            "Cannot liquidate a safely collateralized position",
        ));
    }

    // Compute discounted price
    let discounted_price: Decimal = decimal_division(
        price,
        decimal_subtraction(Decimal::one(), asset_config.auction_discount),
    );

    // Convert asset value in discounted colalteral unit
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

    if left_collateral_amount.is_zero() {
        // all collaterals are sold out
        remove_position(&mut deps.storage, position_idx)?;
    } else if left_asset_amount.is_zero() {
        // all assets are paid
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
        contract_addr: deps.api.human_address(&asset_config.token)?,
        msg: to_binary(&Cw20HandleMsg::Burn {
            amount: liquidated_asset_amount,
        })?,
        send: vec![],
    }));

    // Deduct protocol fee
    let protocol_fee = return_collateral_amount * config.protocol_fee_rate;
    let return_collateral_amount = (return_collateral_amount - protocol_fee).unwrap();

    // return collateral to liqudation initiator(sender)
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

    messages.push(protocol_fee_asset.into_msg(
        &deps,
        env.contract.address,
        deps.api.human_address(&config.collector)?,
    )?);

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

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::AssetConfig { asset_token } => to_binary(&query_asset_config(deps, asset_token)?),
        QueryMsg::Position { position_idx } => to_binary(&query_position(deps, position_idx)?),
        QueryMsg::Positions {
            owner_addr,
            asset_token,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_positions(
            deps,
            owner_addr,
            asset_token,
            start_after,
            limit,
            order_by,
        )?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        oracle: deps.api.human_address(&state.oracle)?,
        collector: deps.api.human_address(&state.collector)?,
        base_denom: state.base_denom,
        token_code_id: state.token_code_id,
        protocol_fee_rate: Decimal::percent(1),
    };

    Ok(resp)
}

pub fn query_asset_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_token: HumanAddr,
) -> StdResult<AssetConfigResponse> {
    let asset_config: AssetConfig =
        read_asset_config(&deps.storage, &deps.api.canonical_address(&asset_token)?)?;

    let resp = AssetConfigResponse {
        token: deps.api.human_address(&asset_config.token).unwrap(),
        auction_discount: asset_config.auction_discount,
        min_collateral_ratio: asset_config.min_collateral_ratio,
        end_price: asset_config.end_price,
    };

    Ok(resp)
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
            })
        })
        .collect();

    Ok(PositionsResponse {
        positions: position_responses?,
    })
}

// Check zero balance & same collateral with position
fn assert_collateral<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    position: &Position,
    collateral: &Asset,
) -> StdResult<()> {
    if !collateral
        .info
        .equal(&position.collateral.info.to_normal(&deps)?)
        || collateral.amount.is_zero()
    {
        return Err(StdError::generic_err("Wrong collateral"));
    }

    Ok(())
}

// Check zero balance & same asset with position
fn assert_asset<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    position: &Position,
    asset: &Asset,
) -> StdResult<()> {
    if !asset.info.equal(&position.asset.info.to_normal(&deps)?) || asset.amount.is_zero() {
        return Err(StdError::generic_err("Wrong asset"));
    }

    Ok(())
}

fn assert_migrated_asset(asset_config: &AssetConfig) -> StdResult<()> {
    if asset_config.end_price.is_some() {
        return Err(StdError::generic_err(
            "Operation is not allowed for the deprecated asset",
        ));
    }

    Ok(())
}

fn assert_auction_discount(auction_discount: Decimal) -> StdResult<()> {
    if auction_discount > Decimal::one() {
        Err(StdError::generic_err(
            "auction_discount must be smaller than 1",
        ))
    } else {
        Ok(())
    }
}

fn assert_min_collateral_ratio(min_collateral_ratio: Decimal) -> StdResult<()> {
    if min_collateral_ratio < Decimal::one() {
        Err(StdError::generic_err(
            "min_collateral_ratio must be bigger than 1",
        ))
    } else {
        Ok(())
    }
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> MigrateResult {
    Ok(MigrateResponse::default())
}
