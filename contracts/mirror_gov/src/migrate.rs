use cosmwasm_std::{Order, StdError, StdResult, Storage};
use cosmwasm_storage::Bucket;

use mirror_protocol::gov::PollStatus;

use std::convert::TryInto;

static PREFIX_POLL_INDEXER_OLD: &[u8] = b"poll_voter";
static PREFIX_POLL_INDEXER: &[u8] = b"poll_indexer";

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

#[cfg(test)]
mod migrate_tests {
    use super::*;
    use crate::state::poll_indexer_store;
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
}
