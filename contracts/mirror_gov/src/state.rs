use cosmwasm_std::{
    Binary, CanonicalAddr, Decimal, Order, ReadonlyStorage, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{
    bucket, bucket_read, singleton, singleton_read, Bucket, ReadonlyBucket, ReadonlySingleton,
    Singleton,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use mirror_protocol::common::OrderBy;
use mirror_protocol::gov::{PollStatus, VoterInfo};

static KEY_CONFIG: &[u8] = b"config";
static KEY_STATE: &[u8] = b"state";

static PREFIX_POLL_VOTER: &[u8] = b"poll_voter";
static PREFIX_POLL: &[u8] = b"poll";
static PREFIX_BANK: &[u8] = b"bank";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub mirror_token: CanonicalAddr,
    pub quorum: Decimal,
    pub threshold: Decimal,
    pub voting_period: u64,
    pub effective_delay: u64,
    pub expiration_period: u64,
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
    pub share: Uint128,                        // total staked balance
    pub locked_balance: Vec<(u64, VoterInfo)>, // maps poll_id to weight voted
    pub participated_polls: Vec<u64>,          // poll_id
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Poll {
    pub id: u64,
    pub creator: CanonicalAddr,
    pub status: PollStatus,
    pub yes_votes: Uint128,
    pub no_votes: Uint128,
    pub end_height: u64,
    pub title: String,
    pub description: String,
    pub link: Option<String>,
    pub execute_data: Option<ExecuteData>,
    pub deposit_amount: Uint128,
    /// Total balance at the end poll
    pub total_balance_at_end_poll: Option<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ExecuteData {
    pub contract: CanonicalAddr,
    pub msg: Binary,
}

pub fn config_store<S: Storage>(storage: &mut S) -> Singleton<S, Config> {
    singleton(storage, KEY_CONFIG)
}

pub fn config_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, Config> {
    singleton_read(storage, KEY_CONFIG)
}

pub fn state_store<S: Storage>(storage: &mut S) -> Singleton<S, State> {
    singleton(storage, KEY_STATE)
}

pub fn state_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, State> {
    singleton_read(storage, KEY_STATE)
}

pub fn poll_store<S: Storage>(storage: &mut S) -> Bucket<S, Poll> {
    bucket(PREFIX_POLL, storage)
}

pub fn poll_read<S: ReadonlyStorage>(storage: &S) -> ReadonlyBucket<S, Poll> {
    bucket_read(PREFIX_POLL, storage)
}

pub fn poll_indexer_store<'a, S: Storage>(
    storage: &'a mut S,
    status: &PollStatus,
) -> Bucket<'a, S, bool> {
    Bucket::multilevel(&[PREFIX_POLL_VOTER, status.to_string().as_bytes()], storage)
}

pub fn poll_voter_store<S: Storage>(storage: &mut S, poll_id: u64) -> Bucket<S, VoterInfo> {
    Bucket::multilevel(&[PREFIX_POLL_VOTER, &poll_id.to_be_bytes()], storage)
}

pub fn poll_voter_read<S: ReadonlyStorage>(
    storage: &S,
    poll_id: u64,
) -> ReadonlyBucket<S, VoterInfo> {
    ReadonlyBucket::multilevel(&[PREFIX_POLL_VOTER, &poll_id.to_be_bytes()], storage)
}

pub fn poll_all_voters<S: ReadonlyStorage>(
    storage: &S,
    poll_id: u64,
) -> StdResult<Vec<CanonicalAddr>> {
    let voters: ReadonlyBucket<S, VoterInfo> =
        ReadonlyBucket::multilevel(&[PREFIX_POLL_VOTER, &poll_id.to_be_bytes()], storage);

    voters
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (k, _) = item?;
            Ok(CanonicalAddr::from(k))
        })
        .collect()
}

pub fn read_poll_voters<'a, S: ReadonlyStorage>(
    storage: &'a S,
    poll_id: u64,
    start_after: Option<CanonicalAddr>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<(CanonicalAddr, VoterInfo)>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Asc) => (calc_range_start_addr(start_after), None, OrderBy::Asc),
        _ => (None, calc_range_end_addr(start_after), OrderBy::Desc),
    };

    let voters: ReadonlyBucket<'a, S, VoterInfo> =
        ReadonlyBucket::multilevel(&[PREFIX_POLL_VOTER, &poll_id.to_be_bytes()], storage);
    voters
        .range(start.as_deref(), end.as_deref(), order_by.into())
        .take(limit)
        .map(|item| {
            let (k, v) = item?;
            Ok((CanonicalAddr::from(k), v))
        })
        .collect()
}

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub fn read_polls<'a, S: ReadonlyStorage>(
    storage: &'a S,
    filter: Option<PollStatus>,
    start_after: Option<u64>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<Vec<Poll>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Asc) => (calc_range_start(start_after), None, OrderBy::Asc),
        _ => (None, calc_range_end(start_after), OrderBy::Desc),
    };

    if let Some(status) = filter {
        let poll_indexer: ReadonlyBucket<'a, S, bool> = ReadonlyBucket::multilevel(
            &[PREFIX_POLL_VOTER, status.to_string().as_bytes()],
            storage,
        );
        poll_indexer
            .range(start.as_deref(), end.as_deref(), order_by.into())
            .take(limit)
            .map(|item| {
                let (k, _) = item?;
                poll_read(storage).load(&k)
            })
            .collect()
    } else {
        let polls: ReadonlyBucket<'a, S, Poll> = ReadonlyBucket::new(PREFIX_POLL, storage);

        polls
            .range(start.as_deref(), end.as_deref(), order_by.into())
            .take(limit)
            .map(|item| {
                let (_, v) = item?;
                Ok(v)
            })
            .collect()
    }
}

pub fn bank_store<S: Storage>(storage: &mut S) -> Bucket<S, TokenManager> {
    bucket(PREFIX_BANK, storage)
}

pub fn bank_read<S: Storage>(storage: &S) -> ReadonlyBucket<S, TokenManager> {
    bucket_read(PREFIX_BANK, storage)
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_start(start_after: Option<u64>) -> Option<Vec<u8>> {
    start_after.map(|id| {
        let mut v = id.to_be_bytes().to_vec();
        v.push(1);
        v
    })
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_end(start_after: Option<u64>) -> Option<Vec<u8>> {
    start_after.map(|id| id.to_be_bytes().to_vec())
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_start_addr(start_after: Option<CanonicalAddr>) -> Option<Vec<u8>> {
    start_after.map(|addr| {
        let mut v = addr.as_slice().to_vec();
        v.push(1);
        v
    })
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_end_addr(start_after: Option<CanonicalAddr>) -> Option<Vec<u8>> {
    start_after.map(|addr| addr.as_slice().to_vec())
}
