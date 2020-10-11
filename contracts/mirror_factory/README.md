# Mirror Factory <!-- omit in toc -->

This contract is for mirror token distribution. It continually mints mirror token and distributes minted tokens to pools in staking contract, which are registered with `whitelist` operation.

## Table of Contents <!-- omit in toc -->

- [Config](#config)
- [InitMsg](#initmsg)
- [HandleMsg](#handlemsg)
  - [`PostInitialize`](#postinitialize)
  - [`UpdateConfig`](#updateconfig)
  - [`UpdateWeight`](#updateweight)
  - [`Whitelist`](#whitelist)
  - [`TokenCreationHook`](#tokencreationhook)
  - [`UniswapCreationHook`](#uniswapcreationhook)
  - [`PassCommand`](#passcommand)
  - [`Mint`](#mint)
- [QueryMsg](#querymsg)
  - [`Config`](#config-1)
  - [`DistributionInfo`](#distributioninfo)

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

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub mint_per_block: Uint128,
    pub token_code_id: u64,
    pub base_denom: String,
}
```

| Key              | Type    | Description                                                |
| ---------------- | ------- | ---------------------------------------------------------- |
| `mint_per_block` | Uint128 | Amount of mirror token to mint per block for each 1 weight |
| `token_code_id`  | u64     | Code ID for asset token                                    |
| `base_denom`     | string  | Native token denom used to create uniswap pair             |

## HandleMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    PostInitialize {
        owner: HumanAddr,
        uniswap_factory: HumanAddr,
        mirror_token: HumanAddr,
        staking_contract: HumanAddr,
        oracle_contract: HumanAddr,
        mint_contract: HumanAddr,
        commission_collector: HumanAddr,
    },
    UpdateConfig {
        owner: Option<HumanAddr>,
        mint_per_block: Option<Uint128>,
        token_code_id: Option<u64>,
    },
    UpdateWeight {
        asset_token: HumanAddr,
        weight: Decimal,
    },
    Whitelist {
        /// asset name used to create token contract
        name: String,
        /// asset symbol used to create token contract
        symbol: String,
        /// authorized asset oracle feeder
        oracle_feeder: HumanAddr,
        /// used to create all necessary contract or register asset
        params: Params,
    },
    TokenCreationHook {
        oracle_feeder: HumanAddr,
    },
    UniswapCreationHook {
        asset_token: HumanAddr,
    },
    PassCommand {
        contract_addr: HumanAddr,
        msg: Binary,
    },
    Mint {
        asset_token: HumanAddr,
    },
}
```

### `PostInitialize`

This operation is used to register all relevant contracts `uniswap_factory`, `mirror_token`, `staking_contract`, `oracle_contract`, `mint_contract`, `commission_collector`. Only owner is allowed to execute it.

| Key                    | Type       | Description                          |
| ---------------------- | ---------- | ------------------------------------ |
| `owner`                | AccAddress | Owner of the Mirror Factory contract |
| `uniswap_factory`      | AccAddress | Uniswap Factory contract address     |
| `mirror_token`         | AccAddress | Mirror Token contract address        |
| `staking_contract`     | AccAddress | Mirror Straking contract address     |
| `oracle_contract`      | AccAddress | Mirror Oracle contract address       |
| `mint_contract`        | AccAddress | Mirror Mint contract address         |
| `commission_collector` | AccAddress | Mirror Collector contract address    |

### `UpdateConfig`

A owner can update `mint_per_block` or `token_code_id`.

| Key                 | Type       | Description                                                |
| ------------------- | ---------- | ---------------------------------------------------------- |
| `owner`\*           | AccAddress | Amount of mirror token to mint per block for each 1 weight |
| `uniswap_factory`\* | AccAddress | Code ID for asset token                                    |
| `mirror_token`\*    | AccAddress | Native token denom used to create uniswap pair             |

\* = optional

### `UpdateWeight`

A owner can update mint weight of a specific symbol asset.

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |
| `weight`      | Decimal    |             |

### `Whitelist`

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

| Key             | Type       | Description                                                |
| --------------- | ---------- | ---------------------------------------------------------- |
| `name`          | AccAddress | Amount of mirror token to mint per block for each 1 weight |
| `symbol`        | AccAddress | Code ID for asset token                                    |
| `oracle_feeder` | AccAddress | Native token denom used to create uniswap pair             |
| `params`        | Params     |                                                            |

### `TokenCreationHook`

| Key             | Type       | Description |
| --------------- | ---------- | ----------- |
| `oracle_feeder` | AccAddress |             |

### `UniswapCreationHook`

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |

### `PassCommand`

Owner can pass any message to any contract with this message. The factory has many ownership privilege, so this interface is for allowing a owner to exert ownership over the child contracts.

| Key             | Type       | Description |
| --------------- | ---------- | ----------- |
| `contract_addr` | AccAddress |             |
| `msg`           | Binary     |             |

### `Mint`

Anyone can execute mint function with a specify asset token argument. The mint amount is calculated with following equation and send it to `staking_contract`'s asset token pool:

```rust
   // mint_amount = weight * mint_per_block * (height - last_height)
   let mint_amount = (config.mint_per_block * distribution_info.weight)
      .multiply_ratio(env.block.height - distribution_info.last_height, 1u64);
```

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |

## QueryMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    DistributionInfo { asset_token: HumanAddr },
}

```

### `Config`

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

### `DistributionInfo`

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
