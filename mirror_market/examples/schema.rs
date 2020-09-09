use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};
use cosmwasm_std::HandleResponse;

use mirror_market::msg::{
    ConfigAssetResponse, ConfigGeneralResponse, ConfigSwapResponse, HandleMsg, InitMsg,
    PoolResponse, QueryMsg, ReverseSimulationResponse, SimulationResponse,
};
use mirror_market::state::{ConfigAsset, ConfigGeneral, ConfigSwap};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InitMsg), &out_dir);
    export_schema(&schema_for!(HandleMsg), &out_dir);
    export_schema(&schema_for!(HandleResponse), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(ConfigGeneral), &out_dir);
    export_schema(&schema_for!(ConfigAsset), &out_dir);
    export_schema(&schema_for!(ConfigSwap), &out_dir);
    export_schema(&schema_for!(ConfigGeneralResponse), &out_dir);
    export_schema(&schema_for!(ConfigAssetResponse), &out_dir);
    export_schema(&schema_for!(ConfigSwapResponse), &out_dir);
    export_schema(&schema_for!(PoolResponse), &out_dir);
    export_schema(&schema_for!(ReverseSimulationResponse), &out_dir);
    export_schema(&schema_for!(SimulationResponse), &out_dir);
}
