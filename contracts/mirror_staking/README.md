# Mirror Staking <!-- omit in toc -->

This contract is a rewards distribution contract, which takes `config.staking_token` and gives `config.reward_token` as rewards.

It keeps a reward index, which represent the cumulated rewards per 1 staking token. Whenever a user change its bonding amount, the pending reward and the index are automatically changed with following equation:

```rust
    let pending_reward = (reward_info.bond_amount * pool_info.reward_index
        - reward_info.bond_amount * reward_info.index)?;

    reward_info.index = pool_info.reward_index;
    reward_info.pending_reward += pending_reward;
```

## Configs

| Name         | Description                       |
| ------------ | --------------------------------- |
| owner        | The owner address                 |
| mirror_token | The Mirror Token contract address |

## Handlers

### Register Asset (Owner)

It is for registering new asset on mirror staking pool. Only the owner can execute this operation.

Request Format

```rust
pub enum HandlerMsg {
    RegisterAsset {
        asset_token: HumanAddr,
        staking_token: HumanAddr,
    }
}
```

### Distribute Reward

This operation is for distribute rewards token to a specific asset stakers. Any one can execute it to put the rewards on the pool.

Distribute request always passed thorugh CW20 token contract.

```rust
pub enum Cw20HookMsg {
    DepositReward { asset_token: HumanAddr },
}
```

### Bond

Users can bond their liquidity token, which is issued from uniswap pair contract as a proof token of pool contribution. The stakers can receive the Mirror inflation rewards in proportion to their staking amount.

Bond request always passed thorugh CW20 token contract.

```rust
pub enum Cw20HookMsg {
    Bond { asset_token: HumanAddr },
}
```

### Unbond

Users can unbond their liquidity token without restriction.

```rust
pub enum HandleMsg {
    Unbond {
        asset_token: HumanAddr,
        amount: Uint128,
    }
}
```

### Withdraw Reward

Whenever user change their staking amount, the rewards go to pending rewards. The pending rewards can be withdrawn with this interface. It withdraws pending rewards including cumulated rewards with reward_index update.

```rust
pub enum HandleMsg {
    Withdraw {
        // If the asset token is not given, then all rewards are withdrawn
        asset_token: Option<HumanAddr>,
    },
}
```

## Table of Contents <!-- omit in toc -->

- [Configs](#configs)
- [Handlers](#handlers)
  - [Register Asset (Owner)](#register-asset-owner)
  - [Distribute Reward](#distribute-reward)
  - [Bond](#bond)
  - [Unbond](#unbond)
  - [Withdraw Reward](#withdraw-reward)
- [InitMsg](#initmsg)
- [HandleMsg](#handlemsg)
  - [`Receive`](#receive)
  - [`UpdateConfig`](#updateconfig)
  - [`RegisterAsset`](#registerasset)
  - [`Unbond`](#unbond-1)
  - [`Withdraw`](#withdraw)
- [QueryMsg](#querymsg)
  - [`Config`](#config)
  - [`PoolInfo`](#poolinfo)
  - [`RewardInfo`](#rewardinfo)

## InitMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub owner: HumanAddr,
    pub mirror_token: HumanAddr,
}
```

| Key            | Type       | Description |
| -------------- | ---------- | ----------- |
| `owner`        | AccAddress |             |
| `mirror_token` | AccAddress |             |

## HandleMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    UpdateConfig {
        owner: Option<HumanAddr>,
    },
    RegisterAsset {
        asset_token: HumanAddr,
        staking_token: HumanAddr,
    },
    Unbond {
        asset_token: HumanAddr,
        amount: Uint128,
    },
    /// withdraw pending rewards
    Withdraw {
        // If the asset token is not given, then all rewards are withdrawn
        asset_token: Option<HumanAddr>,
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

| Key      | Type       | Description |
| -------- | ---------- | ----------- |
| `sender` | AccAddress |             |
| `amount` | AssetInfo  |             |
| `msg`\*  | Binary     |             |

\* = optional

### `UpdateConfig`

| Key       | Type       | Description |
| --------- | ---------- | ----------- |
| `owner`\* | AccAddress |             |

\* = optional

### `RegisterAsset`

| Key             | Type       | Description |
| --------------- | ---------- | ----------- |
| `asset_token`   | AccAddress |             |
| `staking_token` | AccAddress |             |

### `Unbond`

| Key           | Type       | Description |
| ------------- | ---------- | ----------- |
| `asset_token` | AccAddress |             |
| `amount`      | Uint128    |             |

### `Withdraw`

| Key             | Type       | Description |
| --------------- | ---------- | ----------- |
| `asset_token`\* | AccAddress |             |

\* = optional

## QueryMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PoolInfo {
        asset_token: HumanAddr,
    },
    RewardInfo {
        asset_token: Option<HumanAddr>,
        staker: HumanAddr,
    },
}
```

### `Config`

### `PoolInfo`

### `RewardInfo`
