# Mirror Mint <!-- omit in toc -->

## Table of Contents <!-- omit in toc -->

- [InitMsg](#initmsg)
- [HandleMsg](#handlemsg)
  - [`Receive`](#receive)
  - [`UpdateConfig`](#updateconfig)
  - [`UpdateAsset`](#updateasset)
  - [`RegisterAsset`](#registerasset)
  - [`OpenPosition`](#openposition)
  - [`Deposit`](#deposit)
  - [`Withdraw`](#withdraw)
  - [`Mint`](#mint)
- [QueryMsg](#querymsg)
  - [`Config`](#config)
  - [`AssetConfig`](#assetconfig)
  - [`Position`](#position)
  - [`Positions`](#positions)
- [Features](#features)

## InitMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    pub oracle: HumanAddr,
    pub base_asset_info: AssetInfo,
    pub token_code_id: u64,
}
```

| Key              | Type       | Description |
| ---------------- | ---------- | ----------- |
| `owner`          | AccAddress |             |
| `oracle`         | AccAddress |             |
| `base_aset_info` | AssetInfo  |             |
| `token_code_id`  | u64        |             |

## HandleMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    /// Update config; only owner is allowed to execute it
    UpdateConfig {
        owner: Option<HumanAddr>,
        token_code_id: Option<u64>,
    },
    /// Update asset related parameters
    UpdateAsset {
        asset_info: AssetInfo,
        auction_discount: Option<Decimal>,
        min_collateral_ratio: Option<Decimal>,
    },
    /// Generate asset token initialize msg and register required infos except token address
    RegisterAsset {
        asset_token: HumanAddr,
        auction_discount: Decimal,
        min_collateral_ratio: Decimal,
    },
    // create position to meet collateral ratio
    OpenPosition {
        collateral: Asset,
        asset_info: AssetInfo,
        collateral_ratio: Decimal,
    },
    /// deposit more collateral
    Deposit {
        position_idx: Uint128,
        collateral: Asset,
    },
    /// withdraw collateral
    Withdraw {
        position_idx: Uint128,
        collateral: Asset,
    },
    /// convert all deposit collateral to asset
    Mint {
        position_idx: Uint128,
        asset: Asset,
    },
}
```

**Cw20ReceiveMsg** definition:

```rust
/// Cw20ReceiveMsg should be de/serialized under `Receive()` variant in a HandleMsg
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Cw20ReceiveMsg {
    pub sender: HumanAddr,
    pub amount: Uint128,
    pub msg: Option<Binary>,
}
```

### `Receive`

Hook for when the mint contract is the recipient of a CW20 transfer, allows CW20 contract to execute a message defined in mint contract.

| Key      | Type       | Description |
| -------- | ---------- | ----------- |
| `sender` | AccAddress |             |
| `amount` | Uint128    |             |
| `msg`\*  | Binary     |             |

\* = optional

### `UpdateConfig`

| Key               | Type       | Description |
| ----------------- | ---------- | ----------- |
| `owner`\*         | AccAddress |             |
| `token_code_id`\* | u64        |             |

\* = optional

### `UpdateAsset`

| Key                      | Type      | Description |
| ------------------------ | --------- | ----------- |
| `asset_info`             | AssetInfo |             |
| `auction_discount`\*     | Decimal   |             |
| `min_collateral_ratio`\* | Decimal   |             |

\* = optional

### `RegisterAsset`

| Key                    | Type      | Description |
| ---------------------- | --------- | ----------- |
| `asset_token`          | HumanInfo |             |
| `auction_discount`     | Decimal   |             |
| `min_collateral_ratio` | Decimal   |             |

### `OpenPosition`

| Key                | Type      | Description |
| ------------------ | --------- | ----------- |
| `collateral`       | Asset     |             |
| `asset_info`       | AssetInfo |             |
| `collateral_ratio` | Decimal   |             |

### `Deposit`

| Key            | Type    | Description |
| -------------- | ------- | ----------- |
| `position_idx` | Uint128 |             |
| `collateral`   | Asset   |             |

### `Withdraw`

| Key            | Type    | Description |
| -------------- | ------- | ----------- |
| `position_idx` | Uint128 |             |
| `collateral`   | Asset   |             |

### `Mint`

| Key            | Type    | Description |
| -------------- | ------- | ----------- |
| `position_idx` | Uint128 |             |
| `collateral`   | Asset   |             |

## QueryMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    AssetConfig {
        asset_info: AssetInfo,
    },
    Position {
        position_idx: Uint128,
    },
    Positions {
        owner_addr: HumanAddr,
        start_after: Option<Uint128>,
        limit: Option<u32>,
    },
}
```

### `Config`

### `AssetConfig`

### `Position`

### `Positions`

## Features

Mirrror Protocol mint contract provides following features

- Mint & Burn

  The asset can be minted with some colalteral of `config.collateral_denom`. The contract uses asset oracle to get `price` and `config.mint_capacity` to calculate `mint_amount`.

  It also allows a user to add more collateral to protect the mint poisition from the margin call.

  ```rust
  let total_collateral_amount = position.collateral_amount + new_collateral_amount;
  let asset_amount = total_collateral_amount * price * config.mint_capacity;
  let mint_amount = (asset_amount - position.asset_amount).unwrap_or(Uint128::zero());
  ```

The contract recognizes the sent coins with `mint` msg as collateral amount.

```json
{ "mint": {} }
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

- Auction

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
