# Mirror Governance
This is a simple voting contract. It creates a contract to manage token weighted polls,
where voters deposit predefined gov cw20 tokens in order to vote.

## Configs
| Name             | Description                                                          |
| ---------------- | -------------------------------------------------------------------- |
| mirror_token     | Mirror token contract address                                        |
| quorum           | The minium percentage of participation required to pass the poll     |
| threshold        | The minimum percentage of yes vote required to pass the poll         |
| voting_period    | The number of block the poll should be in voting state               |
| effetive_delay   | The number of block it takes for the polls to actually be reflected. |
| proposal_deposit | The minium deposit token to register proposal                        |


## Handle Messages
### Create Poll & End Poll
Anyone can create a poll with predefined `config.deposit` amount of tokens. After the voting period is over, anyone can close the poll. If the quorum is satisfied, the deposit will be returned to the creator, and if not, the deposit will not be returned. The non-refundable deposit is distributed to the staking pool so that all users share it.

A user need to send `Cw20HandleMsg::Send{Cw20HookMsg::CreatePoll}` to mirror token contract.

```rust
pub enum Cw20HookMsg {
    /// CreatePoll need to receive deposit from a proposer
    CreatePoll {
        description: String,
        execute_msg: Option<ExecuteMsg>,
    },
}
```

If the polls is ended with `pass`, it will execute registered `WasmExecute` msg.
```rust
pub enum HandleMsg {
    EndPoll {
        poll_id: u64,
    }
}
```

### Execute Poll

There is some time window for the polls to actually be reflected. Anyone can execute this operation to reflect the poll result after `config.effective_delay` of blocks has passed.

```rust
pub enum HandleMsg {
    ExecutePoll {
        poll_id: u64,
    }
}
```

### Staking
Users can stake their mirror token to receive staking incomes, which are collected from the uniswap, or to cast vote on the polls. 

A user need to send `Cw20HandleMsg::Send{Cw20HookMsg::StakeVotingTokens}` to mirror token contract.

```rust
pub enum Cw20HookMsg {
    /// StakeVotingTokens a user can stake their mirror token to receive rewards
    /// or do vote on polls
    StakeVotingTokens {},
}
```

### Unstaking
Users can withdraw their stake, but not while a poll they've participated in is still in progress.

```rust
pub enum HandleMsg {
    WithdrawVotingTokens {
        amount: Option<Uint128>,
    }
}
```

### Cast Vote
Voters can use their voting power redundantly in multiple polls, and when voting, they can designate as much voting power as they want to vote.

```rust
pub enum HandleMsg {
    CastVote {
        poll_id: u64,
        vote: VoteOption,
        share: Uint128,
    }
}
```
