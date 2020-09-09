# Market Contract

## Features
* Initialize
    Contract owner must initialize all required token contracts and oracle contracts with proper parameters. Only swap related params can be updated later with owner key.

    ```rust
    {
        /// Liquidity token, required to withdraw liquidity position
        pub liquidity_token: HumanAddr,
        /// Inactive commission collector
        pub commission_collector: HumanAddr,
        /// Asset token address
        pub asset_token: HumanAddr,
        /// Asset oracle address
        pub asset_oracle: HumanAddr,
        /// Asset symbol
        pub asset_symbol: String,
        /// Collateral denom
        pub collateral_denom: String,
        /// Commission rate for active liquidity provider
        pub active_commission: Decimal,
        /// Commission rate for mirror token stakers
        pub inactive_commission: Decimal,
    }
    ```
* UpdateConfig
    
    The market contract owner always can change pool configuration with `update_config` msg.
    
    ```json
    {
        "update_config": 
        {
            "owner": Option<HumanAddr>,
            "active_commission": Option<Decimal>,
            "inactive_commission": Option<Decimal>,
        }
    }
    ```

* Provide Liquidity

    The contract has two types of pool, the one is collateral and the other is asset pool. A user can provide liquidity to each pool by sending `provide_liquidity` msgs and also can withdraw with `withdraw_liquidity` msgs. 

    Whenever liquidity is deposited into a pool, special tokens known as liquidity tokens are minted to the provider’s address, in proportion to how much liquidity they contributed to the pool. These tokens are a representation of a liquidity provider’s contribution to a pool. Whenever a trade occurs, the `active_commission%` of fee is distributed pro-rata to all LPs in the pool at the moment of the trade. To receive the underlying liquidity back, plus any fees that were accrued while their liquidity was locked, LPs must burn their liquidity tokens.

    When providing liquidity from a smart contract, the most important thing to keep in mind is that tokens deposited into a pool at any rate other than the current oracle price ratio are vulnerable to being arbitraged. As an example, if the ratio of x:y in a pair is 10:2 (i.e. the price is 5), and someone naively adds liquidity at 5:2 (a price of 2.5), the contract will simply accept all tokens (changing the price to 3.75 and opening up the market to arbitrage), but only issue pool tokens entitling the sender to the amount of assets sent at the proper ratio, in this case 5:1. To avoid donating to arbitrageurs, it is imperative to add liquidity at the current price. Luckily, it’s easy to ensure that this condition is met!

    > Note before executing the `provide_liqudity` operation, a user must allow the contract to use the liquidity amount of asset in the token contract.

    ```json
    { "provide_liquidity": { "coins": [{"denom": "APPL", "amount": "1000000"}]} }
    { "withdraw_liquidity": { "amount": "1000000" } }
    ```

* Buy & Sell

    Any user can buy &sell the asset by sending `buy` or `sell` msg.

    ```json
    { "buy ": { "max_spread": Option<Decimal> } }
    ```
    ```json
    { "sell": {"amount": Uint128, "max_spread": Option<Decimal>}}
    ```

    The spread is decidied by following uniswap-like mechanism:

    ```rust
    // -max_minus_spread < spread < max_spread
    // minus_spread means discount rate.
    // Ensure `asset pool * collateral pool = constant product`
    let cp = Uint128(offer_pool.u128() * ask_pool.u128());
    let return_amount = offer_amount * exchange_rate;
    let return_amount = (ask_pool - cp.multiply_ratio(1u128, offer_pool + offer_amount))?;


    // calculate spread & commission
    let spread_amount: Uint128 =
        (offer_amount * Decimal::from_ratio(ask_pool, offer_pool) - return_amount)?;
    let active_commission: Uint128 = return_amount * config.active_commission;
    let inactive_commission: Uint128 = return_amount * config.inactive_commission;

    // commission will be absorbed to pool
    let return_amount: Uint128 =
        (return_amount - (active_commission + inactive_commission)).unwrap();
    ```

    The `inactive_commssion` fees are transferred to a reward collector contract and distributed as staking rewards.
