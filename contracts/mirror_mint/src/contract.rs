use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, Querier, StdError, StdResult, Storage,
    Uint128, WasmMsg,
};

use crate::msg::{
    AssetConfigResponse, ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, PositionResponse,
    PositionsResponse, QueryMsg,
};

use crate::state::{
    create_position, read_asset_config, read_config, read_position, read_position_idx,
    read_positions, remove_position, store_asset_config, store_config, store_position,
    store_position_idx, AssetConfig, Config, Position,
};

use crate::math::{decimal_multiplication, decimal_subtraction, reverse_decimal};
use crate::querier::load_prices;
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use uniswap::{Asset, AssetInfo, AssetInfoRaw, AssetRaw};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let config = Config {
        owner: deps.api.canonical_address(&msg.owner)?,
        oracle: deps.api.canonical_address(&msg.oracle)?,
        base_asset_info: msg.base_asset_info.to_raw(&deps)?,
        token_code_id: msg.token_code_id,
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
            token_code_id,
        } => try_update_config(deps, env, owner, token_code_id),
        HandleMsg::UpdateAsset {
            asset_info,
            auction_discount,
            min_collateral_ratio,
        } => try_update_asset(
            deps,
            env,
            asset_info,
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
                try_burn(deps, cw20_msg.sender, position_idx, passed_asset)
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
    token_code_id: Option<u64>,
) -> StdResult<HandleResponse> {
    let mut config: Config = read_config(&deps.storage)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(token_code_id) = token_code_id {
        config.token_code_id = token_code_id;
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
    asset_info: AssetInfo,
    auction_discount: Option<Decimal>,
    min_collateral_ratio: Option<Decimal>,
) -> StdResult<HandleResponse> {
    let raw_info = asset_info.to_raw(&deps)?;
    let config: Config = read_config(&deps.storage)?;
    let mut asset: AssetConfig = read_asset_config(&deps.storage, &raw_info)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(auction_discount) = auction_discount {
        asset.auction_discount = auction_discount;
    }

    if let Some(min_collateral_ratio) = min_collateral_ratio {
        asset.min_collateral_ratio = min_collateral_ratio;
    }

    store_asset_config(&mut deps.storage, &raw_info, &asset)?;
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
    let config: Config = read_config(&deps.storage)?;

    // permission check
    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token)?;
    let raw_info = AssetInfoRaw::Token {
        contract_addr: asset_token_raw.clone(),
    };

    if read_asset_config(&deps.storage, &raw_info).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    // Store temp info into base asset store
    store_asset_config(
        &mut deps.storage,
        &raw_info,
        &AssetConfig {
            token: asset_token_raw,
            auction_discount,
            min_collateral_ratio,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "register"), log("asset_token", asset_token)],
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

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_info_raw)?;
    if collateral_ratio < asset_config.min_collateral_ratio {
        return Err(StdError::generic_err(
            "Can not open a position with low collateral ratio than minimum",
        ));
    }

    let config: Config = read_config(&deps.storage)?;
    let (collateral_price, asset_price) = load_prices(
        &deps,
        &config,
        &collateral_info_raw,
        &asset_info_raw,
        Some(env.block.time),
    )?;

    let mint_amount = collateral.amount
        * collateral_price
        * reverse_decimal(asset_price)
        * reverse_decimal(collateral_ratio);
    if mint_amount.is_zero() {
        return Err(StdError::generic_err("collateral is too small"));
    }

    let position_idx = read_position_idx(&deps.storage)?;
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

    let config = read_config(&deps.storage)?;
    let asset_config = read_asset_config(&deps.storage, &position.asset.info)?;

    // Load collateral & asset prices in base token price
    let (collateral_price, asset_price) = load_prices(
        &deps,
        &config,
        &position.collateral.info,
        &position.asset.info,
        Some(env.block.time),
    )?;

    // Compute new collateral amount
    let collateral_amount: Uint128 = (position.collateral.amount - collateral.amount)?;

    // Convert asset to collateral unit
    let asset_value_in_collateral_asset: Uint128 =
        position.asset.amount * asset_price * reverse_decimal(collateral_price);
    // Check minimum collateral ratio is statified
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio > collateral_amount {
        return Err(StdError::generic_err(
            "Cannot withdraw collateral over than minimum collateral ratio",
        ));
    }

    position.collateral.amount = collateral_amount;
    if position.collateral.amount == Uint128::zero() && position.asset.amount == Uint128::zero() {
        remove_position(&mut deps.storage, position_idx, &position.owner)?;
    } else {
        store_position(&mut deps.storage, position_idx, &position)?;
    }

    let tax_amount = collateral.compute_tax(&deps)?;
    Ok(HandleResponse {
        messages: vec![collateral.clone().into_msg(
            &deps,
            env.contract.address,
            env.message.sender,
        )?],
        log: vec![
            log("action", "withdraw"),
            log("position_idx", position_idx.to_string()),
            log("withdraw_amount", collateral.to_string()),
            log(
                "tax_amount",
                tax_amount.to_string() + &collateral.info.to_string(),
            ),
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
    let asset_config: AssetConfig = read_asset_config(&deps.storage, &position.asset.info)?;

    let (collateral_price, asset_price) = load_prices(
        &deps,
        &config,
        &position.collateral.info,
        &position.asset.info,
        Some(env.block.time),
    )?;

    // Compute new asset amount
    let asset_amount: Uint128 = asset.amount + position.asset.amount;

    // Convert asset to collateral unit
    let asset_value_in_collateral_asset: Uint128 =
        asset_amount * asset_price * reverse_decimal(collateral_price);

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
    sender: HumanAddr,
    position_idx: Uint128,
    asset: Asset,
) -> StdResult<HandleResponse> {
    let mut position: Position = read_position(&deps.storage, position_idx)?;
    if position.owner != deps.api.canonical_address(&sender)? {
        return Err(StdError::unauthorized());
    }

    // Check the asset has same token with position asset
    // also Check burn amount is non-zero
    assert_asset(deps, &position, &asset)?;

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &position.asset.info)?;

    if position.asset.amount < asset.amount {
        return Err(StdError::generic_err(
            "Cannot burn asset more than you mint",
        ));
    }

    // Update asset amount
    position.asset.amount = (position.asset.amount - asset.amount)?;
    if position.collateral.amount == Uint128::zero() && position.asset.amount == Uint128::zero() {
        remove_position(&mut deps.storage, position_idx, &position.owner)?;
    } else {
        store_position(&mut deps.storage, position_idx, &position)?;
    }

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&asset_config.token)?,
            msg: to_binary(&Cw20HandleMsg::Burn {
                amount: asset.amount,
            })?,
            send: vec![],
        })],
        log: vec![
            log("action", "burn"),
            log("position_idx", position_idx.to_string()),
            log("burn_amount", asset.to_string()),
        ],
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
    let asset_config: AssetConfig = read_asset_config(&deps.storage, &position.asset.info)?;

    let collateral_info = position.collateral.info.to_normal(&deps)?;
    let position_owner = deps.api.human_address(&position.owner)?;

    if asset.amount > position.asset.amount {
        return Err(StdError::generic_err(
            "Cannot liquidate more than the position amount".to_string(),
        ));
    }

    let (collateral_price, asset_price) = load_prices(
        &deps,
        &config,
        &position.collateral.info,
        &position.asset.info,
        Some(env.block.time),
    )?;

    // Check the position is in auction state
    // asset_amount * price_to_collateral * auction_threshold > collateral_amount
    let asset_value_in_collateral_asset: Uint128 =
        position.asset.amount * asset_price * reverse_decimal(collateral_price);
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio
        < position.collateral.amount
    {
        return Err(StdError::generic_err(
            "Cannot liquidate a safely collateralized position",
        ));
    }

    // Compute discounted collateral price
    let discounted_collateral_price: Decimal = decimal_multiplication(
        collateral_price,
        decimal_subtraction(Decimal::one(), asset_config.auction_discount),
    );

    // Convert asset value in discounted colalteral unit
    let asset_value_in_collateral_asset: Uint128 =
        asset.amount * asset_price * reverse_decimal(discounted_collateral_price);

    let mut messages: Vec<CosmosMsg> = vec![];

    // Cap return collateral amount to position collateral amount
    // If the given asset amount exceeds the amount required to liquidate position,
    // left asset amount will be refunds to executor.
    let (return_collateral_amount, refund_asset_amount) =
        if asset_value_in_collateral_asset > position.collateral.amount {
            // refunds left asset to position liquidator
            let refund_asset_amount =
                (asset_value_in_collateral_asset - position.collateral.amount).unwrap()
                    * discounted_collateral_price
                    * reverse_decimal(asset_price);

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
        remove_position(&mut deps.storage, position_idx, &position.owner)?;
    } else if left_asset_amount.is_zero() {
        // all assets are paid
        remove_position(&mut deps.storage, position_idx, &position.owner)?;

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

    // return collateral to liqudation initiator(sender)
    let return_collateral_asset = Asset {
        info: collateral_info.clone(),
        amount: return_collateral_amount,
    };

    // liquidated asset
    let liquidated_asset: Asset = Asset {
        info: asset.info,
        amount: liquidated_asset_amount,
    };

    let tax_amount = return_collateral_asset.compute_tax(&deps)?;
    messages.push(return_collateral_asset.into_msg(&deps, env.contract.address, sender)?);

    Ok(HandleResponse {
        log: vec![
            log("action", "auction"),
            log("owner", position_owner.as_str()),
            log(
                "return_collateral_amount",
                return_collateral_amount.to_string() + &collateral_info.to_string(),
            ),
            log("liquidated_amount", liquidated_asset.to_string()),
            log(
                "tax_amount",
                tax_amount.to_string() + &collateral_info.to_string(),
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
        QueryMsg::AssetConfig { asset_info } => to_binary(&query_asset_config(deps, asset_info)?),
        QueryMsg::Position { position_idx } => to_binary(&query_position(deps, position_idx)?),
        QueryMsg::Positions {
            owner_addr,
            start_after,
            limit,
        } => to_binary(&query_positions(deps, owner_addr, start_after, limit)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        oracle: deps.api.human_address(&state.oracle)?,
        base_asset_info: state.base_asset_info.to_normal(&deps)?,
        token_code_id: state.token_code_id,
    };

    Ok(resp)
}

pub fn query_asset_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_info: AssetInfo,
) -> StdResult<AssetConfigResponse> {
    let raw_info = asset_info.to_raw(&deps)?;
    let asset_config: AssetConfig = read_asset_config(&deps.storage, &raw_info)?;

    let resp = AssetConfigResponse {
        token: deps.api.human_address(&asset_config.token).unwrap(),
        auction_discount: asset_config.auction_discount,
        min_collateral_ratio: asset_config.min_collateral_ratio,
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
    owner_addr: HumanAddr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<PositionsResponse> {
    let positions: Vec<Position> = read_positions(
        &deps.storage,
        &deps.api.canonical_address(&owner_addr)?,
        start_after,
        limit,
    )?;

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
