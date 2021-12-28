use crate::migration::migrate_pool_infos;
use crate::rewards::{adjust_premium, deposit_reward, query_reward_info, withdraw_reward};
use crate::staking::{
    auto_stake, auto_stake_hook, bond, decrease_short_token, increase_short_token, unbond,
};
use crate::state::{
    read_config, read_pool_info, store_config, store_pool_info, Config, MigrationParams, PoolInfo,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128,
};
use mirror_protocol::staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PoolInfoResponse, QueryMsg,
};

use cw20::Cw20ReceiveMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    store_config(
        deps.storage,
        &Config {
            owner: deps.api.addr_canonicalize(&msg.owner)?,
            mirror_token: deps.api.addr_canonicalize(&msg.mirror_token)?,
            mint_contract: deps.api.addr_canonicalize(&msg.mint_contract)?,
            oracle_contract: deps.api.addr_canonicalize(&msg.oracle_contract)?,
            terraswap_factory: deps.api.addr_canonicalize(&msg.terraswap_factory)?,
            base_denom: msg.base_denom,
            premium_min_update_interval: msg.premium_min_update_interval,
            short_reward_contract: deps.api.addr_canonicalize(&msg.short_reward_contract)?,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::UpdateConfig {
            owner,
            premium_min_update_interval,
            short_reward_contract,
        } => {
            let owner_addr = if let Some(owner_addr) = owner {
                Some(deps.api.addr_validate(&owner_addr)?)
            } else {
                None
            };
            let short_reward_contract_addr =
                if let Some(short_reward_contract) = short_reward_contract {
                    Some(deps.api.addr_validate(&short_reward_contract)?)
                } else {
                    None
                };
            update_config(
                deps,
                info,
                owner_addr,
                premium_min_update_interval,
                short_reward_contract_addr,
            )
        }
        ExecuteMsg::RegisterAsset {
            asset_token,
            staking_token,
        } => {
            let api = deps.api;
            register_asset(
                deps,
                info,
                api.addr_validate(&asset_token)?,
                api.addr_validate(&staking_token)?,
            )
        }
        ExecuteMsg::DeprecateStakingToken {
            asset_token,
            new_staking_token,
        } => {
            let api = deps.api;
            deprecate_staking_token(
                deps,
                info,
                api.addr_validate(&asset_token)?,
                api.addr_validate(&new_staking_token)?,
            )
        }
        ExecuteMsg::Unbond {
            asset_token,
            amount,
        } => {
            let api = deps.api;
            unbond(deps, info.sender, api.addr_validate(&asset_token)?, amount)
        }
        ExecuteMsg::Withdraw { asset_token } => {
            let asset_addr = if let Some(asset_addr) = asset_token {
                Some(deps.api.addr_validate(&asset_addr)?)
            } else {
                None
            };
            withdraw_reward(deps, info, asset_addr)
        }
        ExecuteMsg::AdjustPremium { asset_tokens } => adjust_premium(deps, env, asset_tokens),
        ExecuteMsg::IncreaseShortToken {
            staker_addr,
            asset_token,
            amount,
        } => {
            let api = deps.api;
            increase_short_token(
                deps,
                info,
                api.addr_validate(&staker_addr)?,
                api.addr_validate(&asset_token)?,
                amount,
            )
        }
        ExecuteMsg::DecreaseShortToken {
            staker_addr,
            asset_token,
            amount,
        } => {
            let api = deps.api;
            decrease_short_token(
                deps,
                info,
                api.addr_validate(&staker_addr)?,
                api.addr_validate(&asset_token)?,
                amount,
            )
        }
        ExecuteMsg::AutoStake {
            assets,
            slippage_tolerance,
        } => auto_stake(deps, env, info, assets, slippage_tolerance),
        ExecuteMsg::AutoStakeHook {
            asset_token,
            staking_token,
            staker_addr,
            prev_staking_token_amount,
        } => {
            let api = deps.api;
            auto_stake_hook(
                deps,
                env,
                info,
                api.addr_validate(&asset_token)?,
                api.addr_validate(&staking_token)?,
                api.addr_validate(&staker_addr)?,
                prev_staking_token_amount,
            )
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond { asset_token }) => {
            let pool_info: PoolInfo =
                read_pool_info(deps.storage, &deps.api.addr_canonicalize(&asset_token)?)?;

            // only staking token contract can execute this message
            let token_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
            if pool_info.staking_token != token_raw {
                // if user is trying to bond old token, return friendly error message
                if let Some(params) = pool_info.migration_params {
                    if params.deprecated_staking_token == token_raw {
                        let staking_token_addr =
                            deps.api.addr_humanize(&pool_info.staking_token)?;
                        return Err(StdError::generic_err(format!(
                            "The staking token for this asset has been migrated to {}",
                            staking_token_addr.to_string()
                        )));
                    }
                }

                return Err(StdError::generic_err("unauthorized"));
            }

            let api = deps.api;
            bond(
                deps,
                api.addr_validate(cw20_msg.sender.as_str())?,
                api.addr_validate(asset_token.as_str())?,
                cw20_msg.amount,
            )
        }
        Ok(Cw20HookMsg::DepositReward { rewards }) => {
            let config: Config = read_config(deps.storage)?;

            // only reward token contract can execute this message
            if config.mirror_token != deps.api.addr_canonicalize(info.sender.as_str())? {
                return Err(StdError::generic_err("unauthorized"));
            }

            let mut rewards_amount = Uint128::zero();
            for (_, amount) in rewards.iter() {
                rewards_amount += *amount;
            }

            if rewards_amount != cw20_msg.amount {
                return Err(StdError::generic_err("rewards amount miss matched"));
            }

            deposit_reward(deps, rewards, rewards_amount)
        }
        Err(_) => Err(StdError::generic_err("invalid cw20 hook message")),
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<Addr>,
    premium_min_update_interval: Option<u64>,
    short_reward_contract: Option<Addr>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;

    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(owner.as_str())?;
    }

    if let Some(premium_min_update_interval) = premium_min_update_interval {
        config.premium_min_update_interval = premium_min_update_interval;
    }

    if let Some(short_reward_contract) = short_reward_contract {
        config.short_reward_contract =
            deps.api.addr_canonicalize(short_reward_contract.as_str())?;
    }

    store_config(deps.storage, &config)?;
    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

fn register_asset(
    deps: DepsMut,
    info: MessageInfo,
    asset_token: Addr,
    staking_token: Addr,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let asset_token_raw = deps.api.addr_canonicalize(asset_token.as_str())?;

    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    if read_pool_info(deps.storage, &asset_token_raw).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    store_pool_info(
        deps.storage,
        &asset_token_raw,
        &PoolInfo {
            staking_token: deps.api.addr_canonicalize(staking_token.as_str())?,
            total_bond_amount: Uint128::zero(),
            total_short_amount: Uint128::zero(),
            reward_index: Decimal::zero(),
            short_reward_index: Decimal::zero(),
            pending_reward: Uint128::zero(),
            short_pending_reward: Uint128::zero(),
            premium_rate: Decimal::zero(),
            short_reward_weight: Decimal::zero(),
            premium_updated_time: 0,
            migration_params: None,
        },
    )?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "register_asset"),
        attr("asset_token", asset_token.as_str()),
    ]))
}

