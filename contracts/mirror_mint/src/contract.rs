use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, Order, Querier, StdError, StdResult,
    Storage, Uint128, WasmMsg,
};

use crate::msg::{
    AssetConfigResponse, ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, PositionResponse,
    PositionsResponse, QueryMsg,
};

use crate::state::{
    positions_read, positions_store, read_asset_config, read_config, store_asset_config,
    store_config, AssetConfig, Config, Position,
};

use crate::math::reverse_decimal;
use crate::querier::load_price;
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use std::collections::HashMap;
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
            auction_threshold_ratio,
            min_collateral_ratio,
        } => try_update_asset(
            deps,
            env,
            asset_info,
            auction_discount,
            auction_threshold_ratio,
            min_collateral_ratio,
        ),
        HandleMsg::RegisterAsset {
            asset_token_addr,
            auction_discount,
            auction_threshold_ratio,
            min_collateral_ratio,
        } => try_register_asset(
            deps,
            env,
            asset_token_addr,
            auction_discount,
            auction_threshold_ratio,
            min_collateral_ratio,
        ),
        HandleMsg::Deposit {
            collateral,
            asset_info,
        } => {
            // only native token can be deposited directly
            if !collateral.is_native_token() {
                return Err(StdError::unauthorized());
            }

            try_deposit(
                deps,
                env.clone(),
                env.message.sender,
                collateral,
                asset_info,
            )
        }
        HandleMsg::Withdraw {
            collateral,
            asset_info,
        } => try_withdraw(deps, env, collateral, asset_info),
        HandleMsg::Mint {
            asset,
            collateral_info,
        } => try_mint(deps, env, asset, collateral_info),
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
            Cw20HookMsg::Burn { collateral_info } => try_burn(
                deps,
                env.clone(),
                cw20_msg.sender,
                passed_asset,
                collateral_info,
            ),
            Cw20HookMsg::Auction {
                collateral_info,
                position_owner,
            } => try_auction(
                deps,
                env.clone(),
                cw20_msg.sender,
                position_owner,
                passed_asset,
                collateral_info,
            ),
            Cw20HookMsg::Deposit { asset_info } => {
                try_deposit(deps, env.clone(), cw20_msg.sender, passed_asset, asset_info)
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
    auction_threshold_rate: Option<Decimal>,
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

    if let Some(auction_threshold_rate) = auction_threshold_rate {
        asset.auction_threshold_ratio = auction_threshold_rate;
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
    asset_token_addr: HumanAddr,
    auction_discount: Decimal,
    auction_threshold_ratio: Decimal,
    min_collateral_ratio: Decimal,
) -> StdResult<HandleResponse> {
    let config: Config = read_config(&deps.storage)?;

    // permission check
    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&asset_token_addr)?;
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
            auction_threshold_ratio,
            min_collateral_ratio,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "register"),
            log("asset_token_addr", asset_token_addr),
        ],
        data: None,
    })
}

pub fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    collateral: Asset,
    asset_info: AssetInfo,
) -> HandleResult {
    collateral.assert_sent_native_token_balance(&env)?;
    if collateral.amount.is_zero() {
        return Err(StdError::generic_err("Cannot deposit zero amount"));
    }

    let minter: CanonicalAddr = deps.api.canonical_address(&sender)?;
    let asset_raw_info: AssetInfoRaw = asset_info.to_raw(&deps)?;
    let collateral_raw_info: AssetInfoRaw = collateral.info.to_raw(&deps)?;

    let position_bucket = positions_read(&deps.storage, &minter);
    let position_key = [asset_raw_info.as_bytes(), collateral_raw_info.as_bytes()].concat();
    let mut position: Position = position_bucket
        .load(&position_key)
        .unwrap_or_else(|_| Position {
            collateral: AssetRaw {
                info: collateral_raw_info.clone(),
                amount: Uint128::zero(),
            },
            asset: AssetRaw {
                info: asset_raw_info.clone(),
                amount: Uint128::zero(),
            },
        });

    // just update collateral amount
    position.collateral.amount += collateral.amount;
    positions_store(&mut deps.storage, &minter).save(&position_key, &position)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "deposit"),
            log("pair", format!("{}/{}", asset_info, collateral.info)),
            log("deposit_amount", collateral.amount.to_string()),
        ],
        data: None,
    })
}

