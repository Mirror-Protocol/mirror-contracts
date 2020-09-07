use cosmwasm_std::{
    from_binary, log, to_binary, to_vec, Api, BankMsg, Binary, CanonicalAddr, Coin, CosmosMsg,
    Decimal, Env, Extern, HandleResponse, HumanAddr, InitResponse, Querier, StdError, StdResult,
    Storage, Uint128, WasmMsg,
};

use crate::msg::{
    AssetResponse, ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, PositionResponse, QueryMsg,
};

use crate::state::{
    asset_read, asset_store, config_read, config_store, position_read, position_store, AssetState,
    ConfigState, PositionState,
};

use crate::math::{decimal_multiplication, reverse_decimal};
use crate::querier::load_price;
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use terra_cosmwasm::TerraQuerier;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let config = ConfigState {
        owner: deps.api.canonical_address(&env.message.sender)?,
        collateral_denom: msg.collateral_denom,
        auction_discount: msg.auction_discount,
        auction_threshold_rate: msg.auction_threshold_rate,
        mint_capacity: msg.mint_capacity,
    };

    config_store(&mut deps.storage).save(&config)?;

    let asset = AssetState {
        oracle: deps.api.canonical_address(&msg.asset_oracle)?,
        token: deps.api.canonical_address(&msg.asset_token)?,
        symbol: msg.asset_symbol.to_string(),
    };

    if !is_valid_symbol(&msg.asset_symbol) {
        return Err(StdError::generic_err(
            "Ticker symbol is not in expected format [a-zA-Z]{3,6}",
        ));
    }

    asset_store(&mut deps.storage).save(&asset)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::UpdateConfig {
            owner,
            auction_discount,
            auction_threshold_rate,
            mint_capacity,
        } => try_update_config(
            deps,
            env,
            owner,
            auction_discount,
            auction_threshold_rate,
            mint_capacity,
        ),
        HandleMsg::Mint {} => try_mint(deps, env),
        HandleMsg::Receive(msg) => try_receive(deps, env, msg),
    }
}

// CW20ReceiveMsg Handler
pub fn try_receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<HandleResponse> {
    let asset: AssetState = asset_read(&deps.storage).load()?;
    if asset.token != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::generic_err(
            "Can't be executed from unauthorized contract",
        ));
    }

    match cw20_msg.msg {
        Some(msg) => match from_binary(&msg)? {
            Cw20HookMsg::Burn {} => try_burn(deps, env, cw20_msg.sender, cw20_msg.amount),
            Cw20HookMsg::Auction { owner } => {
                try_auction(deps, env, cw20_msg.sender, cw20_msg.amount, owner)
            }
        },
        None => {
            return Err(StdError::generic_err(
                "Can't send funds without proper exeuction msg",
            ))
        }
    }
}

pub fn try_update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    auction_discount: Option<Decimal>,
    auction_threshold_rate: Option<Decimal>,
    mint_capacity: Option<Decimal>,
) -> StdResult<HandleResponse> {
    let api = deps.api;
    config_store(&mut deps.storage).update(|mut state| {
        if api.canonical_address(&env.message.sender)? != state.owner {
            return Err(StdError::unauthorized());
        }

        if let Some(owner) = owner {
            state.owner = api.canonical_address(&owner)?;
        }

        if let Some(auction_discount) = auction_discount {
            state.auction_discount = auction_discount;
        }

        if let Some(auction_threshold_rate) = auction_threshold_rate {
            state.auction_threshold_rate = auction_threshold_rate;
        }

        if let Some(mint_capacity) = mint_capacity {
            state.mint_capacity = mint_capacity;
        }

        Ok(state)
    })?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

pub fn try_mint<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let asset: AssetState = asset_read(&deps.storage).load()?;
    let config: ConfigState = config_read(&deps.storage).load()?;

    let minter: CanonicalAddr = deps.api.canonical_address(&env.message.sender)?;
    let collateral_coin: &Coin = env
        .message
        .sent_funds
        .iter()
        .find(|x| x.denom == config.collateral_denom)
        .ok_or_else(|| {
            StdError::generic_err(format!("No {} tokens sent", &config.collateral_denom))
        })?;

    let position: PositionState = position_read(&deps.storage, &minter)?;
    let collateral_amount: Uint128 = collateral_coin.amount + position.collateral_amount;

    // load price form the oracle
    let price: Decimal = load_price(
        deps,
        &deps.api.human_address(&asset.oracle)?,
        Some(env.block.time),
    )?;

    // calculated collateralized asset amount;
    // asset amount cannot be decreased by mint
    let asset_amount: Uint128 = std::cmp::max(
        collateral_amount * reverse_decimal(price) * config.mint_capacity,
        position.asset_amount,
    );

    if asset_amount.is_zero() {
        return Err(StdError::generic_err("Mint amount is too small"));
    }

    // store position info
    position_store(&mut deps.storage).set(
        minter.as_slice(),
        &to_vec(&PositionState {
            asset_amount,
            collateral_amount,
        })?,
    );

    // If collateralized asset amount is smaller than current position, we will not mint more assets
    let mint_amount: Uint128 = (asset_amount - position.asset_amount).unwrap();

    let mut messages: Vec<CosmosMsg> = vec![];
    if !mint_amount.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&asset.token)?,
            msg: to_binary(&Cw20HandleMsg::Mint {
                amount: mint_amount,
                recipient: env.message.sender.clone(),
            })?,
            send: vec![],
        }));
    }

    Ok(HandleResponse {
        log: vec![
            log("action", "mint"),
            log(
                "collateral_amount",
                &(collateral_amount.to_string() + config.collateral_denom.as_str()),
            ),
            log(
                "mint_amount",
                &(mint_amount.to_string() + asset.symbol.as_str()),
            ),
        ],
        messages,
        data: None,
    })
}

