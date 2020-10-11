# Mirror Staking <!-- omit in toc -->

This contract is a rewards distribution contract, which takes `config.staking_token` and gives `config.reward_token` as rewards.

It keeps a reward index, which represent the cumulated rewards per 1 staking token. Whenever a user change its bonding amount, the pending reward and the index are automatically changed with following equation:

```rust
    let pending_reward = (reward_info.bond_amount * pool_info.reward_index
        - reward_info.bond_amount * reward_info.index)?;

    reward_info.index = pool_info.reward_index;
    reward_info.pending_reward += pending_reward;
```

## Table of Contents <!-- omit in toc -->

- [InitMsg](#initmsg)
- [HandleMsg](#handlemsg)
  - [`Receive`](#receive)
  - [`UpdateConfig`](#updateconfig)
  - [`RegisterAsset`](#registerasset)
  - [`Unbond`](#unbond)
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
