use cosmwasm_std::{
    from_binary, log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, MigrateResponse, MigrateResult, Querier,
    StdError, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};

use crate::{
    asserts::{assert_auction_discount, assert_min_collateral_ratio},
    migration::{migrate_asset_configs, migrate_config},
    positions::{
        auction, burn, deposit, mint, open_position, query_next_position_idx, query_position,
        query_positions, withdraw,
    },
    querier::load_oracle_feeder,
    state::{
        read_asset_config, read_config, store_asset_config, store_config, store_position_idx,
        AssetConfig, Config,
    },
};

use cw20::Cw20ReceiveMsg;
use mirror_protocol::mint::{
    AssetConfigResponse, ConfigResponse, Cw20HookMsg, HandleMsg, InitMsg, MigrateMsg, QueryMsg,
};
use mirror_protocol::oracle::QueryMsg as OracleQueryMsg;
use mirror_protocol::{
    collateral_oracle::{HandleMsg as CollateralOracleHandleMsg, SourceType},
    mint::IPOParams,
};
use terraswap::asset::{Asset, AssetInfo};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let config = Config {
        owner: deps.api.canonical_address(&msg.owner)?,
        oracle: deps.api.canonical_address(&msg.oracle)?,
        collector: deps.api.canonical_address(&msg.collector)?,
        collateral_oracle: deps.api.canonical_address(&msg.collateral_oracle)?,
        staking: deps.api.canonical_address(&msg.staking)?,
        terraswap_factory: deps.api.canonical_address(&msg.terraswap_factory)?,
        lock: deps.api.canonical_address(&msg.lock)?,
        base_denom: msg.base_denom,
        token_code_id: msg.token_code_id,
        protocol_fee_rate: msg.protocol_fee_rate,
    };

    store_config(&mut deps.storage, &config)?;
    store_position_idx(&mut deps.storage, Uint128(1u128))?;
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Receive(msg) => receive_cw20(deps, env, msg),
        HandleMsg::UpdateConfig {
            owner,
            oracle,
            collector,
            collateral_oracle,
            staking,
            terraswap_factory,
            lock,
            token_code_id,
            protocol_fee_rate,
        } => update_config(
            deps,
            env,
            owner,
            oracle,
            collector,
            collateral_oracle,
            staking,
            terraswap_factory,
            lock,
            token_code_id,
            protocol_fee_rate,
        ),
        HandleMsg::UpdateAsset {
            asset_token,
            auction_discount,
            min_collateral_ratio,
            ipo_params,
        } => update_asset(
            deps,
            env,
            asset_token,
            auction_discount,
            min_collateral_ratio,
            ipo_params,
        ),
        HandleMsg::RegisterAsset {
            asset_token,
            auction_discount,
            min_collateral_ratio,
            ipo_params,
        } => register_asset(
            deps,
            env,
            asset_token,
            auction_discount,
            min_collateral_ratio,
            ipo_params,
        ),
        HandleMsg::RegisterMigration {
            asset_token,
            end_price,
        } => register_migration(deps, env, asset_token, end_price),
        HandleMsg::TriggerIPO { asset_token } => trigger_ipo(deps, env, asset_token),
        HandleMsg::OpenPosition {
            collateral,
            asset_info,
            collateral_ratio,
            short_params,
        } => {
            // only native token can be deposited directly
            if !collateral.is_native_token() {
                return Err(StdError::unauthorized());
            }

            // Check the actual deposit happens
            collateral.assert_sent_native_token_balance(&env)?;

            open_position(
                deps,
                env.clone(),
                env.message.sender,
                collateral,
                asset_info,
                collateral_ratio,
                short_params,
            )
        }
        HandleMsg::Deposit {
            position_idx,
            collateral,
        } => {
            // only native token can be deposited directly
            if !collateral.is_native_token() {
                return Err(StdError::unauthorized());
            }

            // Check the actual deposit happens
            collateral.assert_sent_native_token_balance(&env)?;

            deposit(deps, env.message.sender, position_idx, collateral)
        }
        HandleMsg::Withdraw {
            position_idx,
            collateral,
        } => withdraw(deps, env, position_idx, collateral),
        HandleMsg::Mint {
            position_idx,
            asset,
            short_params,
        } => mint(deps, env, position_idx, asset, short_params),
    }
}

