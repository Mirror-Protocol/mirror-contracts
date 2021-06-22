use crate::math::{
    decimal_division, decimal_multiplication, decimal_subtraction, erf_plus_one, Sign,
};
use cosmwasm_std::{
    to_binary, Api, Binary, Decimal, Env, Extern, HandleResponse, InitResponse, Querier, StdResult,
    Storage,
};
use mirror_protocol::short_reward::{HandleMsg, InitMsg, QueryMsg, ShortRewardWeightResponse};

pub fn init<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: InitMsg,
) -> StdResult<InitResponse> {
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: HandleMsg,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::ShortRewardWeight { premium_rate } => {
            to_binary(&query_short_reward_weight(deps, premium_rate)?)
        }
    }
}

pub fn query_short_reward_weight<S: Storage, A: Api, Q: Querier>(
    _deps: &Extern<S, A, Q>,
    premium_rate: Decimal,
) -> StdResult<ShortRewardWeightResponse> {
    if premium_rate > Decimal::percent(7) {
        return Ok(ShortRewardWeightResponse {
            short_reward_weight: Decimal::percent(40),
        });
    }

    let one = Decimal::one();
    let two = one + one;
    let e10 = 10000000000u128;
    let sqrt_two = Decimal::from_ratio(14142135624u128, e10);

    let p = decimal_multiplication(premium_rate, Decimal::from_ratio(100u128, 1u128));
    let (sign_x, x) = if p > two {
        (
            Sign::Positive,
            decimal_division(decimal_subtraction(p, two), sqrt_two),
        )
    } else {
        (
            Sign::Negative,
            decimal_division(decimal_subtraction(two, p), sqrt_two),
        )
    };

    let short_reward_weight: Decimal =
        decimal_division(erf_plus_one(sign_x, x), Decimal::from_ratio(5u128, 1u128));
    return Ok(ShortRewardWeightResponse {
        short_reward_weight,
    });
}
