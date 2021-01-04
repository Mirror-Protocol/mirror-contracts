pub mod contract;
pub mod querier;
pub mod state;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock_querier;

#[cfg(target_arch = "wasm32")]
cosmwasm_std::create_entry_points_with_migration!(contract);
