# Mirror Governance <!-- omit in toc -->

This is a simple voting contract. It creates a contract to manage token weighted polls,
where voters deposit predefined gov cw20 tokens in order to vote.

## Table of Contents <!-- omit in toc -->

- [Configs](#configs)
- [InitMsg](#initmsg)
- [HandleMsg](#handlemsg)
  - [`update_config`](#update_config)
  - [`cast_vote`](#cast_vote)
  - [`withdraw_voting_tokens`](#withdraw_voting_tokens)
  - [`end_poll`](#end_poll)
- [QueryMsg](#querymsg)
  - [`config`](#config)
  - [`state`](#state)
  - [`stake`](#stake)
  - [`poll`](#poll)
  - [Create Poll & End Poll](#create-poll--end-poll)
  - [Staking](#staking)

## Configs

| Name             | Description                                                      |
| ---------------- | ---------------------------------------------------------------- |
| mirror_token     | Mirror token contract address                                    |
| quorum           | The minium percentage of participation required to pass the poll |
| threshold        | The minimum percentage of yes vote required to pass the poll     |
| voting_period    | The number of block the poll should be in voting state           |
| proposal_deposit | The minium deposit token to register proposal                    |

## InitMsg

```json
{
  "mirror_token": "terra...",
  "quorum": "123.123",
  "threshold": "0.33",
  "voting_period": "1234",
  "proposal_deposit": "1000000"
}
```

| Key                | Type       | Description |
| ------------------ | ---------- | ----------- |
| `mirror_token`     | AccAddress |             |
| `quorum`           | Decimal    |             |
| `threshold`        | Decimal    |             |
| `voting_period`    | u64        |             |
| `proposal_deposit` | Uint128    |             |

## HandleMsg

### `update_config`

```json
{
  "owner": "terra...",
  "quorum": "123.123",
  "threshold": "0.33",
  "voting_period": "1234",
  "proposal_deposit": "1000000"
}
```

| Key                  | Type       | Description |
| -------------------- | ---------- | ----------- |
| `owner`\*            | AccAddress |             |
| `quorum`\*           | AccAddress |             |
| `threshold`\*        | Decimal    |             |
| `voting_period`\*    | u64        |             |
| `proposal_deposit`\* | Uint128    |             |

\* = optional

### `cast_vote`

Voters can use their voting power redundantly in multiple polls, and when voting, they can designate as much voting power as they want to vote.

```json
{
  "cast_vote": {
    "poll_id": "4",
    "vote": "YES",
    "share": "1000000"
  }
}
```

| Key       | Type              | Description                 |
| --------- | ----------------- | --------------------------- |
| `poll_id` | u64               | Poll for which to cast vote |
| `vote`    | `"YES"` or `"NO"` | Vote option                 |
| `share`   | Uint128           |                             |

### `withdraw_voting_tokens`

Users can withdraw their stake, but not while a poll they've participated in is still in progress.

```json
{
  "withdraw_voting_tokens": {
    "amount": "1000000"
  }
}
```

| Key        | Type    | Description |
| ---------- | ------- | ----------- |
| `amount`\* | Uint128 |             |

\* = optional

### `end_poll`

If the polls is ended with `pass`, it will execute registered `WasmExecute` msg.

```json
{
  "end_poll": {
    "poll_id": "42"
  }
}
```

| Key       | Type | Description    |
| --------- | ---- | -------------- |
| `poll_id` | u64  | Poll ID to end |

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
  "quorum": "0.33",
  "threshold": "0.33",
  "voting_period": "1420",
  "proposal_deposit": "1000000"
}
```

| Key                | Type       | Description |
| ------------------ | ---------- | ----------- |
| `owner`            | AccAddress |             |
| `mirror_token`     | AccAddress |             |
| `quorum`           | Decimal    |             |
| `threshold`        | Decimal    |             |
| `voting_period`    | u64        |             |
| `proposal_deposit` | Uint128    |             |

### `state`

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

### `stake`

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

### `poll`

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

# Create Poll & End Poll

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

# Staking

Users can stake their mirror token to receive staking incomes, which are collected from the uniswap, or to cast vote on the polls.

A user need to send `Cw20HandleMsg::Send{Cw20HookMsg::StakeVotingTokens}` to mirror token contract.

```rust
pub enum Cw20HookMsg {
    /// StakeVotingTokens a user can stake their mirror token to receive rewards
    /// or do vote on polls
    StakeVotingTokens {},
}
```