fn deprecate_staking_token(
    deps: DepsMut,
    info: MessageInfo,
    asset_token: Addr,
    new_staking_token: Addr,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let asset_token_raw = deps.api.addr_canonicalize(asset_token.as_str())?;

    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    let mut pool_info: PoolInfo = read_pool_info(deps.storage, &asset_token_raw)?;

    if pool_info.migration_params.is_some() {
        return Err(StdError::generic_err(
            "This asset LP token has already been migrated",
        ));
    }

    let deprecated_token_addr: Addr = deps.api.addr_humanize(&pool_info.staking_token)?;

    pool_info.total_bond_amount = Uint128::zero();
    pool_info.migration_params = Some(MigrationParams {
        index_snapshot: pool_info.reward_index,
        deprecated_staking_token: pool_info.staking_token,
    });
    pool_info.staking_token = deps.api.addr_canonicalize(new_staking_token.as_str())?;

    store_pool_info(deps.storage, &asset_token_raw, &pool_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "depcrecate_staking_token"),
        attr("asset_token", asset_token.to_string()),
        attr(
            "deprecated_staking_token",
            deprecated_token_addr.to_string(),
        ),
        attr("new_staking_token", new_staking_token.to_string()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::PoolInfo { asset_token } => to_binary(&query_pool_info(deps, asset_token)?),
        QueryMsg::RewardInfo {
            staker_addr,
            asset_token,
        } => to_binary(&query_reward_info(deps, staker_addr, asset_token)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.addr_humanize(&state.owner)?.to_string(),
        mirror_token: deps.api.addr_humanize(&state.mirror_token)?.to_string(),
        mint_contract: deps.api.addr_humanize(&state.mint_contract)?.to_string(),
        oracle_contract: deps.api.addr_humanize(&state.oracle_contract)?.to_string(),
        terraswap_factory: deps
            .api
            .addr_humanize(&state.terraswap_factory)?
            .to_string(),
        base_denom: state.base_denom,
        premium_min_update_interval: state.premium_min_update_interval,
        short_reward_contract: deps
            .api
            .addr_humanize(&state.short_reward_contract)?
            .to_string(),
    };

    Ok(resp)
}

pub fn query_pool_info(deps: Deps, asset_token: String) -> StdResult<PoolInfoResponse> {
    let asset_token_raw = deps.api.addr_canonicalize(&asset_token)?;
    let pool_info: PoolInfo = read_pool_info(deps.storage, &asset_token_raw)?;
    Ok(PoolInfoResponse {
        asset_token,
        staking_token: deps
            .api
            .addr_humanize(&pool_info.staking_token)?
            .to_string(),
        total_bond_amount: pool_info.total_bond_amount,
        total_short_amount: pool_info.total_short_amount,
        reward_index: pool_info.reward_index,
        short_reward_index: pool_info.short_reward_index,
        pending_reward: pool_info.pending_reward,
        short_pending_reward: pool_info.short_pending_reward,
        premium_rate: pool_info.premium_rate,
        short_reward_weight: pool_info.short_reward_weight,
        premium_updated_time: pool_info.premium_updated_time,
        migration_deprecated_staking_token: pool_info.migration_params.clone().map(|params| {
            deps.api
                .addr_humanize(&params.deprecated_staking_token)
                .unwrap()
                .to_string()
        }),
        migration_index_snapshot: pool_info
            .migration_params
            .map(|params| params.index_snapshot),
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> StdResult<Response> {
    migrate_pool_infos(deps.storage)?;

    // when the migration is executed, deprecate directly the MIR pool
    let config = read_config(deps.storage)?;
    let self_info = MessageInfo {
        sender: deps.api.addr_humanize(&config.owner)?,
        funds: vec![],
    };
    let asset_token_to_deprecate_addr = deps.api.addr_validate(&msg.asset_token_to_deprecate)?;
    let new_staking_token_addr = deps.api.addr_validate(&msg.new_staking_token)?;
    deprecate_staking_token(
        deps,
        self_info,
        asset_token_to_deprecate_addr,
        new_staking_token_addr,
    )?;

    Ok(Response::default())
}
