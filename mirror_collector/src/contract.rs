use cosmwasm_std::{
    log, to_binary, Api, Binary, Coin, CosmosMsg, Env, Extern, HandleResponse, HandleResult,
    InitResponse, Querier, StdResult, Storage, WasmMsg,
};

use crate::msg::{
    ConfigResponse, HandleMsg, InitMsg, MarketHandleMsg, QueryMsg, StakingCw20HookMsg,
};
use crate::querier::{load_balance, load_token_balance, load_whitelist_info, WhitelistInfo};
use crate::state::{read_config, store_config, Config};

use cw20::Cw20HandleMsg;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            deposit_target: deps.api.canonical_address(&msg.deposit_target)?,
            staking_symbol: msg.staking_symbol,
            collateral_denom: msg.collateral_denom,
        },
    )?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Convert { symbol } => try_convert(deps, env, symbol),
        HandleMsg::Send {} => try_send(deps, env),
    }
}

pub fn try_convert<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    symbol: String,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let whitelist_info: WhitelistInfo =
        load_whitelist_info(&deps, &env.contract.address, symbol.to_string())?;

    let mut messages: Vec<CosmosMsg> = vec![];
    if config.staking_symbol == symbol {
        // uusd => staking token
        let amount = load_balance(
            &deps,
            &env.contract.address,
            config.collateral_denom.to_string(),
        )?;

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&whitelist_info.market_contract)?,
            msg: to_binary(&MarketHandleMsg::Buy { max_spread: None })?,
            send: vec![Coin {
                denom: config.collateral_denom,
                amount,
            }],
        }));
    } else {
        // asset token => uusd
        let amount = load_token_balance(
            &deps,
            &deps.api.human_address(&whitelist_info.token_contract)?,
            &deps.api.canonical_address(&env.contract.address)?,
        )?;

        let market_addr = deps.api.human_address(&whitelist_info.market_contract)?;
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&whitelist_info.token_contract)?,
            msg: to_binary(&Cw20HandleMsg::IncreaseAllowance {
                spender: market_addr.clone(),
                amount,
                expires: None,
            })?,
            send: vec![],
        }));

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: market_addr,
            msg: to_binary(&MarketHandleMsg::Sell {
                amount,
                max_spread: None,
            })?,
            send: vec![],
        }));
    }

    Ok(HandleResponse {
        messages,
        log: vec![log("action", "convert"), log("symbol", symbol)],
        data: None,
    })
}

pub fn try_send<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let whitelist_info: WhitelistInfo =
        load_whitelist_info(&deps, &env.contract.address, config.staking_symbol)?;

    let amount = load_token_balance(
        &deps,
        &deps.api.human_address(&whitelist_info.token_contract)?,
        &deps.api.canonical_address(&env.contract.address)?,
    )?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&whitelist_info.token_contract)?,
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: deps.api.human_address(&whitelist_info.staking_contract)?,
                amount,
                msg: Some(to_binary(&StakingCw20HookMsg::DepositReward {})?),
            })?,
            send: vec![],
        })],
        log: vec![log("action", "send"), log("amount", amount.to_string())],
        data: None,
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        deposit_target: deps.api.human_address(&state.deposit_target)?,
        staking_symbol: state.staking_symbol,
        collateral_denom: state.collateral_denom,
    };

    Ok(resp)
}
