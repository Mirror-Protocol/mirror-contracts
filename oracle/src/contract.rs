use cosmwasm_std::{
    log, to_binary, Api, Binary, Decimal, Env, Extern, HandleResponse, HumanAddr, InitResponse,
    Querier, StdError, StdResult, Storage,
};

use crate::msg::{ConfigResponse, HandleMsg, InitMsg, PriceResponse, QueryMsg};

use crate::state::{config_read, config_store, price_read, price_store, ConfigState, PriceState};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let api = deps.api;
    config_store(&mut deps.storage).save(&ConfigState {
        owner: api.canonical_address(&env.message.sender)?,
        asset_token: api.canonical_address(&msg.asset_token)?,
        base_denom: msg.base_denom.to_string(),
        quote_denom: msg.quote_denom.to_string(),
    })?;

    price_store(&mut deps.storage).save(&PriceState {
        price: Decimal::zero(),
        price_multiplier: Decimal::one(),
        last_update_time: 0u64,
    })?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::FeedPrice { price } => try_feed_price(deps, env, price),
        HandleMsg::UpdateConfig {
            owner,
            asset_token,
            base_denom,
            quote_denom,
            price_multiplier,
        } => try_update_config(
            deps,
            env,
            owner,
            asset_token,
            base_denom,
            quote_denom,
            price_multiplier,
        ),
    }
}

pub fn try_feed_price<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    price: Decimal,
) -> StdResult<HandleResponse> {
    let config_state = config_store(&mut deps.storage).load()?;
    if deps.api.canonical_address(&env.message.sender)? != config_state.owner {
        return Err(StdError::unauthorized());
    }

    price_store(&mut deps.storage).update(|mut price_state| {
        price_state.price = price;
        price_state.last_update_time = env.block.time;
        Ok(price_state)
    })?;

    let res = HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "price_feed"),
            log("price", &price.to_string()),
        ],
        data: None,
    };

    Ok(res)
}

pub fn try_update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    asset_token: Option<HumanAddr>,
    base_denom: Option<String>,
    quote_denom: Option<String>,
    price_multiplier: Option<Decimal>,
) -> StdResult<HandleResponse> {
    let api = deps.api;
    config_store(&mut deps.storage).update(|mut state| {
        if api.canonical_address(&env.message.sender)? != state.owner {
            return Err(StdError::unauthorized());
        }

        if let Some(owner) = owner {
            state.owner = api.canonical_address(&owner)?;
        }

        if let Some(asset_token) = asset_token {
            state.asset_token = api.canonical_address(&asset_token)?;
        }

        if let Some(base_denom) = base_denom {
            state.base_denom = base_denom;
        }

        if let Some(quote_denom) = quote_denom {
            state.quote_denom = quote_denom;
        }

        Ok(state)
    })?;

    if let Some(price_multiplier) = price_multiplier {
        price_store(&mut deps.storage).update(|mut price_state| {
            price_state.price_multiplier = price_multiplier;
            Ok(price_state)
        })?;
    }

    Ok(HandleResponse::default())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Price {} => to_binary(&query_price(deps)?),
    }
}

fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = config_read(&deps.storage).load()?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        asset_token: deps.api.human_address(&state.asset_token)?,
        base_denom: state.base_denom.to_string(),
        quote_denom: state.quote_denom.to_string(),
    };

    Ok(resp)
}

