# Mint Contract

## Features
Mirrror Protocol mint contract provides following features

* Mint & Burn

    The asset can be minted with some colalteral of `config.collateral_denom`. The contract uses asset oracle to get `price` and `config.mint_capacity` to calculate `mint_amount`.

    It also allows a user to add more collateral to protect the mint poisition from the margin call.

    ```rust
    let total_collateral_amount = position.collateral_amount + new_collateral_amount;
    let asset_amount = total_collateral_amount * price * config.mint_capacity;
    let mint_amount = (asset_amount - position.asset_amount).unwrap_or(Uint128::zero());
    ```

    The contract recognizes the sent coins with `mint` msg as collateral amount.

    ```json
    { "mint": { } }
    ```

    Any minter can burn the minted asset by sending `burn` msg. When liquidating a position, the some part of collateral is returned excluding the collateral required for the remaining position.

    ```rust
    let left_asset_amount = position.asset_amount - burn_amount;
    let collateral_amount = left_asset_amount * price / config.mint_capacity;

    if position.asset_amount == burn amount {
        // return all collateral
        return
    }
    
    if collateral_amount > position.collateral_amount {
        // no refund, just decrease position.asset_amount
        return
    }
     
    // refund collateral
    let refund_collateral_amount = position.collateral_amount - collateral_amount;
    ```

    ```json
    { "burn": { "symbol": "APPL", "amount": "1000000" }
    ```

* Auction

    To prevent the position value from becoming larger than the amount of the collateral, an auction is held. The auction provides collateral as discounted price. Any user can buy as much as they want, capped `position.collateral_amount`.

    The auction is held when,
    
    ```rust
    if position.asset_amount * price >= position.collateral_amount * config.auction_threshold_rate {
        
    }
    ```
    
    The provided asset cannot be bigger than the position's asset amount and also the returned collateral amount cannot be bigger than the position's collateral amount. The discounted colalteral price is calculated as follows:

    ```rust
    let discounted_price = price * (1 + config.auction_discount);
    ```
