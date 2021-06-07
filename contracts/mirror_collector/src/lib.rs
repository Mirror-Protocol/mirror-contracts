pub mod contract;
mod migration;
pub mod state;
mod swap;

#[cfg(test)]
mod testing;

#[cfg(test)]
mod mock_querier;

#[cfg(target_arch = "wasm32")]
cosmwasm_std::create_entry_points_with_migration!(contract);
