# Mirror Factory

This contract is for mirror token distribution. It continually mints mirror token and distributes minted tokens to pools in staking contract, which are registered with `whitelist` operation.

## Configs

| Name                 | Description                                                                             |
| -------------------- | --------------------------------------------------------------------------------------- |
| mirror_token         | Mirror token contract address                                                           |
| mint_contract        | The contract address which has minter permission of the created asset token             |
| oracle_contract      | The contract address which is used to feed asset price                                  |
| uniswap_factory      | The contract address which creates uniswap pair contract when a new asset is registered |
| staking_contract     | The contract address which provides staking pools for liqudity(LP) token                |
| commission_collector | The contract address which collects all uniswap owner commission                        |
| mint_per_block       | The amount of mirror token to mint per block for each 1 weight                          |
| token_code_id        | The code ID for asset token                                                             |
| base_denom           | The native token denom used to create uniswap pair                                      |

## HandleMsgs

* PostInitialize (Owner)

   This operation is used to register all relevant contracts `uniswap_factory`, `mirror_token`, `staking_contract`, `oracle_contract`, `mint_contract`, `commission_collector`. Only owner is allowed to execute it.

   **Request Format**
   ```rust
   pub enum HandleMsg {
      PostInitialize {
         owner: HumanAddr,
         uniswap_factory: HumanAddr,
         mirror_token: HumanAddr,
         staking_contract: HumanAddr,
         oracle_contract: HumanAddr,
         mint_contract: HumanAddr,
         commission_collector: HumanAddr,
      }
   }
   ```

* UpdateConfig (Owner)

   A owner can update `mint_per_block` or `token_code_id`. 

   **Request Format**
   ```rust
   pub enum HandleMsg {
      UpdateConfig {
         owner: Option<HumanAddr>,
         mint_per_block: Option<Uint128>,
         token_code_id: Option<u64>,
      },
   }
   ```

* UpdateWeight (Owner)

   A owner can update mint weight of a specific symbol asset

   **Request Format**
   ```rust
   pub enum HandleMsg {
      UpdateWeight {
         asset_token: HumanAddr,
         weight: Decimal,
      },
   }
   ```

* Whitelist (Owner)

   Whitelisting is processed in following order:
   1. Create asset token contract with `config.token_code_id` with `minter` argument
   
   2. Call `TokenCreationHook`
   
      2-1. Initialize distribution info

      2-2. Register asset to mint contract

      2-3. Register asset and oracle feeder 
      to oracle contract

      2-4. Create uniswap pair through uniswap factory

   3. Call `UniswapCreationHook`
   
      3-1. Register asset and liquidity(LP) token to staking contract

   **Request Format**
   ```rust
   pub enum HandleMsg {
      Whitelist {
         /// asset name used to create token contract
         name: String,
         /// asset symbol used to create token contract
         symbol: String,
         /// authorized asset oracle feeder
         oracle_feeder: HumanAddr,
         /// used to create all necessary contract or register asset
         params: Params,
      }
   }
      

   pub struct Params {
      /// inflation weight
      pub weight: Decimal,
      /// Commission rate for active liquidit   provider
      pub lp_commission: Decimal,
      /// Commission rate for owner controlled     commission
      pub owner_commission: Decimal,
      /// Auction discount rate applied to asse   mint
      pub auction_discount: Decimal,
      /// Minium collateral ratio applied to asse   mint
      pub min_collateral_ratio: Decimal,
   }
   ```

* PassCommand (Owner)

   Owner can pass any message to any contract with this message. The factory has many ownership privilege, so this interface is for allowing a owner to exert ownership over the child contracts.

   **Request Format**
   ```rust
   pub enum HandleMsg {
      PassCommand {
         contract_addr: HumanAddr,
         msg: Binary,
      },
   }
   ```

* Mint
  
   Anyone can execute mint function with a specify asset token argument. The mint amount is calculated with following equation and send it to `staking_contract`'s asset token pool:

   ```rust
      // mint_amount = weight * mint_per_block * (height - last_height)
      let mint_amount = (config.mint_per_block * distribution_info.weight)
         .multiply_ratio(env.block.height - distribution_info.last_height, 1u64);
   ```

   **Request Format**
   ```rust
   pub enum HandleMsg {
      Mint {
         asset_token: HumanAddr,
      },
   }
   ```
   
