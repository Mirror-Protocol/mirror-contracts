use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, Env, Order, StdError, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlySingleton, Singleton};

use crate::state::{Config, ExecuteData, Poll, State};
use mirror_protocol::gov::PollStatus;

use std::convert::TryInto;

#[cfg(test)]
use crate::state::{config_read, poll_read, state_read};

static PREFIX_POLL_INDEXER_OLD: &[u8] = b"poll_voter";
static PREFIX_POLL_INDEXER: &[u8] = b"poll_indexer";
static KEY_CONFIG: &[u8] = b"config";
static KEY_STATE: &[u8] = b"state";
static PREFIX_POLL: &[u8] = b"poll";

#[cfg(test)]
pub fn poll_indexer_old_store<'a, S: Storage>(
    storage: &'a mut S,
    status: &PollStatus,
) -> Bucket<'a, S, bool> {
    Bucket::multilevel(
        &[PREFIX_POLL_INDEXER_OLD, status.to_string().as_bytes()],
        storage,
    )
}
#[cfg(test)]
pub fn polls_old_store<'a, S: Storage>(storage: &'a mut S) -> Bucket<'a, S, LegacyPoll> {
    Bucket::new(PREFIX_POLL, storage)
}
#[cfg(test)]
pub fn state_old_store<'a, S: Storage>(storage: &'a mut S) -> Singleton<'a, S, LegacyState> {
    Singleton::new(storage, KEY_STATE)
}
#[cfg(test)]
pub fn config_old_store<'a, S: Storage>(storage: &'a mut S) -> Singleton<'a, S, LegacyConfig> {
    Singleton::new(storage, KEY_CONFIG)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyConfig {
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
pub struct LegacyState {
    pub contract_addr: CanonicalAddr,
    pub poll_count: u64,
    pub total_share: Uint128,
    pub total_deposit: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyPoll {
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
    pub total_balance_at_end_poll: Option<Uint128>,
}

pub fn migrate_poll_indexer<S: Storage>(storage: &mut S, status: &PollStatus) -> StdResult<()> {
    let mut old_indexer_bucket: Bucket<S, bool> = Bucket::multilevel(
        &[PREFIX_POLL_INDEXER_OLD, status.to_string().as_bytes()],
        storage,
    );

    let mut poll_ids: Vec<u64> = vec![];
    for item in old_indexer_bucket.range(None, None, Order::Ascending) {
        let (k, _) = item?;
        poll_ids.push(bytes_to_u64(&k)?);
    }

    for id in poll_ids.clone().into_iter() {
        old_indexer_bucket.remove(&id.to_be_bytes());
    }

    let mut new_indexer_bucket: Bucket<S, bool> = Bucket::multilevel(
        &[PREFIX_POLL_INDEXER, status.to_string().as_bytes()],
        storage,
    );

    for id in poll_ids.into_iter() {
        new_indexer_bucket.save(&id.to_be_bytes(), &true)?;
    }

    return Ok(());
}

fn bytes_to_u64(data: &[u8]) -> StdResult<u64> {
    match data[0..8].try_into() {
        Ok(bytes) => Ok(u64::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 8 byte expected.",
        )),
    }
}

pub fn migrate_config<S: Storage>(
    storage: &mut S,
    voter_weight: Decimal,
    snapshot_period: u64,
    voting_period: u64,
    effective_delay: u64,
    expiration_period: u64,
) -> StdResult<()> {
    let legacty_store: ReadonlySingleton<S, LegacyConfig> = singleton_read(storage, KEY_CONFIG);
    let legacy_config: LegacyConfig = legacty_store.load()?;
    let config = Config {
        mirror_token: legacy_config.mirror_token,
        owner: legacy_config.owner,
        quorum: legacy_config.quorum,
        threshold: legacy_config.threshold,
        voting_period,
        effective_delay,
        expiration_period,
        proposal_deposit: legacy_config.proposal_deposit,
        voter_weight: voter_weight,
        snapshot_period: snapshot_period,
    };
    let mut store: Singleton<S, Config> = singleton(storage, KEY_CONFIG);
    store.save(&config)?;
    Ok(())
}

pub fn migrate_state<S: Storage>(storage: &mut S) -> StdResult<()> {
    let legacy_store: ReadonlySingleton<S, LegacyState> = singleton_read(storage, KEY_STATE);
    let legacy_state: LegacyState = legacy_store.load()?;
    let state = State {
        contract_addr: legacy_state.contract_addr,
        poll_count: legacy_state.poll_count,
        total_share: legacy_state.total_share,
        total_deposit: legacy_state.total_deposit,
        pending_voting_rewards: Uint128::zero(),
    };
    let mut store: Singleton<S, State> = singleton(storage, KEY_STATE);
    store.save(&state)?;
    Ok(())
}

pub fn migrate_polls<S: Storage>(storage: &mut S, env: Env) -> StdResult<()> {
    let mut legacy_polls_bucket: Bucket<S, LegacyPoll> = Bucket::new(PREFIX_POLL, storage);

    let mut read_polls: Vec<(u64, LegacyPoll)> = vec![];
    for item in legacy_polls_bucket.range(None, None, Order::Ascending) {
        let (k, p) = item?;
        read_polls.push((bytes_to_u64(&k)?, p));
    }

    for (id, _) in read_polls.clone().into_iter() {
        legacy_polls_bucket.remove(&id.to_be_bytes());
    }

    let mut new_polls_bucket: Bucket<S, Poll> = Bucket::new(PREFIX_POLL, storage);

    for (id, poll) in read_polls.into_iter() {
        let end_time = if poll.end_height >= env.block.height {
            let time_to_end: u64 = (poll.end_height - env.block.height) * 13 / 2; // 6.5 avg block time

            env.block.time + time_to_end
        } else {
            let time_since_end: u64 = (env.block.height - poll.end_height) * 13 / 2;

            env.block.time - time_since_end as u64
        };
        let new_poll = &Poll {
            id: poll.id,
            creator: poll.creator,
            status: poll.status,
            yes_votes: poll.yes_votes,
            no_votes: poll.no_votes,
            abstain_votes: Uint128::zero(),
            end_time,
            title: poll.title,
            description: poll.description,
            link: poll.link,
            execute_data: poll.execute_data,
            deposit_amount: poll.deposit_amount,
            total_balance_at_end_poll: poll.total_balance_at_end_poll,
            voters_reward: Uint128::zero(),
            staked_amount: None,
        };
        new_polls_bucket.save(&id.to_be_bytes(), new_poll)?;
    }

    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use super::*;
    use crate::state::poll_indexer_store;
    use crate::tests::mock_env_height;
    use cosmwasm_std::testing::mock_dependencies;

    #[test]
    fn test_poll_indexer_migration() {
        let mut deps = mock_dependencies(20, &[]);
        poll_indexer_old_store(&mut deps.storage, &PollStatus::InProgress)
            .save(&1u64.to_be_bytes(), &true)
            .unwrap();

        poll_indexer_old_store(&mut deps.storage, &PollStatus::Executed)
            .save(&2u64.to_be_bytes(), &true)
            .unwrap();

        migrate_poll_indexer(&mut deps.storage, &PollStatus::InProgress).unwrap();
        migrate_poll_indexer(&mut deps.storage, &PollStatus::Executed).unwrap();
        migrate_poll_indexer(&mut deps.storage, &PollStatus::Passed).unwrap();

        assert_eq!(
            poll_indexer_store(&mut deps.storage, &PollStatus::InProgress)
                .load(&1u64.to_be_bytes())
                .unwrap(),
            true
        );

        assert_eq!(
            poll_indexer_store(&mut deps.storage, &PollStatus::Executed)
                .load(&2u64.to_be_bytes())
                .unwrap(),
            true
        );
    }

    #[test]
    fn test_polls_migration() {
        let mut deps = mock_dependencies(20, &[]);
        polls_old_store(&mut deps.storage)
            .save(
                &1u64.to_be_bytes(),
                &LegacyPoll {
                    id: 1u64,
                    creator: CanonicalAddr::default(),
                    status: PollStatus::Executed,
                    yes_votes: Uint128::zero(),
                    no_votes: Uint128::zero(),
                    end_height: 50u64,
                    title: "test".to_string(),
                    description: "description".to_string(),
                    link: None,
                    execute_data: None,
                    deposit_amount: Uint128::zero(),
                    total_balance_at_end_poll: None,
                },
            )
            .unwrap();
        polls_old_store(&mut deps.storage)
            .save(
                &2u64.to_be_bytes(),
                &LegacyPoll {
                    id: 2u64,
                    creator: CanonicalAddr::default(),
                    status: PollStatus::InProgress,
                    yes_votes: Uint128::zero(),
                    no_votes: Uint128::zero(),
                    end_height: 125u64,
                    title: "test2".to_string(),
                    description: "description".to_string(),
                    link: None,
                    execute_data: None,
                    deposit_amount: Uint128::zero(),
                    total_balance_at_end_poll: None,
                },
            )
            .unwrap();

        let env = mock_env_height("addr0000", &[], 100, 650);
        migrate_polls(&mut deps.storage, env).unwrap();

        let poll1: Poll = poll_read(&mut deps.storage)
            .load(&1u64.to_be_bytes())
            .unwrap();
        assert_eq!(
            poll1,
            Poll {
                id: 1u64,
                creator: CanonicalAddr::default(),
                status: PollStatus::Executed,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                end_time: 325u64, // 650 - (100 - 50) * 6.5
                title: "test".to_string(),
                description: "description".to_string(),
                link: None,
                execute_data: None,
                deposit_amount: Uint128::zero(),
                total_balance_at_end_poll: None,
                abstain_votes: Uint128::zero(),
                voters_reward: Uint128::zero(),
                staked_amount: None,
            }
        );
        let poll2: Poll = poll_read(&mut deps.storage)
            .load(&2u64.to_be_bytes())
            .unwrap();
        assert_eq!(
            poll2,
            Poll {
                id: 2u64,
                creator: CanonicalAddr::default(),
                status: PollStatus::InProgress,
                yes_votes: Uint128::zero(),
                no_votes: Uint128::zero(),
                end_time: 812u64, // 650 + 25 * 6.5
                title: "test2".to_string(),
                description: "description".to_string(),
                link: None,
                execute_data: None,
                deposit_amount: Uint128::zero(),
                total_balance_at_end_poll: None,
                abstain_votes: Uint128::zero(),
                voters_reward: Uint128::zero(),
                staked_amount: None,
            }
        );
    }

    #[test]
    fn test_config_migration() {
        let mut deps = mock_dependencies(20, &[]);
        let mut legacy_config_store = config_old_store(&mut deps.storage);
        legacy_config_store
            .save(&LegacyConfig {
                mirror_token: CanonicalAddr::default(),
                owner: CanonicalAddr::default(),
                quorum: Decimal::one(),
                threshold: Decimal::one(),
                voting_period: 100u64,
                effective_delay: 100u64,
                expiration_period: 100u64,
                proposal_deposit: Uint128(100000u128),
            })
            .unwrap();

        migrate_config(
            &mut deps.storage,
            Decimal::percent(50u64),
            50u64,
            200u64,
            100u64,
            75u64,
        )
        .unwrap();

        let config: Config = config_read(&deps.storage).load().unwrap();
        assert_eq!(
            config,
            Config {
                mirror_token: CanonicalAddr::default(),
                owner: CanonicalAddr::default(),
                quorum: Decimal::one(),
                threshold: Decimal::one(),
                voting_period: 200u64,
                effective_delay: 100u64,
                expiration_period: 75u64,
                proposal_deposit: Uint128(100000u128),
                voter_weight: Decimal::percent(50u64),
                snapshot_period: 50u64,
            }
        )
    }

    #[test]
    fn test_state_migration() {
        let mut deps = mock_dependencies(20, &[]);
        let mut legacy_state_store = state_old_store(&mut deps.storage);
        legacy_state_store
            .save(&LegacyState {
                contract_addr: CanonicalAddr::default(),
                poll_count: 0,
                total_share: Uint128::zero(),
                total_deposit: Uint128::zero(),
            })
            .unwrap();

        migrate_state(&mut deps.storage).unwrap();

        let state: State = state_read(&deps.storage).load().unwrap();
        assert_eq!(
            state,
            State {
                contract_addr: CanonicalAddr::default(),
                poll_count: 0,
                total_share: Uint128::zero(),
                total_deposit: Uint128::zero(),
                pending_voting_rewards: Uint128::zero(),
            }
        )
    }
}
