use cosmwasm_std::{Binary, CanonicalAddr, Decimal, Storage, Uint128};
use cosmwasm_storage::{
    bucket, bucket_read, singleton, singleton_read, Bucket, ReadonlyBucket, ReadonlySingleton,
    Singleton,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static CONFIG_KEY: &[u8] = b"config";
static STATE_KEY: &[u8] = b"state";
static POLL_KEY: &[u8] = b"polls";
static BANK_KEY: &[u8] = b"bank";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub quorum: Decimal,
    pub threshold: Decimal,
    pub voting_period: u64,
    pub proposal_deposit: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub contract_addr: CanonicalAddr,
    pub poll_count: u64,
    pub total_share: Uint128,
    pub total_deposit: Uint128,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenManager {
    pub share: Uint128,                    // total staked balance
    pub locked_share: Vec<(u64, Uint128)>, // maps poll_id to weight voted
    pub participated_polls: Vec<u64>,      // poll_id
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VoteOption {
    YES,
    NO,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Voter {
    pub vote: VoteOption,
    pub share: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PollStatus {
    InProgress,
    Tally,
    Passed,
    Rejected,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Poll {
    pub creator: CanonicalAddr,
    pub status: PollStatus,
    pub yes_votes: Uint128,
    pub no_votes: Uint128,
    pub voters: Vec<CanonicalAddr>,
    pub voter_info: Vec<Voter>,
    pub end_height: u64,
    pub description: String,
    pub execute_data: Option<ExecuteData>,
    pub deposit_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ExecuteData {
    pub contract: CanonicalAddr,
    pub msg: Binary,
}

pub fn config_store<S: Storage>(storage: &mut S) -> Singleton<S, Config> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, Config> {
    singleton_read(storage, CONFIG_KEY)
}

pub fn state_store<S: Storage>(storage: &mut S) -> Singleton<S, State> {
    singleton(storage, STATE_KEY)
}

pub fn state_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, State> {
    singleton_read(storage, STATE_KEY)
}

pub fn poll_store<S: Storage>(storage: &mut S) -> Bucket<S, Poll> {
    bucket(POLL_KEY, storage)
}

pub fn poll_read<S: Storage>(storage: &S) -> ReadonlyBucket<S, Poll> {
    bucket_read(POLL_KEY, storage)
}

pub fn bank_store<S: Storage>(storage: &mut S) -> Bucket<S, TokenManager> {
    bucket(BANK_KEY, storage)
}

pub fn bank_read<S: Storage>(storage: &S) -> ReadonlyBucket<S, TokenManager> {
    bucket_read(BANK_KEY, storage)
}
