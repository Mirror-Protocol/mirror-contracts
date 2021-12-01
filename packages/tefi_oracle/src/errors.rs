use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("Sender is not authorized to execute this operation")]
    Unauthorized {},

    #[error("The proxy address is not registered")]
    ProxyNotRegistered {},

    #[error("The asset token is not registered")]
    AssetNotRegistered {},

    #[error("Quote asset not supported")]
    InvalidQuote {},

    #[error("There is no price available with the requested constrains")]
    PriceNotAvailable {},

    #[error("Proxy error: {reason}")]
    ProxyError { reason: String },
}
