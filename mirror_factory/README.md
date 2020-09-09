# Factory Contract

This contract is for mirror token distribution. It continually mint mirror token and distribute minted tokens to staking contract, which is registered with `whitelist` operation.

## Features

* UpdateConfig (Owner)
   It is used when the owner want to update `config.mint_per_block`

* UpdateWeight (Owner)
   It is used to update mint weight of a specific symbol asset

* Whitelist (Owner)
   Append new whitelist item. A owner must specify all required contract infos.

   ```rust
     Whitelist {
        symbol: String,
        weight: Decimal,
        token_contract: HumanAddr,
        mint_contract: HumanAddr,
        market_contract: HumanAddr,
        oracle_contract: HumanAddr,
        staking_contract: HumanAddr,
     }
   ```

* Mint
   Any user can execute mint function. It will calculate mint amount with following equation and send it to predefined `whitelist_info.staking_contract`:

   ```rust
      // mint_amount = weight * mint_per_block * (height - last_height)
      let mint_amount = (config.mint_per_block * distribution_info.weight)
         .multiply_ratio(env.block.height - distribution_info.last_height, 1u64);
   ```
   
