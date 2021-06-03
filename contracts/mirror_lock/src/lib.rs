pub mod contract;
mod state;

#[cfg(test)]
mod mock_querier;
#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
cosmwasm_std::create_entry_points!(contract);
