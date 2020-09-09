use crate::state::PollStatus;
use cosmwasm_std::{Binary, HumanAddr, Uint128};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub mirror_token: HumanAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    CastVote {
        poll_id: u64,
        vote: String,
        weight: Uint128,
    },
    WithdrawVotingTokens {
        amount: Option<Uint128>,
    },
    CreatePoll {
        quorum_percentage: Option<u8>,
        description: String,
        start_height: Option<u64>,
        end_height: Option<u64>,
        execute_msg: Option<ExecuteMsg>,
    },
    EndPoll {
        poll_id: u64,
    },
    /// Receive is cw20 token send handler
    Receive(Cw20ReceiveMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    StakeVotingTokens {},
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
    TokenStake { address: HumanAddr },
    Poll { poll_id: u64 },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct PollResponse {
    pub creator: HumanAddr,
    pub status: PollStatus,
    pub quorum_percentage: Option<u8>,
    pub end_height: Option<u64>,
    pub start_height: Option<u64>,
    pub description: String,
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
pub struct TokenStakeResponse {
    pub token_balance: Uint128,
}