fn query_price<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> StdResult<PriceResponse> {
    let state = price_read(&deps.storage).load()?;
    let resp = PriceResponse {
        price: state.price,
        price_multiplier: state.price_multiplier,
        last_update_time: state.last_update_time,
    };

    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{from_binary, BlockInfo, Env, StdError};
    use std::str::FromStr;

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            asset_token: HumanAddr("asset0000".to_string()),
            base_denom: "base0000".to_string(),
            quote_denom: "quote0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let value = query_config(&deps).unwrap();
        assert_eq!("addr0000", value.owner.as_str());
        assert_eq!("asset0000", value.asset_token.as_str());
        assert_eq!("base0000", value.base_denom.as_str());
        assert_eq!("quote0000", value.quote_denom.as_str());
    }

    #[test]
    fn update_config() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            asset_token: HumanAddr("asset0000".to_string()),
            base_denom: "base0000".to_string(),
            quote_denom: "quote0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        // update owner
        let env = mock_env("addr0000", &[]);
        let msg = HandleMsg::UpdateConfig {
            owner: Some(HumanAddr("addr0001".to_string())),
            asset_token: None,
            base_denom: None,
            quote_denom: None,
            price_multiplier: None,
        };

        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let value = query_config(&deps).unwrap();
        assert_eq!("addr0001", value.owner.as_str());
        assert_eq!("asset0000", value.asset_token.as_str());
        assert_eq!("base0000", value.base_denom.as_str());
        assert_eq!("quote0000", value.quote_denom.as_str());

        // update left items
        let env = mock_env("addr0001", &[]);
        let msg = HandleMsg::UpdateConfig {
            owner: None,
            asset_token: Some(HumanAddr("asset0001".to_string())),
            base_denom: Some("base0001".to_string()),
            quote_denom: Some("quote0001".to_string()),
            price_multiplier: Some(Decimal::from_ratio(101u128, 10u128)),
        };

        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let value = query_config(&deps).unwrap();
        assert_eq!("addr0001", value.owner.as_str());
        assert_eq!("asset0001", value.asset_token.as_str());
        assert_eq!("base0001", value.base_denom.as_str());
        assert_eq!("quote0001", value.quote_denom.as_str());
        let value = query_price(&deps).unwrap();
        assert_eq!(Decimal::from_ratio(101u128, 10u128), value.price_multiplier);

        // Unauthorzied err
        let env = mock_env("addr0000", &[]);
        let msg = HandleMsg::UpdateConfig {
            owner: None,
            asset_token: None,
            base_denom: None,
            quote_denom: None,
            price_multiplier: None,
        };

        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }
    }

    #[test]
    fn feed_price() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            asset_token: HumanAddr("asset0000".to_string()),
            base_denom: "base0000".to_string(),
            quote_denom: "quote0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        // update price
        let env = mock_env("addr0000", &[]);
        let env = Env {
            block: BlockInfo {
                height: 1,
                time: 123,
                chain_id: "columbus".to_string(),
            },
            ..env
        };

        let msg = HandleMsg::FeedPrice {
            price: Decimal::from_str("1.2").unwrap(),
        };

        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let value: PriceResponse = query_price(&deps).unwrap();
        assert_eq!("1.2", format!("{}", value.price));
        assert_eq!(123u64, value.last_update_time);
        assert_eq!(Decimal::one(), value.price_multiplier);

        // Unautorized try
        let env = mock_env("addr0001", &[]);
        let msg = HandleMsg::FeedPrice {
            price: Decimal::from_str("1.2").unwrap(),
        };

        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }
    }

    #[test]
    fn can_query_config() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            asset_token: HumanAddr("asset0000".to_string()),
            base_denom: "base0000".to_string(),
            quote_denom: "quote0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        let query_msg = QueryMsg::Config {};
        let query_result = query(&deps, query_msg).unwrap();
        let value: ConfigResponse = from_binary(&query_result).unwrap();
        assert_eq!("addr0000", value.owner.as_str());
        assert_eq!("asset0000", value.asset_token.as_str());
        assert_eq!("base0000", value.base_denom.as_str());
    }

    #[test]
    fn can_query_price() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            asset_token: HumanAddr("asset0000".to_string()),
            base_denom: "base0000".to_string(),
            quote_denom: "quote0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        // update price
        let env = mock_env("addr0000", &[]);
        let msg = HandleMsg::FeedPrice {
            price: Decimal::from_str("1.2").unwrap(),
        };

        let _res = handle(&mut deps, env, msg).unwrap();

        let query_msg = QueryMsg::Price {};
        let query_result = query(&deps, query_msg).unwrap();
        let value: PriceResponse = from_binary(&query_result).unwrap();
        assert_eq!("1.2", format!("{}", value.price));
        assert_eq!(Decimal::one(), value.price_multiplier);
    }
}
