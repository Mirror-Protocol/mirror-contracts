pub mod contract;
mod querier;
mod staking;
pub mod state;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock_querier;

#[cfg(target_arch = "wasm32")]
cosmwasm_std::create_entry_points_with_migration!(contract);
