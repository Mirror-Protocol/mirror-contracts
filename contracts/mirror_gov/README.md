# Mirror Governance <!-- omit in toc -->

This is a simple voting contract. It creates a contract to manage token weighted polls,
where voters deposit predefined gov cw20 tokens in order to vote.

## Table of Contents <!-- omit in toc -->

- [Config](#config)
- [InitMsg](#initmsg)
- [HandleMsg](#handlemsg)
  - [`UpdateConfig`](#updateconfig)
  - [`CastVote`](#castvote)
  - [`WithdrawVotingTokens`](#withdrawvotingtokens)
  - [`EndPoll`](#endpoll)
  - [`ExecutePoll`](#executepoll)
  - [`ExpirePoll`](#expirepoll)
- [QueryMsg](#querymsg)
  - [`Config`](#config-1)
    - [Request](#request)
    - [Response](#response)
  - [`State`](#state)
    - [Request](#request-1)
    - [Response](#response-1)
  - [`Stake`](#stake)
    - [Request](#request-2)
    - [Response](#response-2)
  - [`Poll`](#poll)
    - [Request](#request-3)
    - [Response](#response-3)
- [Features](#features)
  - [Create Poll & End Poll](#create-poll--end-poll)
  - [Staking](#staking)

## Config

| Name             | Description                                                      |
| ---------------- | ---------------------------------------------------------------- |
| mirror_token     | Mirror token contract address                                    |
| quorum           | The minium percentage of participation required to pass the poll |
| threshold        | The minimum percentage of yes vote required to pass the poll     |
| voting_period    | The number of block the poll should be in voting state           |
| proposal_deposit | The minium deposit token to register proposal                    |

## InitMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub mirror_token: HumanAddr,
    pub quorum: Decimal,
    pub threshold: Decimal,
    pub voting_period: u64,
    pub effective_delay: u64,
    pub expiration_period: u64,
    pub proposal_deposit: Uint128,
}
```

| Key                 | Type       | Description                                  |
| ------------------- | ---------- | -------------------------------------------- |
| `mirror_token`      | AccAddress |                                              |
| `quorum`            | Decimal    |                                              |
| `threshold`         | Decimal    |                                              |
| `voting_period`     | u64        |                                              |
| `effective_delay`   | u64        | num of blocks must be passed before executed |
| `expiration_period` | u64        | num of blocks must be passed before expired  |
| `proposal_deposit`  | Uint128    |                                              |

## HandleMsg

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    UpdateConfig {
        owner: Option<HumanAddr>,
        quorum: Option<Decimal>,
        threshold: Option<Decimal>,
        voting_period: Option<u64>,
        effective_delay: Option<u64>,
        expiration_period: Option<u64>,
        proposal_deposit: Option<Uint128>,
    },
    CastVote {
        poll_id: u64,
        vote: VoteOption,
        share: Uint128,
    },
    WithdrawVotingTokens {
        amount: Option<Uint128>,
    },
    EndPoll {
        poll_id: u64,
    },
    ExecutePoll {
        poll_id: u64,
    },
    ExpirePoll {
        poll_id: u64,
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

### `UpdateConfig`

| Key                  | Type       | Description |
| -------------------- | ---------- | ----------- |
| `owner`\*            | AccAddress |             |
| `quorum`\*           | AccAddress |             |
| `threshold`\*        | Decimal    |             |
| `voting_period`\*    | u64        |             |
| `proposal_deposit`\* | Uint128    |             |

\* = optional

### `CastVote`

Voters can use their voting power redundantly in multiple polls, and when voting, they can designate as much voting power as they want to vote.

| Key       | Type              | Description                 |
| --------- | ----------------- | --------------------------- |
| `poll_id` | u64               | Poll for which to cast vote |
| `vote`    | `"YES"` or `"NO"` | Vote option                 |
| `share`   | Uint128           |                             |

### `WithdrawVotingTokens`

Users can withdraw their stake, but not while a poll they've participated in is still in progress.

| Key        | Type    | Description |
| ---------- | ------- | ----------- |
| `amount`\* | Uint128 |             |

\* = optional

### `EndPoll`

Tally the poll and refund the deposit depends on the result.

| Key       | Type | Description    |
| --------- | ---- | -------------- |
| `poll_id` | u64  | Poll ID to end |

### `ExecutePoll`

If the polls is ended with `pass`, it will execute registered `WasmExecute` msg after `effective_delay` blocks.

| Key       | Type | Description    |
| --------- | ---- | -------------- |
| `poll_id` | u64  | Poll ID to end |


### `ExpirePoll`

If the polls is ended with `pass` and is not executed during `expiration_period`, it will update polls state to `exipred`.

| Key       | Type | Description    |
| --------- | ---- | -------------- |
| `poll_id` | u64  | Poll ID to end |

## QueryMsg

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
  "quorum": "0.33",
  "threshold": "0.33",
  "voting_period": 1420,
  "effective_delay": 1000,
  "expiration_period": 2000,
  "proposal_deposit": "1000000"
}
```

| Key                 | Type       | Description |
| ------------------- | ---------- | ----------- |
| `owner`             | AccAddress |             |
| `mirror_token`      | AccAddress |             |
| `quorum`            | Decimal    |             |
| `threshold`         | Decimal    |             |
| `voting_period`     | u64        |             |
| `effective_delay`   | u64        |             |
| `expiration_period` | u64        |             |
| `proposal_deposit`  | Uint128    |             |

### `State`

#### Request

```json
{
  "state": {}
}
```

#### Response

```json
{
  "poll_count": "142",
  "total_share": "1000000"
}
```

| Key           | Type    | Description |
| ------------- | ------- | ----------- |
| `poll_count`  | u64     |             |
| `total_share` | Uint128 |             |

### `Stake`

#### Request

```json
{
  "stake": {
    "address": "terra..."
  }
}
```

| Key       | Type       | Description      |
| --------- | ---------- | ---------------- |
| `address` | AccAddress | Address to query |

#### Response

```json
{
  "balance": "1000000",
  "share": "1000000"
}
```

| Key       | Type    | Description |
| --------- | ------- | ----------- |
| `balance` | Uint128 |             |
| `share`   | Uint128 |             |

### `Poll`

#### Request

```json
{
  "poll": {
    "poll_id": "42"
  }
}
```

| Key       | Type | Description      |
| --------- | ---- | ---------------- |
| `poll_id` | u64  | Poll ID to query |

#### Response

```json
{
  "creator": "terra...",
  "status": "passed",
  "end_height": "100",
  "description": "poll description...",
  "deposit_amount": "1000000"
}
```

| Key              | Type       | Description                                          |
| ---------------- | ---------- | ---------------------------------------------------- |
| `creator`        | AccAddress | Poll ID to end                                       |
| `status`         | PollStatus | One of: `in_progress`, `tally`, `passed`, `rejected` |
| `end_height`     | u64        | Block height at which poll ended                     |
| `description`    | string     | Poll description                                     |
| `deposit_amount` | Uint128    | Total deposit amount                                 |

## Features

### Create Poll & End Poll

Anyone can create a poll with predefined `config.deposit` amount of tokens. After the voting period is over, anyone can close the poll. If the quorum is satisfied, the deposit will be returned to the creator, and if not, the deposit will not be returned. The non-refundable deposit is distributed on the staking pool so that all users can divide and withdraw.

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
