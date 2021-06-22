use cosmwasm_schema::{export_schema, remove_schemas, schema_for};
use std::env::current_dir;
use std::fs::create_dir_all;

use mirror_protocol::gov::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, HandleMsg, InitMsg, MigrateMsg, PollCountResponse,
    PollResponse, PollsResponse, QueryMsg, SharesResponse, StakerResponse,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InitMsg), &out_dir);
    export_schema(&schema_for!(HandleMsg), &out_dir);
    export_schema(&schema_for!(Cw20HookMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(StakerResponse), &out_dir);
    export_schema(&schema_for!(ConfigResponse), &out_dir);
    export_schema(&schema_for!(PollResponse), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(PollsResponse), &out_dir);
    export_schema(&schema_for!(PollCountResponse), &out_dir);
    export_schema(&schema_for!(SharesResponse), &out_dir);
    export_schema(&schema_for!(MigrateMsg), &out_dir);
}
