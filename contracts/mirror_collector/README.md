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

## HandleMsg

### `convert`

It is permissionless function to swap all balance of an asset token to `config.collateral_denom` thorugh uniswap contract. It retreives uniswap pair(`config.distribution_contract`<>`asset_token`) contract address from the `config.uniswap_factory`. If the given asset token is `config.mirror_token`, it swaps all `config.collateral_denom` to `config.mirror_token`.

The steps are

- Asset Token => Collateral Denom
- Collateral Denom => Mirror Token

```json
{
  "convert": {
    "asset_token": "terra..."
  }
}
```

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |

### `send`

Send all balance of the `config.mirror_token` to `config.distribution_contract`.

```json
{
  "send": {}
}
```

## QueryMsg

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