pub fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    collateral: Asset,
    asset_info: AssetInfo,
) -> HandleResult {
    if collateral.amount.is_zero() {
        return Err(StdError::generic_err("Cannot deposit zero amount"));
    }

    let minter: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    let asset_raw_info: AssetInfoRaw = asset_info.to_raw(&deps)?;
    let collateral_raw_info: AssetInfoRaw = collateral.info.to_raw(&deps)?;

    let config = read_config(&deps.storage)?;
    let asset_config = read_asset_config(&deps.storage, &asset_raw_info)?;
    let position_bucket = positions_read(&deps.storage, &minter);
    let position_key = [asset_raw_info.as_bytes(), collateral_raw_info.as_bytes()].concat();
    let mut position: Position = position_bucket.load(&position_key)?;

    if position.collateral.amount < collateral.amount {
        return Err(StdError::generic_err(
            "Cannot withdraw more than you provide",
        ));
    }

    // collateral can be token or native token
    let collateral_price = if collateral
        .info
        .equal(&config.base_asset_info.to_normal(&deps)?)
    {
        Decimal::one()
    } else {
        // load collateral price form the oracle
        load_price(
            &deps,
            &deps.api.human_address(&config.oracle)?,
            &collateral_raw_info,
            Some(env.block.time),
        )?
    };

    // load asset price from the oracle
    let asset_price = load_price(
        &deps,
        &deps.api.human_address(&config.oracle)?,
        &asset_raw_info,
        Some(env.block.time),
    )?;

    let collateral_amount: Uint128 = (position.collateral.amount - collateral.amount)?;
    let asset_value_in_collateral_asset: Uint128 =
        position.asset.amount * asset_price * reverse_decimal(collateral_price);
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio > collateral_amount {
        return Err(StdError::generic_err(
            "Cannot withdraw collateral over than minimum collateral ratio",
        ));
    }

    position.collateral.amount = collateral_amount;
    if position.collateral.amount == Uint128::zero() && position.asset.amount == Uint128::zero() {
        positions_store(&mut deps.storage, &minter).remove(&position_key);
    } else {
        positions_store(&mut deps.storage, &minter).save(&position_key, &position)?;
    }

    Ok(HandleResponse {
        messages: vec![collateral.clone().into_msg(
            &deps,
            env.contract.address,
            env.message.sender,
        )?],
        log: vec![
            log("action", "withdraw"),
            log("pair", format!("{}/{}", asset_info, collateral.info)),
            log("withdraw_amount", collateral.amount.to_string()),
        ],
        data: None,
    })
}

