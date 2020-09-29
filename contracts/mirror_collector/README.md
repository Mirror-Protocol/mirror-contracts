# Mirror Collector

This contract is a rewards collector contract, which converts all collected rewards to `config.mirror_token` through terraswap and send it to `config.distribution_contract`. 

## Features

* **Convert**

   It is permissionless function to swap all balance of an asset token to `config.collateral_denom` thorugh terraswap contract. It retreives terraswap pair(`config.distribution_contract`<>`asset_token`) contract address from the `config.terraswap_factory`. If the given asset token is `config.mirror_token`, it swaps all `config.collateral_denom` to `config.mirror_token`.

   ```json
    {"convert": { "asset_token": HumanAddr } }
   ```
   
   The steps are 
   * Asset Token => Collateral Denom
   * Collateral Denom => Mirror Token

* **Send**

   Send all balance of the `config.mirror_token` to `config.distribution_contract`.
