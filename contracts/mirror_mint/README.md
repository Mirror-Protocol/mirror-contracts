# Mint Contract
The Mint Contract provides a permissionless Collateralized Debt Position (CDP) over mirror assets. Anyone can open or close CDP with dedicated mirror asssets or pre-defined native token as collateral.

## Configs

### General Config
| Name            | Description                                     |
| --------------- | ----------------------------------------------- |
| owner           | The owner address who can update the configs    |
| oracle          | The oracle contract address is for price lookup |
| base_asset_info | The asset info the oracle price is based on     |
| token_code_id   | The token contract id for asset token contract  |

### Asset Config
| Name                 | Description                                                                               |
| -------------------- | ----------------------------------------------------------------------------------------- |
| token                | The token contract addres                                                                 |
| auction_discount     | When the auction took place, the system sell the collateral asset with this discount rate |
| min_collateral_ratio | All CDP have collateral ratio bigger than this config. If not, the auction takes place.   |


## Handlers

### Open Position
Anyone can register a position without permission, and the registered position will receive a global uniquip id. When opening a position, the user can specify the collateral and the target asset to mint and set the desired collateral ratio. According to the given input, the quantity of assets that satisfy the collateral ratio is minted.

```rust
let mint_amount = collateral.amount
    * collateral_price
    * reverse_decimal(asset_price)
    * reverse_decimal(collateral_ratio);
```

Request Format
```rust
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    OpenPosition {
        collateral: Asset,
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
    }
}

pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Token { contract_addr: HumanAddr },
    NativeToken { denom: String },
}
```

### Deposit Collateral
Users must keep their positions safe to prevent margin calls. In order to do that, the user must be able to increase the collateral on the position. This operation is only for increase the collateral amount of a position. 

The collateral can be both native token and cw20 token, so it provide two interfaces.

* Native Token Deposit
    ```rust
    #[serde(rename_all = "snake_case")]
    pub enum HandleMsg {
        Deposit {
            position_idx: Uint128,
            collateral: Asset,
        }
    }
    ```

* CW20 Token Deposit
   ```rust
   #[serde(rename_all = "snake_case")]
   pub enum Cw20HookMsg {
      Deposit { position_idx: Uint128 },
   }
   ```

### Withdraw Collateral
Users can always withdraw the CDP collateral. However, they are forced to always keep the minimum amount of collateral asset to cover their CDP as follow.

```rust
// Compute new collateral amount
let collateral_amount: Uint128 = (position.collateral.amount - collateral.amount)?;

// Convert asset to collateral unit
let asset_value_in_collateral_asset: Uint128 =
    position.asset.amount * asset_price * reverse_decimal(collateral_price);

// Check minimum collateral ratio is statified
if asset_value_in_collateral_asset * asset_config.min_collateral_ratio > collateral_amount {
    return Err(StdError::generic_err(
        "Cannot withdraw collateral over than minimum collateral ratio",
    ));
}
```

Request Format
```rust
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Withdraw {
        position_idx: Uint128,
        collateral: Asset,
    }
}
```

### Mint Asset

Users can mint any mirror asset with mirror assets or pre-defined native token as collateral. The contract enforces following logics at mint process to keep minimum collateral ratio.

```rust
// Compute new asset amount
let asset_amount: Uint128 = asset.amount + position.asset.amount;

// Convert asset to collateral unit
let asset_value_in_collateral_asset: Uint128 =
    asset_amount * asset_price * reverse_decimal(collateral_price);

// Check minimum collateral ratio is statified
if asset_value_in_collateral_asset * asset_config.min_collateral_ratio
    > position.collateral.amount
{
    return Err(StdError::generic_err(
        "Cannot mint asset over than min collateral ratio",
    ));
}
```

Request Format

```rust
pub enum HandleMsg {
    Mint {
        position_idx: Uint128,
        asset: Asset,
    },
}

pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Token { contract_addr: HumanAddr },
    NativeToken { denom: String },
}
```

### Burn Asset
Users can burn the minted asset without restriction to increase the collateral ratio or to close the position. 

Burn request always passed thorugh CW20 token contract.

```rust
pub enum Cw20HookMsg {
    Burn { position_idx: Uint128 },
}
```
    
### Auction
The CDPs are always in liquidation danger, so position owners need to keep the collateral ratio bigger than minimum.
If the collateral ratio becomes smaller than minimum, the liquidation auction is held to let anyone liquidate the position.

Auction Held Condition
```rust
let asset_value_in_collateral_asset: Uint128 =
        position.asset.amount * asset_price * reverse_decimal(collateral_price);
if asset_value_in_collateral_asset * asset_config.min_collateral_ratio
    < position.collateral.amount
{
    return Err(StdError::generic_err(
        "Cannot liquidate a safely collateralized position",
    ));
}
```

Discounted Collateral Price
```rust
let discounted_collateral_price = collateral_price  * (1 - auction_discount)
asset_amount * asset_price / collateral_price  * (1 + auction_discount)
```

The provided asset cannot be bigger than the position's asset amount and also the returned collateral amount cannot be bigger than the position's collateral amount.

The left collateral amount after liqudate all asset is transferred to the position owner.

Auction request always passed thorugh CW20 token contract.
```rust
pub enum Cw20HookMsg {
    Auction { position_idx: Uint128 },
}
```
