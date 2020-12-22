use cosmwasm_std::{
    Api, CanonicalAddr, Extern, Order, Querier, ReadonlyStorage, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{Bucket, ReadonlyBucket};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{
    bank_store, poll_store, poll_voter_store, ExecuteData, Poll, PollStatus, TokenManager,
    VoteOption, VoterInfo,
};

static PREFIX_POLL_VOTER: &[u8] = b"poll_voter";
static PREFIX_POLL: &[u8] = b"poll";
static PREFIX_BANK: &[u8] = b"bank";

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OldTokenManager {
    pub share: Uint128,                         // total staked balance
    pub locked_share: Vec<(u64, OldVoterInfo)>, // maps poll_id to weight voted
    pub participated_polls: Vec<u64>,           // poll_id
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OldVoterInfo {
    pub vote: VoteOption,
    pub share: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OldPoll {
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
    /// Total share at the end poll
    pub total_share_at_end_poll: Option<Uint128>,
}

pub fn read_all_old_polls<'a, S: ReadonlyStorage>(storage: &'a S) -> StdResult<Vec<OldPoll>> {
    let polls: ReadonlyBucket<'a, S, OldPoll> = ReadonlyBucket::new(PREFIX_POLL, storage);
    polls
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item?;
            Ok(v)
        })
        .collect()
}

pub fn read_all_old_voters<S: ReadonlyStorage>(
    storage: &S,
    poll_id: u64,
) -> StdResult<Vec<(CanonicalAddr, OldVoterInfo)>> {
    let voters: ReadonlyBucket<S, OldVoterInfo> =
        ReadonlyBucket::multilevel(&[PREFIX_POLL_VOTER, &poll_id.to_be_bytes()], storage);

    voters
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (k, v) = item?;
            Ok((CanonicalAddr::from(k), v))
        })
        .collect()
}

pub fn read_all_old_stakers<S: Storage>(
    storage: &S,
) -> StdResult<Vec<(CanonicalAddr, OldTokenManager)>> {
    let stakers: ReadonlyBucket<S, OldTokenManager> = ReadonlyBucket::new(PREFIX_BANK, storage);

    stakers
        .range(None, None, Order::Ascending)
        .map(|item| {
            let (k, v) = item?;
            Ok((CanonicalAddr::from(k), v))
        })
        .collect()
}

