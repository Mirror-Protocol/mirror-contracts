use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use mirror_protocol::mint::{
    AssetConfigResponse, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg,
    NextPositionIdxResponse, PositionResponse, PositionsResponse, QueryMsg, ShortParams,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(ShortParams), &out_dir);
    export_schema(&schema_for!(Cw20HookMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(ConfigResponse), &out_dir);
    export_schema(&schema_for!(NextPositionIdxResponse), &out_dir);
    export_schema(&schema_for!(AssetConfigResponse), &out_dir);
    export_schema(&schema_for!(PositionResponse), &out_dir);
    export_schema(&schema_for!(PositionsResponse), &out_dir);
}