pub fn try_burn<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let config: ConfigState = config_read(&deps.storage).load()?;
    let asset: AssetState = asset_read(&deps.storage).load()?;
    let burner: CanonicalAddr = deps.api.canonical_address(&sender)?;
    let position: PositionState = position_read(&deps.storage, &burner)?;
    if position.asset_amount < amount {
        return Err(StdError::generic_err(
            "Burn amount is bigger than the position amount".to_string(),
        ));
    }

    // load price form the oracle
    let price: Decimal = load_price(
        deps,
        &deps.api.human_address(&asset.oracle)?,
        Some(env.block.time),
    )?;

    // Calculated required collateral to collateralize left asset;
    // collateral cannot be increased by burn
    let asset_amount: Uint128 = (position.asset_amount - amount).unwrap();
    let collateral_amount: Uint128 = std::cmp::min(
        price * asset_amount * reverse_decimal(config.mint_capacity),
        position.collateral_amount,
    );

    if asset_amount.is_zero() {
        // all asset tokens are paid back, remove position and refunds all collateral
        position_store(&mut deps.storage).remove(burner.as_slice());
    } else {
        position_store(&mut deps.storage).set(
            burner.as_slice(),
            &to_vec(&PositionState {
                asset_amount,
                collateral_amount,
            })?,
        );
    }

    // burn received tokens
    let mut messages: Vec<CosmosMsg> = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.human_address(&asset.token)?,
        msg: to_binary(&Cw20HandleMsg::Burn { amount: amount })?,
        send: vec![],
    })];

    // refund collateral
    // If required collateral is bigger than position collateral,
    // we will not refunds any collateral to burner
    let refund_collateral_amount: Uint128 =
        (position.collateral_amount - collateral_amount).unwrap();
    if !refund_collateral_amount.is_zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address.clone(),
            to_address: sender,
            amount: vec![deduct_tax(
                &deps,
                Coin {
                    denom: config.collateral_denom.to_string(),
                    amount: refund_collateral_amount,
                },
            )?],
        }));
    }

    Ok(HandleResponse {
        log: vec![
            log("action", "burn"),
            log(
                "refund_amount",
                &(refund_collateral_amount.to_string() + config.collateral_denom.as_str()),
            ),
            log("burn_amount", &(amount.to_string() + asset.symbol.as_str())),
        ],
        messages,
        data: None,
    })
}

