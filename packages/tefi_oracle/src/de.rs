use cosmwasm_std::{Addr, StdError, StdResult};
use std::array::TryFromSliceError;
use std::convert::TryInto;

/// This code is mostly just a copy of the necessary functions from storage-plus
/// but not introduced until cw-storage-plus 0.10.0.  Can remove this
/// file entirely once we upgrade cw-storage-plus and use the prefix_de/range_de
/// methods instead.

pub fn deserialize_key<K: KeyDeserialize>(key: Vec<u8>) -> StdResult<K::Output> {
    K::from_vec(key)
}

pub trait KeyDeserialize {
    type Output: Sized;

    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output>;
}

impl KeyDeserialize for u64 {
    type Output = u64;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Ok(u64::from_be_bytes(value.as_slice().try_into().map_err(
            |err: TryFromSliceError| StdError::generic_err(err.to_string()),
        )?))
    }
}

impl KeyDeserialize for String {
    type Output = String;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        String::from_utf8(value).map_err(StdError::invalid_utf8)
    }
}

impl KeyDeserialize for Addr {
    type Output = Addr;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Ok(Addr::unchecked(String::from_vec(value)?))
    }
}
