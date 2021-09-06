#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};

use crate::order::{
    cancel_order, execute_order, query_last_order_id, query_order, query_orders, submit_order,
};
use crate::state::init_last_order_id;

use cw20::Cw20ReceiveMsg;
use mirror_protocol::limit_order::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use terraswap::asset::{Asset, AssetInfo};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    init_last_order_id(deps.storage)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::SubmitOrder {
            offer_asset,
            ask_asset,
        } => {
            if !offer_asset.is_native_token() {
                return Err(StdError::generic_err("must provide native token"));
            }

            offer_asset.assert_sent_native_token_balance(&info)?;
            submit_order(deps, info.sender, offer_asset, ask_asset)
        }
        ExecuteMsg::CancelOrder { order_id } => cancel_order(deps, info, order_id),
        ExecuteMsg::ExecuteOrder {
            execute_asset,
            order_id,
        } => {
            if !execute_asset.is_native_token() {
                return Err(StdError::generic_err("must provide native token"));
            }

            execute_asset.assert_sent_native_token_balance(&info)?;
            execute_order(deps, info.sender, execute_asset, order_id)
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let sender = deps.api.addr_validate(cw20_msg.sender.as_str())?;

    let provided_asset = Asset {
        info: AssetInfo::Token {
            contract_addr: info.sender.to_string(),
        },
        amount: cw20_msg.amount,
    };

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::SubmitOrder { ask_asset }) => {
            submit_order(deps, sender, provided_asset, ask_asset)
        }
        Ok(Cw20HookMsg::ExecuteOrder { order_id }) => {
            execute_order(deps, sender, provided_asset, order_id)
        }
        Err(_) => Err(StdError::generic_err("invalid cw20 hook message")),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Order { order_id } => to_binary(&query_order(deps, order_id)?),
        QueryMsg::Orders {
            bidder_addr,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_orders(
            deps,
            bidder_addr,
            start_after,
            limit,
            order_by,
        )?),
        QueryMsg::LastOrderId {} => to_binary(&query_last_order_id(deps)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
