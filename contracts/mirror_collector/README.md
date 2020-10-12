# Mirror Collector <!-- omit in toc -->

This contract is a rewards collector contract, which converts all collected rewards to `config.mirror_token` through uniswap and send it to `config.distribution_contract`.

## Table of Contents <!-- omit in toc -->

- [InitMsg](#initmsg)
- [HandleMsg](#handlemsg)
  - [`convert`](#convert)
  - [`send`](#send)
- [QueryMsg](#querymsg)
  - [`config`](#config)

## InitMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub distribution_contract: HumanAddr, // collected rewards receiver
    pub uniswap_factory: HumanAddr,
    pub mirror_token: HumanAddr,
    pub base_denom: String,
}
```

| Key                     | Type       | Description |
| ----------------------- | ---------- | ----------- |
| `distribution_contract` | AccAddress |             |
| `uniswap_factory`       | AccAddress |             |
| `mirror_token`          | AccAddress |             |
| `base_denom`            | string     |             |

## HandleMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Convert { asset_token: HumanAddr },
    Send {},
}
```

### `convert`

It is permissionless function to swap all balance of an asset token to `config.collateral_denom` thorugh uniswap contract. It retreives uniswap pair(`config.distribution_contract`<>`asset_token`) contract address from the `config.uniswap_factory`. If the given asset token is `config.mirror_token`, it swaps all `config.collateral_denom` to `config.mirror_token`.

The steps are

- Asset Token => Collateral Denom
- Collateral Denom => Mirror Token

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |

### `send`

Send all balance of the `config.mirror_token` to `config.distribution_contract`.

## QueryMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}
```

### `config`

#### Request

```json
{
  "config": {}
}
```

#### Response

```json
{
  "distribution_contract": "terra...",
  "uniswap_factory": "terra...",
  "mirror_token": "terra...",
  "base_denom": "uusd"
}
```

| Key                     | Type       | Description |
| ----------------------- | ---------- | ----------- |
| `distribution_contract` | AccAddress |             |
| `uniswap_factory`       | AccAddress |             |
| `mirror_token`          | AccAddress |             |
| `base_denom`            | string     |             |
