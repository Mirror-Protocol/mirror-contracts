use cosmwasm_std::{
    log, Api, Decimal, Env, Extern, HandleResponse, HandleResult, HumanAddr, Querier, StdError,
    StdResult, Storage, Uint128,
};

use terraswap::asset::Asset;

use crate::state::{
    increase_last_order_id, read_last_order_id, read_order, read_orders,
    read_orders_with_bidder_indexer, remove_order, store_order, Order,
};
use mirror_protocol::common::OrderBy;
use mirror_protocol::limit_order::{LastOrderIdResponse, OrderResponse, OrdersResponse};

pub fn submit_order<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    sender: HumanAddr,
    offer_asset: Asset,
    ask_asset: Asset,
) -> HandleResult {
    let order_id = increase_last_order_id(&mut deps.storage)?;

    let offer_asset_raw = offer_asset.to_raw(&deps)?;
    let ask_asset_raw = ask_asset.to_raw(&deps)?;
    store_order(
        &mut deps.storage,
        &Order {
            order_id,
            bidder_addr: deps.api.canonical_address(&sender)?,
            offer_asset: offer_asset_raw,
            ask_asset: ask_asset_raw,
            filled_offer_amount: Uint128::zero(),
            filled_ask_amount: Uint128::zero(),
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "submit_order"),
            log("order_id", order_id),
            log("bidder_addr", sender),
            log("offer_asset", offer_asset.to_string()),
            log("ask_asset", ask_asset.to_string()),
        ],
        data: None,
    })
}

pub fn cancel_order<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    order_id: u64,
) -> HandleResult {
    let order: Order = read_order(&deps.storage, order_id)?;
    if order.bidder_addr != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    // Compute refund asset
    let left_offer_amount = (order.offer_asset.amount - order.filled_offer_amount)?;
    let bidder_refund = Asset {
        info: order.offer_asset.info.to_normal(&deps)?,
        amount: left_offer_amount,
    };

    // Build refund msg
    let messages = if left_offer_amount > Uint128::zero() {
        vec![bidder_refund
            .clone()
            .into_msg(&deps, env.contract.address, env.message.sender)?]
    } else {
        vec![]
    };

    remove_order(&mut deps.storage, &order);

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "cancel_order"),
            log("order_id", order_id),
            log("bidder_refund", bidder_refund),
        ],
        data: None,
    })
}

pub fn execute_order<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    sender: HumanAddr,
    contract_addr: HumanAddr,
    execute_asset: Asset,
    order_id: u64,
) -> HandleResult {
    let mut order: Order = read_order(&deps.storage, order_id)?;
    if !execute_asset
        .info
        .equal(&order.ask_asset.info.to_normal(&deps)?)
    {
        return Err(StdError::generic_err("invalid asset given"));
    }

    // Compute left offer & ask amount
    let left_offer_amount = (order.offer_asset.amount - order.filled_offer_amount)?;
    let left_ask_amount = (order.ask_asset.amount - order.filled_ask_amount)?;
    if left_ask_amount < execute_asset.amount || left_offer_amount.is_zero() {
        return Err(StdError::generic_err("insufficient order amount left"));
    }

    // cap the send amount to left_offer_amount
    let executor_receive = Asset {
        info: order.offer_asset.info.to_normal(&deps)?,
        amount: std::cmp::min(
            left_offer_amount,
            execute_asset.amount
                * Decimal::from_ratio(order.offer_asset.amount, order.ask_asset.amount),
        ),
    };

    let bidder_addr = deps.api.human_address(&order.bidder_addr)?;
    let bidder_receive = execute_asset;

    // When left amount is zero, close order
    if left_ask_amount == bidder_receive.amount {
        remove_order(&mut deps.storage, &order);
    } else {
        order.filled_ask_amount = order.filled_ask_amount + bidder_receive.amount;
        order.filled_offer_amount = order.filled_offer_amount + executor_receive.amount;
        store_order(&mut deps.storage, &order)?;
    }

    Ok(HandleResponse {
        messages: vec![
            executor_receive
                .clone()
                .into_msg(&deps, contract_addr.clone(), sender)?,
            bidder_receive
                .clone()
                .into_msg(&deps, contract_addr, bidder_addr)?,
        ],
        log: vec![
            log("action", "execute_order"),
            log("order_id", order_id),
            log("executor_receive", executor_receive),
            log("bidder_receive", bidder_receive),
        ],
        data: None,
    })
}

pub fn query_order<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    order_id: u64,
) -> StdResult<OrderResponse> {
    let order: Order = read_order(&deps.storage, order_id)?;
    let resp = OrderResponse {
        order_id: order.order_id,
        bidder_addr: deps.api.human_address(&order.bidder_addr)?,
        offer_asset: order.offer_asset.to_normal(&deps)?,
        ask_asset: order.ask_asset.to_normal(&deps)?,
        filled_offer_amount: order.filled_offer_amount,
        filled_ask_amount: order.filled_ask_amount,
    };

    Ok(resp)
}

pub fn query_orders<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    bidder_addr: Option<HumanAddr>,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<OrdersResponse> {
    let orders: Vec<Order> = if let Some(bidder_addr) = bidder_addr {
        let bidder_addr_raw = deps.api.canonical_address(&bidder_addr)?;
        read_orders_with_bidder_indexer(
            &deps.storage,
            &bidder_addr_raw,
            start_after,
            limit,
            order_by,
        )?
    } else {
        read_orders(&deps.storage, start_after, limit, order_by)?
    };

    let resp = OrdersResponse {
        orders: orders
            .iter()
            .map(|order| {
                Ok(OrderResponse {
                    order_id: order.order_id,
                    bidder_addr: deps.api.human_address(&order.bidder_addr)?,
                    offer_asset: order.offer_asset.to_normal(&deps)?,
                    ask_asset: order.ask_asset.to_normal(&deps)?,
                    filled_offer_amount: order.filled_offer_amount,
                    filled_ask_amount: order.filled_ask_amount,
                })
            })
            .collect::<StdResult<Vec<OrderResponse>>>()?,
    };

    Ok(resp)
}

pub fn query_last_order_id<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<LastOrderIdResponse> {
    let last_order_id = read_last_order_id(&deps.storage)?;
    let resp = LastOrderIdResponse { last_order_id };

    Ok(resp)
}
