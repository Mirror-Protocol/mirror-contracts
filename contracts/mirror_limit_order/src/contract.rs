use cosmwasm_std::{
    from_binary, to_binary, Api, Binary, Env, Extern, HandleResponse, HandleResult, InitResponse,
    InitResult, MigrateResponse, MigrateResult, Querier, StdError, StdResult, Storage,
};

use crate::order::{
    cancel_order, execute_order, query_last_order_id, query_order, query_orders, submit_order,
};
use crate::state::init_last_order_id;

use cw20::Cw20ReceiveMsg;
use mirror_protocol::limit_order::{Cw20HookMsg, HandleMsg, InitMsg, MigrateMsg, QueryMsg};
use terraswap::asset::{Asset, AssetInfo};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: InitMsg,
) -> InitResult {
    init_last_order_id(&mut deps.storage)?;
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Receive(msg) => receive_cw20(deps, env, msg),
        HandleMsg::SubmitOrder {
            offer_asset,
            ask_asset,
        } => {
            if !offer_asset.is_native_token() {
                return Err(StdError::generic_err("must provide native token"));
            }

            offer_asset.assert_sent_native_token_balance(&env)?;
            submit_order(deps, env.message.sender, offer_asset, ask_asset)
        }
        HandleMsg::CancelOrder { order_id } => cancel_order(deps, env, order_id),
        HandleMsg::ExecuteOrder {
            execute_asset,
            order_id,
        } => {
            if !execute_asset.is_native_token() {
                return Err(StdError::generic_err("must provide native token"));
            }

            execute_asset.assert_sent_native_token_balance(&env)?;
            execute_order(
                deps,
                env.message.sender,
                env.contract.address,
                execute_asset,
                order_id,
            )
        }
    }
}

pub fn receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> HandleResult {
    if let Some(msg) = cw20_msg.msg {
        let provided_asset = Asset {
            info: AssetInfo::Token {
                contract_addr: env.message.sender,
            },
            amount: cw20_msg.amount,
        };

        match from_binary(&msg)? {
            Cw20HookMsg::SubmitOrder { ask_asset } => {
                submit_order(deps, cw20_msg.sender, provided_asset, ask_asset)
            }
            Cw20HookMsg::ExecuteOrder { order_id } => execute_order(
                deps,
                cw20_msg.sender,
                env.contract.address,
                provided_asset,
                order_id,
            ),
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
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

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> MigrateResult {
    Ok(MigrateResponse::default())
}
