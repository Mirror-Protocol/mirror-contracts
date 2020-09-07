# Oracle Contract

This contract is a OCS conformant smart contract. It provides simple interfcae to feed the price from the owner key and allow to change owner. It also provides the variable `price_multiplier` to cope with events like stock split or merge. The oracle users can calculate the active price by multipling `price` and `price_multiplier`.

## Features

* Feed Price

    The owner of oracle contract can feed the price with `feed_price` msg.

    ```json
    { "feed_price": { "price": "1300.0" } }
    ```

* Update Config

    The owner also can do update `price_multiplier` and `owner` by sending `update_config` msg.

    ```json
    { "update_config": { "price_multiplier":  "1.2" } }

    { "update_config": { "owner": "terra~~" } }

    { "update_config": { "owner": "terra~~", "price_multiplier":  "1.2" } }
    ```

    