pub fn try_mint<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset: Asset,
    collateral_info: AssetInfo,
) -> HandleResult {
    if asset.amount.is_zero() {
        return Err(StdError::generic_err("Cannot mint zero amount"));
    }

    let config: Config = read_config(&deps.storage)?;
    let minter: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    let asset_raw_info: AssetInfoRaw = asset.info.to_raw(&deps)?;
    let collateral_raw_info: AssetInfoRaw = collateral_info.to_raw(&deps)?;

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_raw_info)?;
    let position_bucket = positions_read(&deps.storage, &minter);
    let position_key = [asset_raw_info.as_bytes(), collateral_raw_info.as_bytes()].concat();
    let mut position: Position = position_bucket.load(&position_key)?;

    // collateral can be token or native token
    let collateral_price = if collateral_info.equal(&config.base_asset_info.to_normal(&deps)?) {
        Decimal::one()
    } else {
        // load collateral price form the oracle
        load_price(
            &deps,
            &deps.api.human_address(&config.oracle)?,
            &collateral_raw_info,
            Some(env.block.time),
        )?
    };

    // load asset price from the oracle
    let asset_price = load_price(
        &deps,
        &deps.api.human_address(&config.oracle)?,
        &asset_raw_info,
        Some(env.block.time),
    )?;

    // asset amount always satisfy;
    // asset_amount * price_to_collateral * min_collateral_ratio < collateral_amount
    let asset_amount: Uint128 = asset.amount + position.asset.amount;
    let asset_value_in_collateral_asset: Uint128 =
        asset_amount * asset_price * reverse_decimal(collateral_price);
    if asset_value_in_collateral_asset * asset_config.min_collateral_ratio
        > position.collateral.amount
    {
        return Err(StdError::generic_err(
            "Cannot mint asset over than min collateral ratio",
        ));
    }

    position.asset.amount = asset_amount;
    positions_store(&mut deps.storage, &minter).save(&position_key, &position)?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&asset_config.token)?,
            msg: to_binary(&Cw20HandleMsg::Mint {
                amount: asset.amount,
                recipient: env.message.sender.clone(),
            })?,
            send: vec![],
        })],
        log: vec![
            log("action", "mint"),
            log("pair", format!("{}/{}", asset.info, collateral_info)),
            log("mint_amount", asset.amount.to_string()),
        ],
        data: None,
    })
}

pub fn try_burn<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    sender: HumanAddr,
    asset: Asset,
    collateral_info: AssetInfo,
) -> StdResult<HandleResponse> {
    // burn only can be called from asset contract
    if asset.amount.is_zero() {
        return Err(StdError::generic_err("Cannot burn zero amount"));
    }

    let minter: CanonicalAddr = deps.api.canonical_address(&sender)?;
    let asset_raw_info: AssetInfoRaw = asset.info.to_raw(&deps)?;
    let collateral_raw_info: AssetInfoRaw = collateral_info.to_raw(&deps)?;

    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_raw_info)?;
    let position_bucket = positions_read(&deps.storage, &minter);
    let position_key = [asset_raw_info.as_bytes(), collateral_raw_info.as_bytes()].concat();
    let mut position: Position = position_bucket.load(&position_key)?;

    if position.asset.amount < asset.amount {
        return Err(StdError::generic_err(
            "Cannot burn asset more than you mint",
        ));
    }

    position.asset.amount = (position.asset.amount - asset.amount)?;
    if position.collateral.amount == Uint128::zero() && position.asset.amount == Uint128::zero() {
        positions_store(&mut deps.storage, &minter).remove(&position_key);
    } else {
        positions_store(&mut deps.storage, &minter).save(&position_key, &position)?;
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
            log("pair", format!("{}/{}", asset.info, collateral_info)),
            log("burn_amount", asset.amount.to_string()),
        ],
        data: None,
    })
}