pub fn receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> HandleResult {
    let passed_asset: Asset = Asset {
        info: AssetInfo::Token {
            contract_addr: env.message.sender.clone(),
        },
        amount: cw20_msg.amount,
    };

    if let Some(msg) = cw20_msg.msg {
        match from_binary(&msg)? {
            Cw20HookMsg::OpenPosition {
                asset_info,
                collateral_ratio,
                short_params,
            } => open_position(
                deps,
                env,
                cw20_msg.sender,
                passed_asset,
                asset_info,
                collateral_ratio,
                short_params,
            ),
            Cw20HookMsg::Deposit { position_idx } => {
                deposit(deps, cw20_msg.sender, position_idx, passed_asset)
            }
            Cw20HookMsg::Burn { position_idx } => {
                burn(deps, env, cw20_msg.sender, position_idx, passed_asset)
            }
            Cw20HookMsg::Auction { position_idx } => {
                auction(deps, env, cw20_msg.sender, position_idx, passed_asset)
            }
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
    oracle: Option<HumanAddr>,
    collector: Option<HumanAddr>,
    collateral_oracle: Option<HumanAddr>,
    staking: Option<HumanAddr>,
    terraswap_factory: Option<HumanAddr>,
    lock: Option<HumanAddr>,
    token_code_id: Option<u64>,
    protocol_fee_rate: Option<Decimal>,
) -> StdResult<HandleResponse> {
    let mut config: Config = read_config(&deps.storage)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    if let Some(oracle) = oracle {
        config.oracle = deps.api.canonical_address(&oracle)?;
    }

    if let Some(collector) = collector {
        config.collector = deps.api.canonical_address(&collector)?;
    }

    if let Some(collateral_oracle) = collateral_oracle {
        config.collateral_oracle = deps.api.canonical_address(&collateral_oracle)?;
    }

    if let Some(staking) = staking {
        config.staking = deps.api.canonical_address(&staking)?;
    }

    if let Some(terraswap_factory) = terraswap_factory {
        config.terraswap_factory = deps.api.canonical_address(&terraswap_factory)?;
    }

    if let Some(lock) = lock {
        config.lock = deps.api.canonical_address(&lock)?;
    }

    if let Some(token_code_id) = token_code_id {
        config.token_code_id = token_code_id;
    }

    if let Some(protocol_fee_rate) = protocol_fee_rate {
        config.protocol_fee_rate = protocol_fee_rate;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

pub fn update_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    auction_discount: Option<Decimal>,
    min_collateral_ratio: Option<Decimal>,
    ipo_params: Option<IPOParams>,
) -> StdResult<HandleResponse> {
    let config: Config = read_config(&deps.storage)?;
    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let mut asset: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(auction_discount) = auction_discount {
        assert_auction_discount(auction_discount)?;
        asset.auction_discount = auction_discount;
    }

    if let Some(min_collateral_ratio) = min_collateral_ratio {
        assert_min_collateral_ratio(min_collateral_ratio)?;
        asset.min_collateral_ratio = min_collateral_ratio;
    }

    if let Some(ipo_params) = ipo_params {
        assert_min_collateral_ratio(ipo_params.min_collateral_ratio_after_ipo)?;
        asset.ipo_params = Some(ipo_params);
    }

    store_asset_config(&mut deps.storage, &asset_token_raw, &asset)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_asset")],
        data: None,
    })
}

pub fn register_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    auction_discount: Decimal,
    min_collateral_ratio: Decimal,
    ipo_params: Option<IPOParams>,
) -> StdResult<HandleResponse> {
    assert_auction_discount(auction_discount)?;
    assert_min_collateral_ratio(min_collateral_ratio)?;

    let config: Config = read_config(&deps.storage)?;

    // permission check
    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    if read_asset_config(&deps.storage, &asset_token_raw).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    let mut messages: Vec<CosmosMsg> = vec![];

    // check if it is a preIPO asset
    if let Some(params) = ipo_params.clone() {
        assert_min_collateral_ratio(params.min_collateral_ratio_after_ipo)?;
    } else {
        // only non-preIPO assets can be used as collateral
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.collateral_oracle)?,
            send: vec![],
            msg: to_binary(&CollateralOracleHandleMsg::RegisterCollateralAsset {
                asset: AssetInfo::Token {
                    contract_addr: asset_token.clone(),
                },
                multiplier: Decimal::one(), // default collateral multiplier for new mAssets
                price_source: SourceType::TerraOracle {
                    terra_oracle_query: to_binary(&WasmQuery::Smart {
                        contract_addr: deps.api.human_address(&config.oracle)?,
                        msg: to_binary(&OracleQueryMsg::Price {
                            base_asset: config.base_denom,
                            quote_asset: asset_token.to_string(),
                        })?,
                    })?,
                },
            })?,
        }));
    }

    // Store temp info into base asset store
    store_asset_config(
        &mut deps.storage,
        &asset_token_raw,
        &AssetConfig {
            token: deps.api.canonical_address(&asset_token)?,
            auction_discount,
            min_collateral_ratio,
            end_price: None,
            ipo_params,
        },
    )?;

    Ok(HandleResponse {
        messages,
        log: vec![log("action", "register"), log("asset_token", asset_token)],
        data: None,
    })
}

