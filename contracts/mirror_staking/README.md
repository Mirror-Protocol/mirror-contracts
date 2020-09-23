# Staking Contract

This contract is a rewards distribution contract, which takes `config.staking_token` and gives `config.reward_token` as rewards. 

It keeps a reward index, which represent the cumulated rewards per 1 staking token. Whenever a user change its bonding amount, the pending reward and the index are automatically changed with following equation: 

```rust
    let pending_reward = (reward_info.bond_amount * pool_info.reward_index
        - reward_info.bond_amount * reward_info.index)?;

    reward_info.index = pool_info.reward_index;
    reward_info.pending_reward += pending_reward;
```


## Features

* Bond

* Unbond

* Distribute Reward

* Withdraw Reward
    