pub fn try_auction<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    position_owner: HumanAddr,
    asset: Asset,
    collateral_info: AssetInfo,
) -> StdResult<HandleResponse> {
    let asset_raw_info: AssetInfoRaw = asset.info.to_raw(&deps)?;
    let collateral_raw_info: AssetInfoRaw = collateral_info.to_raw(&deps)?;
    let position_owner_raw: CanonicalAddr = deps.api.canonical_address(&position_owner)?;
    let config: Config = read_config(&deps.storage)?;
    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_raw_info)?;
    let position_bucket = positions_read(&deps.storage, &position_owner_raw);
    let position_key = [asset_raw_info.as_bytes(), collateral_raw_info.as_bytes()].concat();
    let mut position: Position = position_bucket.load(&position_key)?;

    if asset.amount > position.asset.amount {
        return Err(StdError::generic_err(
            "Cannot liquidate more than the position amount".to_string(),
        ));
    }

    // collateral can be token or native token
    let collateral_price = if collateral_info.equal(&config.base_asset_info.to_normal(&deps)?) {
        Decimal::one()
    } else {
        // load collateral price form the oracle
        load_price(
            &deps,
            &deps.api.human_address(&config.oracle)?,
            &collateral_raw_info,
            Some(env.block.time),
        )?
    };

    // load asset price from the oracle
    let asset_price = load_price(
        &deps,
        &deps.api.human_address(&config.oracle)?,
        &asset_raw_info,
        Some(env.block.time),
    )?;

    // Check the position is in auction state
    // asset_amount * price_to_collateral * auction_threshold > collateral_amount
    let asset_amount: Uint128 = asset.amount + position.asset.amount;
    let asset_value_in_collateral_asset: Uint128 =
        asset_amount * asset_price * reverse_decimal(collateral_price);
    if asset_value_in_collateral_asset * asset_config.auction_threshold_ratio
        < position.collateral.amount
    {
        return Err(StdError::generic_err(
            "Cannot liquidate a safely collateralized position",
        ));
    }

    // compute collateral amount from the asset amount a user sent
    let asset_value_in_collateral_asset: Uint128 =
        asset.amount * asset_price * reverse_decimal(collateral_price);
    let return_collateral_amount: Uint128 = std::cmp::min(
        asset_value_in_collateral_asset
            + asset_value_in_collateral_asset * asset_config.auction_discount,
        position.collateral.amount,
    );

    let left_asset_amount = (position.asset.amount - asset.amount).unwrap();
    let left_collateral_amount = (position.collateral.amount - return_collateral_amount).unwrap();

    let mut messages: Vec<CosmosMsg> = vec![];
    if left_collateral_amount.is_zero() {
        // all collaterals are sold out
        positions_store(&mut deps.storage, &position_owner_raw).remove(&position_key);
    } else if left_asset_amount.is_zero() {
        // all assets are paid
        positions_store(&mut deps.storage, &position_owner_raw).remove(&position_key);

        // refunds left collaterals to position owner
        let refund_collateral_asset: Asset = Asset {
            info: collateral_info.clone(),
            amount: left_collateral_amount,
        };

        messages.push(refund_collateral_asset.into_msg(
            &deps,
            env.contract.address.clone(),
            position_owner.clone(),
        )?);
    } else {
        position.collateral.amount = left_collateral_amount;
        position.asset.amount = left_asset_amount;

        positions_store(&mut deps.storage, &position_owner_raw).save(&position_key, &position)?;
    }

    // token burn message
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.human_address(&asset_config.token)?,
        msg: to_binary(&Cw20HandleMsg::Burn {
            amount: asset.amount,
        })?,
        send: vec![],
    }));

    // return collateral to liqudation initiator(sender)
    let return_collateral_asset = Asset {
        info: collateral_info,
        amount: return_collateral_amount,
    };

    messages.push(return_collateral_asset.into_msg(&deps, env.contract.address, sender)?);

    Ok(HandleResponse {
        log: vec![
            log("action", "auction"),
            log("owner", position_owner.as_str()),
            log(
                "return_collateral_amount",
                return_collateral_amount.to_string(),
            ),
            log("liquidated_amount", asset.amount.to_string()),
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
        QueryMsg::Position {
            minter,
            asset_info,
            collateral_info,
        } => to_binary(&query_position(deps, minter, asset_info, collateral_info)?),
        QueryMsg::Positions { minter } => to_binary(&query_positions(deps, minter)?),
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
        auction_threshold_ratio: asset_config.auction_threshold_ratio,
        min_collateral_ratio: asset_config.min_collateral_ratio,
    };

    Ok(resp)
}

pub fn query_position<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    minter: HumanAddr,
    asset_info: AssetInfo,
    collateral_info: AssetInfo,
) -> StdResult<PositionResponse> {
    let asset_raw_info: AssetInfoRaw = asset_info.to_raw(&deps)?;
    let collateral_raw_info: AssetInfoRaw = collateral_info.to_raw(&deps)?;
    let minter_raw = deps.api.canonical_address(&minter)?;

    let config: Config = read_config(&deps.storage)?;
    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_raw_info)?;
    let position_bucket = positions_read(&deps.storage, &minter_raw);
    let position_key = [asset_raw_info.as_bytes(), collateral_raw_info.as_bytes()].concat();
    let position: Position = position_bucket.load(&position_key)?;

    // collateral can be token or native token
    let collateral_price = if collateral_info.equal(&config.base_asset_info.to_normal(&deps)?) {
        Decimal::one()
    } else {
        // load collateral price form the oracle
        load_price(
            &deps,
            &deps.api.human_address(&config.oracle)?,
            &collateral_raw_info,
            None,
        )?
    };

    // load asset price from the oracle
    let asset_price = load_price(
        &deps,
        &deps.api.human_address(&config.oracle)?,
        &asset_raw_info,
        None,
    )?;

    // the position is in auction state, when
    // asset_amount * price_to_collateral * auction_threshold > collateral_amount
    let asset_value_in_collateral_asset: Uint128 =
        position.asset.amount * asset_price * reverse_decimal(collateral_price);
    let is_auction_open = asset_value_in_collateral_asset * asset_config.auction_threshold_ratio
        > position.collateral.amount;

    let resp = PositionResponse {
        collateral: position.collateral.to_normal(&deps)?,
        asset: position.asset.to_normal(&deps)?,
        is_auction_open,
    };

    Ok(resp)
}

