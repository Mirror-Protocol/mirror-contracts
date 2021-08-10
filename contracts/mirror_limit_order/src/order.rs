use cosmwasm_std::{
    attr, Addr, CosmosMsg, Decimal, Deps, DepsMut, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use terraswap::asset::Asset;

use crate::state::{
    increase_last_order_id, read_last_order_id, read_order, read_orders,
    read_orders_with_bidder_indexer, remove_order, store_order, Order,
};
use mirror_protocol::common::OrderBy;
use mirror_protocol::limit_order::{LastOrderIdResponse, OrderResponse, OrdersResponse};

pub fn submit_order(
    deps: DepsMut,
    sender: Addr,
    offer_asset: Asset,
    ask_asset: Asset,
) -> StdResult<Response> {
    let order_id = increase_last_order_id(deps.storage)?;

    let offer_asset_raw = offer_asset.to_raw(deps.api)?;
    let ask_asset_raw = ask_asset.to_raw(deps.api)?;
    store_order(
        deps.storage,
        &Order {
            order_id,
            bidder_addr: deps.api.addr_canonicalize(&sender.as_str())?,
            offer_asset: offer_asset_raw,
            ask_asset: ask_asset_raw,
            filled_offer_amount: Uint128::zero(),
            filled_ask_amount: Uint128::zero(),
        },
    )?;

    Ok(Response {
        messages: vec![],
        submessages: vec![],
        attributes: vec![
            attr("action", "submit_order"),
            attr("order_id", order_id),
            attr("bidder_addr", sender),
            attr("offer_asset", offer_asset.to_string()),
            attr("ask_asset", ask_asset.to_string()),
        ],
        data: None,
    })
}

pub fn cancel_order(deps: DepsMut, info: MessageInfo, order_id: u64) -> StdResult<Response> {
    let order: Order = read_order(deps.storage, order_id)?;
    if order.bidder_addr != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    // Compute refund asset
    let left_offer_amount = order
        .offer_asset
        .amount
        .checked_sub(order.filled_offer_amount)?;
    let bidder_refund = Asset {
        info: order.offer_asset.info.to_normal(deps.api)?,
        amount: left_offer_amount,
    };

    // Build refund msg
    let messages = if left_offer_amount > Uint128::zero() {
        vec![bidder_refund.clone().into_msg(&deps.querier, info.sender)?]
    } else {
        vec![]
    };

    remove_order(deps.storage, &order);

    Ok(Response {
        messages,
        submessages: vec![],
        attributes: vec![
            attr("action", "cancel_order"),
            attr("order_id", order_id),
            attr("bidder_refund", bidder_refund),
        ],
        data: None,
    })
}

pub fn execute_order(
    deps: DepsMut,
    sender: Addr,
    execute_asset: Asset,
    order_id: u64,
) -> StdResult<Response> {
    let mut order: Order = read_order(deps.storage, order_id)?;
    if !execute_asset
        .info
        .equal(&order.ask_asset.info.to_normal(deps.api)?)
    {
        return Err(StdError::generic_err("invalid asset given"));
    }

    // Compute left offer & ask amount
    let left_offer_amount = order
        .offer_asset
        .amount
        .checked_sub(order.filled_offer_amount)?;
    let left_ask_amount = order
        .ask_asset
        .amount
        .checked_sub(order.filled_ask_amount)?;
    if left_ask_amount < execute_asset.amount || left_offer_amount.is_zero() {
        return Err(StdError::generic_err("insufficient order amount left"));
    }

    // Cap the send amount to left_offer_amount
    let executor_receive = Asset {
        info: order.offer_asset.info.to_normal(deps.api)?,
        amount: if left_ask_amount == execute_asset.amount {
            left_offer_amount
        } else {
            std::cmp::min(
                left_offer_amount,
                execute_asset.amount
                    * Decimal::from_ratio(order.offer_asset.amount, order.ask_asset.amount),
            )
        },
    };

    let bidder_addr = deps.api.addr_humanize(&order.bidder_addr)?;
    let bidder_receive = execute_asset;

    // When left amount is zero, close order
    if left_ask_amount == bidder_receive.amount {
        remove_order(deps.storage, &order);
    } else {
        order.filled_ask_amount = order.filled_ask_amount + bidder_receive.amount;
        order.filled_offer_amount = order.filled_offer_amount + executor_receive.amount;
        store_order(deps.storage, &order)?;
    }

    let mut messages: Vec<CosmosMsg> = vec![];
    if !executor_receive.amount.is_zero() {
        messages.push(
            executor_receive
                .clone()
                .into_msg(&deps.querier, deps.api.addr_validate(sender.as_str())?)?,
        );
    }

    if !bidder_receive.amount.is_zero() {
        messages.push(
            bidder_receive
                .clone()
                .into_msg(&deps.querier, bidder_addr)?,
        );
    }

    Ok(Response {
        messages,
        submessages: vec![],
        attributes: vec![
            attr("action", "execute_order"),
            attr("order_id", order_id),
            attr("executor_receive", executor_receive),
            attr("bidder_receive", bidder_receive),
        ],
        data: None,
    })
}

pub fn query_order(deps: Deps, order_id: u64) -> StdResult<OrderResponse> {
    let order: Order = read_order(deps.storage, order_id)?;
    let resp = OrderResponse {
        order_id: order.order_id,
        bidder_addr: deps.api.addr_humanize(&order.bidder_addr)?.to_string(),
        offer_asset: order.offer_asset.to_normal(deps.api)?,
        ask_asset: order.ask_asset.to_normal(deps.api)?,
        filled_offer_amount: order.filled_offer_amount,
        filled_ask_amount: order.filled_ask_amount,
    };

    Ok(resp)
}

pub fn query_orders(
    deps: Deps,
    bidder_addr: Option<String>,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<OrdersResponse> {
    let orders: Vec<Order> = if let Some(bidder_addr) = bidder_addr {
        let bidder_addr_raw = deps.api.addr_canonicalize(&bidder_addr)?;
        read_orders_with_bidder_indexer(
            deps.storage,
            &bidder_addr_raw,
            start_after,
            limit,
            order_by,
        )?
    } else {
        read_orders(deps.storage, start_after, limit, order_by)?
    };

    let resp = OrdersResponse {
        orders: orders
            .iter()
            .map(|order| {
                Ok(OrderResponse {
                    order_id: order.order_id,
                    bidder_addr: deps.api.addr_humanize(&order.bidder_addr)?.to_string(),
                    offer_asset: order.offer_asset.to_normal(deps.api)?,
                    ask_asset: order.ask_asset.to_normal(deps.api)?,
                    filled_offer_amount: order.filled_offer_amount,
                    filled_ask_amount: order.filled_ask_amount,
                })
            })
            .collect::<StdResult<Vec<OrderResponse>>>()?,
    };

    Ok(resp)
}

pub fn query_last_order_id(deps: Deps) -> StdResult<LastOrderIdResponse> {
    let last_order_id = read_last_order_id(deps.storage)?;
    let resp = LastOrderIdResponse { last_order_id };

    Ok(resp)
}
