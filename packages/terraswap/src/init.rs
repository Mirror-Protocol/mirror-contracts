use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::AssetInfo;
use crate::hook::InitHook;
use cosmwasm_std::{CanonicalAddr, Decimal, HumanAddr, StdError, StdResult, Uint128};
use cw20::{Cw20CoinHuman, MinterResponse};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairInitMsg {
    /// Contract owner who can update configs
    pub owner: HumanAddr,
    /// Inactive commission collector
    pub commission_collector: HumanAddr,
    /// Asset infos
    pub asset_infos: [AssetInfo; 2],
    /// Commission rate for active liquidity provider
    pub lp_commission: Decimal,
    /// Commission rate for owner controlled commission
    pub owner_commission: Decimal,
    /// Token contract code id for initialization
    pub token_code_id: u64,
    /// Hook for post initalization
    pub init_hook: Option<InitHook>,
}

/// TokenContract InitMsg
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TokenInitMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Vec<Cw20CoinHuman>,
    pub mint: Option<MinterResponse>,
    pub init_hook: Option<InitHook>,
}

impl TokenInitMsg {
    pub fn get_cap(&self) -> Option<Uint128> {
        self.mint.as_ref().and_then(|v| v.cap)
    }

    pub fn validate(&self) -> StdResult<()> {
        // Check name, symbol, decimals
        if !is_valid_name(&self.name) {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        if !is_valid_symbol(&self.symbol) {
            return Err(StdError::generic_err(
                "Ticker symbol is not in expected format [a-zA-Z\\-]{3,12}",
            ));
        }
        if self.decimals > 18 {
            return Err(StdError::generic_err("Decimals must not exceed 18"));
        }
        Ok(())
    }
}

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 50 {
        return false;
    }
    true
}

fn is_valid_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    if bytes.len() < 3 || bytes.len() > 12 {
        return false;
    }
    for byte in bytes.iter() {
        if (*byte != 45) && (*byte < 65 || *byte > 90) && (*byte < 97 || *byte > 122) {
            return false;
        }
    }
    true
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct PairConfigRaw {
    pub owner: CanonicalAddr,
    pub contract_addr: CanonicalAddr,
    pub liquidity_token: CanonicalAddr,
    pub commission_collector: CanonicalAddr,
}