pub fn migrate_share_to_balance<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
) -> StdResult<()> {
    let old_stakers: Vec<(CanonicalAddr, OldTokenManager)> = read_all_old_stakers(&deps.storage)?;
    let mut bank_bucket: Bucket<S, TokenManager> = bank_store(&mut deps.storage);
    for old_staker in old_stakers {
        bank_bucket.save(
            &old_staker.0.as_slice(),
            &TokenManager {
                share: old_staker.1.share,
                participated_polls: old_staker.1.participated_polls,
                locked_balance: old_staker
                    .1
                    .locked_share
                    .iter()
                    .map(|v| {
                        (
                            v.0,
                            VoterInfo {
                                vote: v.1.vote.clone(),
                                balance: v.1.share,
                            },
                        )
                    })
                    .collect(),
            },
        )?;
    }

    let old_polls: Vec<OldPoll> = read_all_old_polls(&deps.storage)?;
    let mut poll_bucket: Bucket<S, Poll> = poll_store(&mut deps.storage);
    for old_poll in old_polls.clone() {
        poll_bucket.save(
            &old_poll.id.to_be_bytes(),
            &Poll {
                id: old_poll.id,
                creator: old_poll.creator,
                status: old_poll.status,
                yes_votes: old_poll.yes_votes,
                no_votes: old_poll.no_votes,
                end_height: old_poll.end_height,
                title: old_poll.title,
                description: old_poll.description,
                link: old_poll.link,
                execute_data: old_poll.execute_data,
                deposit_amount: old_poll.deposit_amount,
                total_balance_at_end_poll: old_poll.total_share_at_end_poll,
            },
        )?;
    }

    for old_poll in old_polls {
        let voters: Vec<(CanonicalAddr, OldVoterInfo)> =
            read_all_old_voters(&deps.storage, old_poll.id)?;
        let mut voter_bucket: Bucket<S, VoterInfo> =
            poll_voter_store(&mut deps.storage, old_poll.id);

        for voter in voters {
            voter_bucket.save(
                voter.0.as_slice(),
                &VoterInfo {
                    vote: voter.1.vote,
                    balance: voter.1.share,
                },
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{CanonicalAddr, HumanAddr};
    use cosmwasm_storage::bucket;
    fn old_bank_store<S: Storage>(storage: &mut S) -> Bucket<S, OldTokenManager> {
        bucket(PREFIX_BANK, storage)
    }

    fn old_poll_voter_store<S: Storage>(storage: &mut S, poll_id: u64) -> Bucket<S, OldVoterInfo> {
        Bucket::multilevel(&[PREFIX_POLL_VOTER, &poll_id.to_be_bytes()], storage)
    }

    fn old_poll_store<S: Storage>(storage: &mut S) -> Bucket<S, OldPoll> {
        bucket(PREFIX_POLL, storage)
    }

    fn read_all_stakers<S: Storage>(storage: &S) -> StdResult<Vec<(CanonicalAddr, TokenManager)>> {
        let stakers: ReadonlyBucket<S, TokenManager> = ReadonlyBucket::new(PREFIX_BANK, storage);
        stakers
            .range(None, None, Order::Ascending)
            .map(|item| {
                let (k, v) = item?;
                Ok((CanonicalAddr::from(k), v))
            })
            .collect()
    }

    fn read_all_polls<'a, S: ReadonlyStorage>(storage: &'a S) -> StdResult<Vec<Poll>> {
        let polls: ReadonlyBucket<'a, S, Poll> = ReadonlyBucket::new(PREFIX_POLL, storage);
        polls
            .range(None, None, Order::Ascending)
            .map(|item| {
                let (_, v) = item?;
                Ok(v)
            })
            .collect()
    }

    fn read_all_voters<S: ReadonlyStorage>(
        storage: &S,
        poll_id: u64,
    ) -> StdResult<Vec<(CanonicalAddr, VoterInfo)>> {
        let voters: ReadonlyBucket<S, VoterInfo> =
            ReadonlyBucket::multilevel(&[PREFIX_POLL_VOTER, &poll_id.to_be_bytes()], storage);
        voters
            .range(None, None, Order::Ascending)
            .map(|item| {
                let (k, v) = item?;
                Ok((CanonicalAddr::from(k), v))
            })
            .collect()
    }

    #[test]
    fn poll_voters_migration_test() {
        let mut deps = mock_dependencies(20, &[]);

        let acc0 = HumanAddr::from("addr0000");
        let acc1 = HumanAddr::from("addr0001");
        let acc2 = HumanAddr::from("addr0002");

        let acc0_raw = deps.api.canonical_address(&acc0).unwrap();
        let acc1_raw = deps.api.canonical_address(&acc1).unwrap();
        let acc2_raw = deps.api.canonical_address(&acc2).unwrap();

        let mut old_poll_bucket = old_poll_store(&mut deps.storage);
        old_poll_bucket
            .save(
                &1u64.to_be_bytes(),
                &OldPoll {
                    id: 1u64,
                    creator: acc0_raw.clone(),
                    status: PollStatus::InProgress {},
                    yes_votes: Uint128::from(1u128),
                    no_votes: Uint128::from(1u128),
                    end_height: 100u64,
                    title: "TITLE".to_string(),
                    description: "DESC".to_string(),
                    link: Some("LINK".to_string()),
                    execute_data: None,
                    deposit_amount: Uint128::from(1u128),
                    total_share_at_end_poll: None,
                },
            )
            .unwrap();
        old_poll_bucket
            .save(
                &2u64.to_be_bytes(),
                &OldPoll {
                    id: 2u64,
                    creator: acc1_raw.clone(),
                    status: PollStatus::Passed {},
                    yes_votes: Uint128::from(2u128),
                    no_votes: Uint128::from(2u128),
                    end_height: 200u64,
                    title: "TITLE".to_string(),
                    description: "DESC".to_string(),
                    link: Some("LINK".to_string()),
                    execute_data: None,
                    deposit_amount: Uint128::from(100u128),
                    total_share_at_end_poll: Some(Uint128(300u128)),
                },
            )
            .unwrap();

        let mut old_poll_voter_bucket = old_poll_voter_store(&mut deps.storage, 1u64);
        old_poll_voter_bucket
            .save(
                &acc0_raw.as_slice(),
                &OldVoterInfo {
                    vote: VoteOption::Yes,
                    share: Uint128::from(100u128),
                },
            )
            .unwrap();
        old_poll_voter_bucket
            .save(
                &acc1_raw.as_slice(),
                &OldVoterInfo {
                    vote: VoteOption::No,
                    share: Uint128::from(200u128),
                },
            )
            .unwrap();

        let mut old_poll_voter_bucket = old_poll_voter_store(&mut deps.storage, 2u64);
        old_poll_voter_bucket
            .save(
                &acc1_raw.as_slice(),
                &OldVoterInfo {
                    vote: VoteOption::Yes,
                    share: Uint128::from(100u128),
                },
            )
            .unwrap();
        old_poll_voter_bucket
            .save(
                &acc2_raw.as_slice(),
                &OldVoterInfo {
                    vote: VoteOption::No,
                    share: Uint128::from(200u128),
                },
            )
            .unwrap();

        migrate_share_to_balance(&mut deps).unwrap();

        let voters = read_all_voters(&deps.storage, 1u64).unwrap();
        assert_eq!(
            voters,
            vec![
                (
                    acc0_raw,
                    VoterInfo {
                        vote: VoteOption::Yes,
                        balance: Uint128::from(100u128),
                    }
                ),
                (
                    acc1_raw.clone(),
                    VoterInfo {
                        vote: VoteOption::No,
                        balance: Uint128::from(200u128),
                    }
                )
            ]
        );

        let voters = read_all_voters(&deps.storage, 2u64).unwrap();
        assert_eq!(
            voters,
            vec![
                (
                    acc1_raw,
                    VoterInfo {
                        vote: VoteOption::Yes,
                        balance: Uint128::from(100u128),
                    }
                ),
                (
                    acc2_raw,
                    VoterInfo {
                        vote: VoteOption::No,
                        balance: Uint128::from(200u128),
                    }
                )
            ]
        );
    }

    #[test]
    fn poll_migration_test() {
        let mut deps = mock_dependencies(20, &[]);

        let acc0 = HumanAddr::from("addr0000");
        let acc1 = HumanAddr::from("addr0001");
        let acc2 = HumanAddr::from("addr0002");

        let acc0_raw = deps.api.canonical_address(&acc0).unwrap();
        let acc1_raw = deps.api.canonical_address(&acc1).unwrap();
        let acc2_raw = deps.api.canonical_address(&acc2).unwrap();

        let mut old_poll_bucket = old_poll_store(&mut deps.storage);
        old_poll_bucket
            .save(
                &1u64.to_be_bytes(),
                &OldPoll {
                    id: 1u64,
                    creator: acc0_raw.clone(),
                    status: PollStatus::InProgress {},
                    yes_votes: Uint128::from(1u128),
                    no_votes: Uint128::from(1u128),
                    end_height: 100u64,
                    title: "TITLE".to_string(),
                    description: "DESC".to_string(),
                    link: Some("LINK".to_string()),
                    execute_data: None,
                    deposit_amount: Uint128::from(1u128),
                    total_share_at_end_poll: None,
                },
            )
            .unwrap();
        old_poll_bucket
            .save(
                &2u64.to_be_bytes(),
                &OldPoll {
                    id: 2u64,
                    creator: acc1_raw.clone(),
                    status: PollStatus::Passed {},
                    yes_votes: Uint128::from(2u128),
                    no_votes: Uint128::from(2u128),
                    end_height: 200u64,
                    title: "TITLE".to_string(),
                    description: "DESC".to_string(),
                    link: Some("LINK".to_string()),
                    execute_data: None,
                    deposit_amount: Uint128::from(100u128),
                    total_share_at_end_poll: Some(Uint128(300u128)),
                },
            )
            .unwrap();
        old_poll_bucket
            .save(
                &3u64.to_be_bytes(),
                &OldPoll {
                    id: 3u64,
                    creator: acc2_raw.clone(),
                    status: PollStatus::Executed {},
                    yes_votes: Uint128::from(3u128),
                    no_votes: Uint128::from(3u128),
                    end_height: 300u64,
                    title: "TITLE".to_string(),
                    description: "DESC".to_string(),
                    link: Some("LINK".to_string()),
                    execute_data: None,
                    deposit_amount: Uint128::from(100u128),
                    total_share_at_end_poll: Some(Uint128(300u128)),
                },
            )
            .unwrap();

        migrate_share_to_balance(&mut deps).unwrap();

        let polls = read_all_polls(&deps.storage).unwrap();
        assert_eq!(
            polls,
            vec![
                Poll {
                    id: 1u64,
                    creator: acc0_raw,
                    status: PollStatus::InProgress {},
                    yes_votes: Uint128::from(1u128),
                    no_votes: Uint128::from(1u128),
                    end_height: 100u64,
                    title: "TITLE".to_string(),
                    description: "DESC".to_string(),
                    link: Some("LINK".to_string()),
                    execute_data: None,
                    deposit_amount: Uint128::from(1u128),
                    total_balance_at_end_poll: None,
                },
                Poll {
                    id: 2u64,
                    creator: acc1_raw,
                    status: PollStatus::Passed {},
                    yes_votes: Uint128::from(2u128),
                    no_votes: Uint128::from(2u128),
                    end_height: 200u64,
                    title: "TITLE".to_string(),
                    description: "DESC".to_string(),
                    link: Some("LINK".to_string()),
                    execute_data: None,
                    deposit_amount: Uint128::from(100u128),
                    total_balance_at_end_poll: Some(Uint128(300u128)),
                },
                Poll {
                    id: 3u64,
                    creator: acc2_raw,
                    status: PollStatus::Executed {},
                    yes_votes: Uint128::from(3u128),
                    no_votes: Uint128::from(3u128),
                    end_height: 300u64,
                    title: "TITLE".to_string(),
                    description: "DESC".to_string(),
                    link: Some("LINK".to_string()),
                    execute_data: None,
                    deposit_amount: Uint128::from(100u128),
                    total_balance_at_end_poll: Some(Uint128(300u128)),
                }
            ]
        );
    }

    #[test]
    fn bank_migration_test() {
        let mut deps = mock_dependencies(20, &[]);

        let acc0 = HumanAddr::from("addr0000");
        let acc1 = HumanAddr::from("addr0001");
        let acc2 = HumanAddr::from("addr0002");

        let acc0_raw = deps.api.canonical_address(&acc0).unwrap();
        let acc1_raw = deps.api.canonical_address(&acc1).unwrap();
        let acc2_raw = deps.api.canonical_address(&acc2).unwrap();

        let mut old_bank_bucket = old_bank_store(&mut deps.storage);

        old_bank_bucket
            .save(
                &acc0_raw.as_slice(),
                &OldTokenManager {
                    share: Uint128::from(1u128),
                    locked_share: vec![
                        (
                            1u64,
                            OldVoterInfo {
                                vote: VoteOption::Yes,
                                share: Uint128::from(1u128),
                            },
                        ),
                        (
                            2u64,
                            OldVoterInfo {
                                vote: VoteOption::No,
                                share: Uint128::from(2u128),
                            },
                        ),
                    ],
                    participated_polls: vec![1u64, 2u64],
                },
            )
            .unwrap();

        old_bank_bucket
            .save(
                &acc1_raw.as_slice(),
                &OldTokenManager {
                    share: Uint128::from(2u128),
                    locked_share: vec![
                        (
                            2u64,
                            OldVoterInfo {
                                vote: VoteOption::Yes,
                                share: Uint128::from(2u128),
                            },
                        ),
                        (
                            3u64,
                            OldVoterInfo {
                                vote: VoteOption::No,
                                share: Uint128::from(3u128),
                            },
                        ),
                    ],
                    participated_polls: vec![2u64, 3u64],
                },
            )
            .unwrap();

        old_bank_bucket
            .save(
                &acc2_raw.as_slice(),
                &OldTokenManager {
                    share: Uint128::from(3u128),
                    locked_share: vec![
                        (
                            3u64,
                            OldVoterInfo {
                                vote: VoteOption::Yes,
                                share: Uint128::from(3u128),
                            },
                        ),
                        (
                            4u64,
                            OldVoterInfo {
                                vote: VoteOption::No,
                                share: Uint128::from(4u128),
                            },
                        ),
                    ],
                    participated_polls: vec![3u64, 4u64],
                },
            )
            .unwrap();

        migrate_share_to_balance(&mut deps).unwrap();

        let stakers = read_all_stakers(&deps.storage).unwrap();
        assert_eq!(
            stakers,
            vec![
                (
                    acc0_raw,
                    TokenManager {
                        share: Uint128::from(1u128),
                        locked_balance: vec![
                            (
                                1u64,
                                VoterInfo {
                                    vote: VoteOption::Yes,
                                    balance: Uint128::from(1u128),
                                },
                            ),
                            (
                                2u64,
                                VoterInfo {
                                    vote: VoteOption::No,
                                    balance: Uint128::from(2u128),
                                },
                            ),
                        ],
                        participated_polls: vec![1u64, 2u64],
                    }
                ),
                (
                    acc1_raw,
                    TokenManager {
                        share: Uint128::from(2u128),
                        locked_balance: vec![
                            (
                                2u64,
                                VoterInfo {
                                    vote: VoteOption::Yes,
                                    balance: Uint128::from(2u128),
                                },
                            ),
                            (
                                3u64,
                                VoterInfo {
                                    vote: VoteOption::No,
                                    balance: Uint128::from(3u128),
                                },
                            ),
                        ],
                        participated_polls: vec![2u64, 3u64],
                    }
                ),
                (
                    acc2_raw,
                    TokenManager {
                        share: Uint128::from(3u128),
                        locked_balance: vec![
                            (
                                3u64,
                                VoterInfo {
                                    vote: VoteOption::Yes,
                                    balance: Uint128::from(3u128),
                                },
                            ),
                            (
                                4u64,
                                VoterInfo {
                                    vote: VoteOption::No,
                                    balance: Uint128::from(4u128),
                                },
                            ),
                        ],
                        participated_polls: vec![3u64, 4u64],
                    }
                )
            ]
        )
    }
}
