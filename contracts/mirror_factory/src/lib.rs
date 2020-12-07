pub mod contract;
pub mod math;
pub mod msg;
pub mod register_msgs;
pub mod state;
pub mod querier;

#[cfg(test)]
mod testing;

#[cfg(test)]
mod mock_querier;

#[cfg(target_arch = "wasm32")]
cosmwasm_std::create_entry_points_with_migration!(contract);
