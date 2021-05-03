#[cfg(test)]
mod tests {
    use crate::math::{erf_plus_one, short_reward_weight};
    use cosmwasm_std::Decimal;

    #[test]
    fn erf_plus_one_test() {
        let e6 = 1000000u128;
        assert_eq!(erf_plus_one(Decimal::zero()), Decimal::zero());
        assert_eq!(
            erf_plus_one(Decimal::one()),
            Decimal::from_ratio(013090u128, e6)
        );
        assert_eq!(
            erf_plus_one(Decimal::from_ratio(3u128, 1u128)),
            Decimal::from_ratio(1000000u128, e6)
        );
        assert_eq!(
            erf_plus_one(Decimal::from_ratio(5u128, 1u128)),
            Decimal::from_ratio(1954499u128, e6)
        );
        assert_eq!(
            erf_plus_one(Decimal::from_ratio(10u128, 1u128)),
            Decimal::from_ratio(2000000u128, e6)
        );
        assert_eq!(
            erf_plus_one(Decimal::from_ratio(14u128, 1u128)),
            Decimal::from_ratio(2000000u128, e6)
        );
    }

    #[test]
    fn short_reward_weight_test() {
        let e6 = 1000000u128;
        let e7 = 10000000u128;
        assert_eq!(short_reward_weight(Decimal::zero()), Decimal::zero());
        assert_eq!(
            short_reward_weight(Decimal::percent(1)),
            Decimal::from_ratio(002618u128, e6),
        );
        assert_eq!(
            short_reward_weight(Decimal::percent(3)),
            Decimal::percent(20)
        );
        assert_eq!(
            short_reward_weight(Decimal::percent(5)),
            Decimal::from_ratio(3908998u128, e7)
        );
        assert_eq!(
            short_reward_weight(Decimal::percent(10)),
            Decimal::percent(40)
        );
        assert_eq!(
            short_reward_weight(Decimal::percent(15)),
            Decimal::percent(40)
        );
    }
}