pub fn try_auction<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    offer_asset_amount: Uint128,
    position_owner: HumanAddr,
) -> StdResult<HandleResponse> {
    let config: ConfigState = config_read(&deps.storage).load()?;
    let asset: AssetState = asset_read(&deps.storage).load()?;

    let position: PositionState =
        position_read(&deps.storage, &deps.api.canonical_address(&position_owner)?)?;

    if offer_asset_amount > position.asset_amount {
        return Err(StdError::generic_err(
            "The buy amount is bigger than the position amount".to_string(),
        ));
    }

    // load price form the oracle
    let price: Decimal = load_price(
        deps,
        &deps.api.human_address(&asset.oracle)?,
        Some(env.block.time),
    )?;

    let position_value: Uint128 = position.asset_amount * price;
    let auction_begin_treshold = position.collateral_amount * config.auction_threshold_rate;
    if position_value < auction_begin_treshold {
        return Err(StdError::generic_err("Auction is not opened".to_string()));
    }

    // discount = price * (1 + discount_rate); cap return collateral to position colllateral
    let discount_price: Decimal = price + decimal_multiplication(price, config.auction_discount);
    let return_collateral_amount: Uint128 = std::cmp::min(
        offer_asset_amount * discount_price,
        position.collateral_amount,
    );

    let left_asset_amount = (position.asset_amount - offer_asset_amount).unwrap();
    let left_collateral_amount = (position.collateral_amount - return_collateral_amount).unwrap();

    let mut messages: Vec<CosmosMsg> = vec![];
    if left_collateral_amount.is_zero() {
        // all collateral sold out
        position_store(&mut deps.storage)
            .remove(&deps.api.canonical_address(&position_owner)?.as_slice());
    } else if left_asset_amount.is_zero() {
        // all assets paid
        position_store(&mut deps.storage)
            .remove(&deps.api.canonical_address(&position_owner)?.as_slice());

        // refunds left collaterals to position owner
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address.clone(),
            to_address: position_owner.clone(),
            amount: vec![deduct_tax(
                &deps,
                Coin {
                    denom: config.collateral_denom.to_string(),
                    amount: left_collateral_amount,
                },
            )?],
        }));
    } else {
        position_store(&mut deps.storage).set(
            &deps.api.canonical_address(&position_owner)?.as_slice(),
            &to_vec(&PositionState {
                asset_amount: left_asset_amount,
                collateral_amount: left_collateral_amount,
            })?,
        );
    }

    // token burn message
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.human_address(&asset.token)?,
        msg: to_binary(&Cw20HandleMsg::Burn {
            amount: offer_asset_amount,
        })?,
        send: vec![],
    }));

    messages.push(CosmosMsg::Bank(BankMsg::Send {
        from_address: env.contract.address.clone(),
        to_address: sender,
        amount: vec![deduct_tax(
            &deps,
            Coin {
                denom: config.collateral_denom.to_string(),
                amount: return_collateral_amount,
            },
        )?],
    }));

    Ok(HandleResponse {
        log: vec![
            log("action", "auction"),
            log("owner", position_owner.as_str()),
            log(
                "return_amount",
                &(return_collateral_amount.to_string() + config.collateral_denom.as_str()),
            ),
            log(
                "offer_amount",
                &(offer_asset_amount.to_string() + asset.symbol.as_str()),
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
        QueryMsg::Asset {} => to_binary(&query_asset(deps)?),
        QueryMsg::Position { address } => to_binary(&query_position(deps, address)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = config_read(&deps.storage).load()?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        collateral_denom: state.collateral_denom,
        auction_discount: state.auction_discount,
        auction_threshold_rate: state.auction_threshold_rate,
        mint_capacity: state.mint_capacity,
    };

    Ok(resp)
}

pub fn query_asset<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<AssetResponse> {
    let asset: AssetState = asset_read(&deps.storage).load()?;

    let resp = AssetResponse {
        symbol: asset.symbol,
        oracle: deps.api.human_address(&asset.oracle).unwrap(),
        token: deps.api.human_address(&asset.token).unwrap(),
    };

    Ok(resp)
}

pub fn query_position<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> StdResult<PositionResponse> {
    let config: ConfigState = config_read(&deps.storage).load()?;
    let asset: AssetState = asset_read(&deps.storage).load()?;

    let position: PositionState =
        position_read(&deps.storage, &deps.api.canonical_address(&address)?)?;
    let price = load_price(&deps, &deps.api.human_address(&asset.oracle)?, None)?;

    // load price form the oracle
    let position_value: Uint128 = position.asset_amount * price;

    let auction_begin_treshold: Uint128 =
        position.collateral_amount * config.auction_threshold_rate;

    let mut is_auction_open: bool = false;
    if position_value > auction_begin_treshold {
        is_auction_open = true;
    }

    let resp = PositionResponse {
        collateral_amount: position.collateral_amount,
        asset_amount: position.asset_amount,
        is_auction_open,
    };

    Ok(resp)
}

fn is_valid_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    if bytes.len() < 3 || bytes.len() > 6 {
        return false;
    }
    for byte in bytes.iter() {
        if !((*byte >= 65 && *byte <= 90) || (*byte >= 97 && *byte <= 122)) {
            return false;
        }
    }
    true
}

fn deduct_tax<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    coin: Coin,
) -> StdResult<Coin> {
    let terra_querier = TerraQuerier::new(&deps.querier);
    let tax_rate: Decimal = terra_querier.query_tax_rate()?;
    let tax_cap: Uint128 = terra_querier.query_tax_cap(coin.denom.to_string())?;
    Ok(Coin {
        amount: std::cmp::max(
            (coin.amount - coin.amount * tax_rate)?,
            (coin.amount - tax_cap).unwrap_or(Uint128::zero()),
        ),
        ..coin
    })
}
