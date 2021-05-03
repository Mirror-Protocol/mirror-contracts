#[cfg(test)]
mod tests {
    use crate::math::short_reward_weight;
    use cosmwasm_std::Decimal;

    #[test]
    fn short_reward_weight_test() {
        let e6 = 1000000u128;
        let e7 = 10000000u128;
        assert_eq!(
            short_reward_weight(Decimal::zero()),
            Decimal::from_ratio(002618u128, e6)
        );
        assert_eq!(
            short_reward_weight(Decimal::percent(1)),
            Decimal::from_ratio(0634168u128, e7),
        );
        assert_eq!(
            short_reward_weight(Decimal::percent(2)),
            Decimal::percent(20)
        );
        assert_eq!(
            short_reward_weight(Decimal::percent(4)),
            Decimal::from_ratio(3908998u128, e7)
        );
        assert_eq!(
            short_reward_weight(Decimal::percent(8)),
            Decimal::percent(40)
        );
        assert_eq!(
            short_reward_weight(Decimal::percent(15)),
            Decimal::percent(40)
        );
    }
}
