# Collector Contract

This contract is a rewards collector contract, which converts all collected rewards to `config.staking_token` and send it to `config.distribution_contract`. 

## Features

* Convert
   Swap all given symbol token to `config.collateral_denom` thorugh market(uniswap) contract. It retreives all symbol related infos from `config.factory_contract` with `whitelist` query. If the given symbol is `config.staking_symbol`, it will try to swap all `config.collateral_denom` to `config.staking_symbol`.

   ```json
    {"convert": { "symbol": String } }
   ```
   
   The steps are 
   * Asset Token => Collateral Denom
   * Collateral Denom => Staking

* Send
   Send all collected `config.staking_symbol` to `stkaing_contract` of `staking_symbol` by executing `distribute_reward` operation.
