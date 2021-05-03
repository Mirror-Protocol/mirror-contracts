#[cfg(test)]
mod tests {
    use crate::math::{short_reward_weight};
    use cosmwasm_std::Decimal;

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