pub fn register_migration<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
    end_price: Decimal,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    if config.owner != deps.api.canonical_address(&env.message.sender)? {
        return Err(StdError::unauthorized());
    }

    let asset_token_raw = deps.api.canonical_address(&asset_token)?;
    let asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;

    // update asset config
    store_asset_config(
        &mut deps.storage,
        &asset_token_raw,
        &AssetConfig {
            end_price: Some(end_price),
            min_collateral_ratio: Decimal::percent(100),
            ipo_params: None,
            ..asset_config
        },
    )?;

    // flag asset as revoked in the collateral oracle
    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.collateral_oracle)?,
            send: vec![],
            msg: to_binary(&CollateralOracleHandleMsg::RevokeCollateralAsset {
                asset: AssetInfo::Token {
                    contract_addr: asset_token.clone(),
                },
            })?,
        })],
        log: vec![
            log("action", "migrate_asset"),
            log("asset_token", asset_token.as_str()),
            log("end_price", end_price.to_string()),
        ],
        data: None,
    })
}

pub fn trigger_ipo<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_token: HumanAddr,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    let asset_token_raw: CanonicalAddr = deps.api.canonical_address(&&asset_token)?;
    let oracle_feeder: HumanAddr = deps.api.human_address(&load_oracle_feeder(
        &deps,
        &deps.api.human_address(&config.oracle)?,
        &asset_token_raw,
    )?)?;

    // only asset feeder can trigger ipo
    if oracle_feeder != env.message.sender {
        return Err(StdError::unauthorized());
    }

    let mut asset_config: AssetConfig = read_asset_config(&deps.storage, &asset_token_raw)?;

    let ipo_params: IPOParams = match asset_config.ipo_params {
        Some(v) => v,
        None => return Err(StdError::generic_err("Asset does not have IPO params")),
    };

    asset_config.min_collateral_ratio = ipo_params.min_collateral_ratio_after_ipo;
    asset_config.ipo_params = None;

    store_asset_config(&mut deps.storage, &asset_token_raw, &asset_config)?;

    // register asset in collateral oracle
    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.collateral_oracle)?,
            send: vec![],
            msg: to_binary(&CollateralOracleHandleMsg::RegisterCollateralAsset {
                asset: AssetInfo::Token {
                    contract_addr: asset_token.clone(),
                },
                multiplier: Decimal::one(), // default collateral multiplier for new mAssets
                price_source: SourceType::TerraOracle {
                    terra_oracle_query: to_binary(&WasmQuery::Smart {
                        contract_addr: deps.api.human_address(&config.oracle)?,
                        msg: to_binary(&OracleQueryMsg::Price {
                            base_asset: config.base_denom,
                            quote_asset: asset_token.to_string(),
                        })?,
                    })?,
                },
            })?,
        })],
        log: vec![
            log("action", "trigger_ipo"),
            log("asset_token", asset_token.as_str()),
        ],
        data: None,
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::AssetConfig { asset_token } => to_binary(&query_asset_config(deps, asset_token)?),
        QueryMsg::Position { position_idx } => to_binary(&query_position(deps, position_idx)?),
        QueryMsg::Positions {
            owner_addr,
            asset_token,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_positions(
            deps,
            owner_addr,
            asset_token,
            start_after,
            limit,
            order_by,
        )?),
        QueryMsg::NextPositionIdx {} => to_binary(&query_next_position_idx(deps)?),
    }
}

pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        oracle: deps.api.human_address(&state.oracle)?,
        staking: deps.api.human_address(&state.staking)?,
        collector: deps.api.human_address(&state.collector)?,
        collateral_oracle: deps.api.human_address(&state.collateral_oracle)?,
        terraswap_factory: deps.api.human_address(&state.terraswap_factory)?,
        lock: deps.api.human_address(&state.lock)?,
        base_denom: state.base_denom,
        token_code_id: state.token_code_id,
        protocol_fee_rate: Decimal::percent(1),
    };

    Ok(resp)
}

pub fn query_asset_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    asset_token: HumanAddr,
) -> StdResult<AssetConfigResponse> {
    let asset_config: AssetConfig =
        read_asset_config(&deps.storage, &deps.api.canonical_address(&asset_token)?)?;

    let resp = AssetConfigResponse {
        token: deps.api.human_address(&asset_config.token).unwrap(),
        auction_discount: asset_config.auction_discount,
        min_collateral_ratio: asset_config.min_collateral_ratio,
        end_price: asset_config.end_price,
        ipo_params: asset_config.ipo_params,
    };

    Ok(resp)
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: MigrateMsg,
) -> MigrateResult {
    // migrate config
    migrate_config(
        &mut deps.storage,
        deps.api.canonical_address(&msg.staking)?,
        deps.api.canonical_address(&msg.terraswap_factory)?,
        deps.api.canonical_address(&msg.collateral_oracle)?,
        deps.api.canonical_address(&msg.lock)?,
    )?;

    // migrate all asset configurations to use new add mint_end parameter
    migrate_asset_configs(&mut deps.storage)?;

    Ok(MigrateResponse::default())
}
