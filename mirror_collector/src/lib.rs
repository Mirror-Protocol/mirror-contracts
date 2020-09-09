pub mod contract;
pub mod msg;
pub mod state;
pub mod querier;

#[cfg(test)]
mod testing;

#[cfg(test)]
mod mock_querier;

#[cfg(target_arch = "wasm32")]
cosmwasm_std::create_entry_points!(contract);