pub fn query_positions<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    minter: HumanAddr,
) -> StdResult<PositionsResponse> {
    let minter_raw = deps.api.canonical_address(&minter)?;

    let config: Config = read_config(&deps.storage)?;
    let position_bucket = positions_read(&deps.storage, &minter_raw);
    let positions: StdResult<Vec<Position>> = position_bucket
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (_k, v) = item?;
            Ok(v)
        })
        .collect();

    let mut responses: Vec<PositionResponse> = vec![];
    let mut price_map: HashMap<String, Decimal> = HashMap::new();
    let base_asset_info: AssetInfo = config.base_asset_info.to_normal(&deps)?;
    for position in positions? {
        let asset_config: AssetConfig = read_asset_config(&deps.storage, &position.asset.info)?;
        let asset_info = position.asset.info.to_normal(&deps)?;
        let collateral_info = position.collateral.info.to_normal(&deps)?;

        // collateral can be token or native token
        let collateral_price = if collateral_info.equal(&base_asset_info) {
            Decimal::one()
        } else if let Some(price) = price_map.get(&collateral_info.to_string()) {
            price.clone()
        } else {
            // load collateral price form the oracle
            let price = load_price(
                &deps,
                &deps.api.human_address(&config.oracle)?,
                &position.collateral.info,
                None,
            )?;

            price_map.insert(collateral_info.to_string(), price);
            price
        };

        // load asset price from the oracle
        let asset_price = if let Some(price) = price_map.get(&asset_info.to_string()) {
            price.clone()
        } else {
            // load collateral price form the oracle
            let price = load_price(
                &deps,
                &deps.api.human_address(&config.oracle)?,
                &position.asset.info,
                None,
            )?;

            price_map.insert(collateral_info.to_string(), price);
            price
        };

        // the position is in auction state, when
        // asset_amount * price_to_collateral * auction_threshold > collateral_amount
        let asset_value_in_collateral_asset: Uint128 =
            position.asset.amount * asset_price * reverse_decimal(collateral_price);
        let is_auction_open = asset_value_in_collateral_asset
            * asset_config.auction_threshold_ratio
            > position.collateral.amount;

        responses.push(PositionResponse {
            collateral: position.collateral.to_normal(&deps)?,
            asset: position.asset.to_normal(&deps)?,
            is_auction_open,
        });
    }

    Ok(PositionsResponse {
        minter,
        positions: responses,
    })
}
