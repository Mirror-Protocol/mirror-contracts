# Mirror Factory <!-- omit in toc -->

This contract is for mirror token distribution. It continually mints mirror token and distributes minted tokens to pools in staking contract, which are registered with `whitelist` operation.

## Table of Contents <!-- omit in toc -->

- [Config](#config)
- [InitMsg](#initmsg)
- [HandleMsg](#handlemsg)
  - [`post_initialize`](#post_initialize)
  - [`update_config`](#update_config)
  - [`update_weight`](#update_weight)
  - [`whitelist`](#whitelist)
  - [`token_creation_hook`](#token_creation_hook)
  - [`uniswap_creation_hook`](#uniswap_creation_hook)
  - [`pass_command`](#pass_command)
  - [`mint`](#mint)
- [QueryMsg](#querymsg)
  - [`config`](#config-1)
  - [`distribution_info`](#distribution_info)

## Config

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

## InitMsg

```json
{
  "mint_per_block": "1000000",
  "token_code_id": 42,
  "base_denom": "uusd"
}
```

| Key              | Type    | Description                                                |
| ---------------- | ------- | ---------------------------------------------------------- |
| `mint_per_block` | Uint128 | Amount of mirror token to mint per block for each 1 weight |
| `token_code_id`  | u64     | Code ID for asset token                                    |
| `base_denom`     | string  | Native token denom used to create uniswap pair             |

## HandleMsg

### `post_initialize`

This operation is used to register all relevant contracts `uniswap_factory`, `mirror_token`, `staking_contract`, `oracle_contract`, `mint_contract`, `commission_collector`. Only owner is allowed to execute it.

```json
{
  "post_initialize": {
    "owner": "terra...",
    "uniswap_factory": "terra...",
    "mirror_token": "terra...",
    "staking_contract": "terra...",
    "oracle_contract": "terra...",
    "mint_contract": "terra...",
    "commission_collector": "terra..."
  }
}
```

| Key                    | Type       | Description                          |
| ---------------------- | ---------- | ------------------------------------ |
| `owner`                | AccAddress | Owner of the Mirror Factory contract |
| `uniswap_factory`      | AccAddress | Uniswap Factory contract address     |
| `mirror_token`         | AccAddress | Mirror Token contract address        |
| `staking_contract`     | AccAddress | Mirror Straking contract address     |
| `oracle_contract`      | AccAddress | Mirror Oracle contract address       |
| `mint_contract`        | AccAddress | Mirror Mint contract address         |
| `commission_collector` | AccAddress | Mirror Collector contract address    |

### `update_config`

A owner can update `mint_per_block` or `token_code_id`.

```json
{
  "update_config": {
    "owner": "terra...",
    "mint_per_block": "1000000",
    "token_code_id": 25
  }
}
```

| Key                 | Type       | Description                                                |
| ------------------- | ---------- | ---------------------------------------------------------- |
| `owner`\*           | AccAddress | Amount of mirror token to mint per block for each 1 weight |
| `uniswap_factory`\* | AccAddress | Code ID for asset token                                    |
| `mirror_token`\*    | AccAddress | Native token denom used to create uniswap pair             |

\* = optional

### `update_weight`

A owner can update mint weight of a specific symbol asset.

```json
{
  "update_weight": {
    "asset_token": "terra...",
    "weight": "123.456"
  }
}
```

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |
| `weight`      | Decimal    |             |

### `whitelist`

<details><summary>Whitelist Procedure</summary>
<p>

Whitelisting is processed in following order:

1.  Create asset token contract with `config.token_code_id` with `minter` argument

2.  Call `TokenCreationHook`

    2-1. Initialize distribution info

    2-2. Register asset to mint contract

    2-3. Register asset and oracle feeder
    to oracle contract

    2-4. Create uniswap pair through uniswap factory

3.  Call `UniswapCreationHook`

    3-1. Register asset and liquidity(LP) token to staking contract

</p>
</details>

```json
{
  "whitelist": {
    "name": "terra...",
    "symbol": "1000000",
    "oracle_feeder": 25,
    "params": {
      "weight": "123.1231",
      "lp_commission": "123.1231",
      "owner_commission": "123.1231",
      "auction_discount": "123.1231",
      "min_collateral_ratio": "123.1231"
    }
  }
}
```

| Key             | Type       | Description                                                |
| --------------- | ---------- | ---------------------------------------------------------- |
| `name`          | AccAddress | Amount of mirror token to mint per block for each 1 weight |
| `symbol`        | AccAddress | Code ID for asset token                                    |
| `oracle_feeder` | AccAddress | Native token denom used to create uniswap pair             |
| `params`        | Params     |                                                            |

### `token_creation_hook`

```json
{
  "token_creation_hook": {
    "oracle_feeder": "terra..."
  }
}
```

| Key             | Type       | Description |
| --------------- | ---------- | ----------- |
| `oracle_feeder` | AccAddress |             |

### `uniswap_creation_hook`

```json
{
  "uniswap_creation_hook": {
    "asset_token": "terra..."
  }
}
```

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |

### `pass_command`

Owner can pass any message to any contract with this message. The factory has many ownership privilege, so this interface is for allowing a owner to exert ownership over the child contracts.

```json
{
  "pass_command": {
    "contract_addr": "terra...",
    "msg": "..."
  }
}
```

| Key             | Type       | Description |
| --------------- | ---------- | ----------- |
| `contract_addr` | AccAddress |             |
| `msg`           | Binary     |             |

### `mint`

Anyone can execute mint function with a specify asset token argument. The mint amount is calculated with following equation and send it to `staking_contract`'s asset token pool:

```rust
   // mint_amount = weight * mint_per_block * (height - last_height)
   let mint_amount = (config.mint_per_block * distribution_info.weight)
      .multiply_ratio(env.block.height - distribution_info.last_height, 1u64);
```

```json
{
  "mint": {
    "asset_token": "terra..."
  }
}
```

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |

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
  "owner": "terra...",
  "mirror_token": "terra...",
  "mint_contract": "terra...",
  "staking_contract": "terra...",
  "commission_collector": "terra...",
  "oracle_contract": "terra...",
  "uniswap_factory": "terra...",
  "mint_per_block": "1000000",
  "token_code_id": "23",
  "base_denom": "uusd"
}
```

| Key                    | Type       | Description |
| ---------------------- | ---------- | ----------- |
| `owner`                | AccAddress |             |
| `mirror_token`         | AccAddress |             |
| `mint_contract`        | AccAddress |
| `staking_contract`     | AccAddress |             |
| `commission_collector` | AccAddress |             |
| `oracle_contract`      | AccAddress |             |
| `uniswap_factory`      | AccAddress |             |
| `mint_per_block`       | Uint128    |             |
| `token_code_id`        | u64        |             |
| `base_denom`           | string     |             |

### `distribution_info`

#### Request

```json
{
  "distribution_info": {
    "asset_token": "terra..."
  }
}
```

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |

#### Response

```json
{
  "weight": "123.123",
  "last_height": "5"
}
```

| Key           | Type    | Description |
| ------------- | ------- | ----------- |
| `weight`      | Decimal |             |
| `last_height` | u64     |             |
