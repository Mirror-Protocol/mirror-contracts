# Mirror Oracle <!-- omit in toc -->

This contract is a OCS conformant smart contract. It provides simple interfcae to feed the price from the authorized feeder key. It also provides the variable `price_multiplier` to cope with events like stock split or merge. The oracle users can calculate the active price by multipling `price` and `price_multiplier`.

## Table of Contents <!-- omit in toc -->

- [InitMsg](#initmsg)
- [HandleMsg](#handlemsg)
  - [`UpdateConfig`](#updateconfig)
  - [`RegisterAsset`](#registerasset)
  - [`FeedPrice`](#feedprice)
- [QueryMsg](#querymsg)
  - [`Config`](#config)
  - [`Asset`](#asset)
  - [`Price`](#price)

## InitMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    pub base_asset_info: AssetInfo,
}
```

| Key               | Type       | Description |
| ----------------- | ---------- | ----------- |
| `owner`           | AccAddress |             |
| `base_asset_info` | AssetInfo  |             |

## HandleMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    UpdateConfig {
        owner: Option<HumanAddr>,
    },
    RegisterAsset {
        asset_info: AssetInfo,
        feeder: HumanAddr,
    },
    FeedPrice {
        asset_info: AssetInfo,
        price: Decimal,
        price_multiplier: Option<Decimal>,
    },
}
```

### `UpdateConfig`

| Key       | Type       | Description |
| --------- | ---------- | ----------- |
| `owner`\* | AccAddress |             |

\* = optional

### `RegisterAsset`

| Key          | Type       | Description |
| ------------ | ---------- | ----------- |
| `asset_info` | AssetInfo  |             |
| `feeder`     | AccAddress |             |

### `FeedPrice`

| Key                  | Type      | Description |
| -------------------- | --------- | ----------- |
| `asset_info`         | AssetInfo |             |
| `price`              | Decimal   |             |
| `price_multiplier`\* | Decimal   |             |

\* = optional

## QueryMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Asset { asset_info: AssetInfo },
    Price { asset_info: AssetInfo },
}
```

### `Config`

### `Asset`

### `Price`
