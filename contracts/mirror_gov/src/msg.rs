use crate::state::{PollStatus, VoteOption};
use cosmwasm_std::{Binary, Decimal, HumanAddr, Uint128};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub mirror_token: HumanAddr,
    pub quorum: Decimal,
    pub threshold: Decimal,
    pub voting_period: u64,
    pub effective_delay: u64,
    pub proposal_deposit: Uint128,
}

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
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// StakeVotingTokens a user can stake their mirror token to receive rewards
    /// or do vote on polls
    StakeVotingTokens {},
    /// CreatePoll need to receive deposit from a proposer
    CreatePoll {
        description: String,
        execute_msg: Option<ExecuteMsg>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ExecuteMsg {
    pub contract: HumanAddr,
    pub msg: Binary,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    Stake { address: HumanAddr },
    Poll { poll_id: u64 },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: HumanAddr,
    pub mirror_token: HumanAddr,
    pub quorum: Decimal,
    pub threshold: Decimal,
    pub voting_period: u64,
    pub effective_delay: u64,
    pub proposal_deposit: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub poll_count: u64,
    pub total_share: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct PollResponse {
    pub creator: HumanAddr,
    pub status: PollStatus,
    pub end_height: u64,
    pub description: String,
    pub deposit_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct CreatePollResponse {
    pub poll_id: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct PollCountResponse {
    pub poll_count: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct StakeResponse {
    pub balance: Uint128,
    pub share: Uint128,
}
