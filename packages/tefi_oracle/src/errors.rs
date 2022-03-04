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

    #[error("The proxy is not registered as a source for this asset")]
    ProxyNotRegistered {},

    #[error("The proxy was already registered for this asset")]
    ProxyAlreadyRegistered {},

    #[error("The proxy address is not whitelisted")]
    ProxyNotWhitelisted {},

    #[error("This asset exceeds the maximum proxies per symbol ({max})")]
    TooManyProxiesForSymbol { max: u8 },

    #[error("Can not whiteliste more than maximum ({max})")]
    TooManyWhitelistedProxies { max: u8 },

    #[error("The symbol is not registered")]
    SymbolNotRegistered {},

    #[error("This asset token is not mapped to any symbol")]
    MappingNotFound {},

    #[error("Quote asset not supported")]
    InvalidQuote {},

    #[error("Invalid priority list")]
    InvalidPriorities {},

    #[error("There is no price available with the requested constrains")]
    PriceNotAvailable {},

    #[error("Proxy error: {reason}")]
    ProxyError { reason: String },
}
