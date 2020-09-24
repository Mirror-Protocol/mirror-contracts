# Staking Contract

This contract is a rewards distribution contract, which takes `config.staking_token` and gives `config.reward_token` as rewards. 


## Reward Index
The reward index represents the cumulated rewards per 1 staking token. Whenever the contract receives the rewards, the index keep increasing in following way.

```rust
let mut pool_info: PoolInfo = read_pool_info(&deps.storage, &asset_token_raw)?;
let reward_per_bond = Decimal::from_ratio(amount, pool_info.total_bond_amount);
pool_info.reward_index = pool_info.reward_index + reward_per_bond;
```

The contract keeps recording the reward index for each user, so it is possible to compute the cumulated rewards when a user want to withdraw it by subtracting reward index from the global one.

```rust
let pending_reward = (reward_info.bond_amount * pool_info.reward_index
    - reward_info.bond_amount * reward_info.index)?;
```

This way is applicable to track rewards only when the bonding amount is not changed. Whenever a user change its bonding amount, the pending reward and the index must be changed with following way: 

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
