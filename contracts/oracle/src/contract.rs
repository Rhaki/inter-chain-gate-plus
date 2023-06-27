use std::cmp::min;

#[cfg(not(feature = "library"))]
use cosmwasm_std::{entry_point, DepsMut, Env, MessageInfo, Response};
use cosmwasm_std::{
    from_binary, to_binary, Addr, Attribute, Binary, Coin, CosmosMsg, Decimal, Deps, Order,
    QueryRequest, StdError, StdResult, Storage, WasmMsg, WasmQuery,
};
use cw_storage_plus::Bound;

use crate::{
    errors::ContractError,
    msgs::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{Asset, ASSETS, CHAINS_CONTRACT, GATE, OWNER},
};

use gate_pkg::{ExecuteMsg as GateExecuteMsg, GateMsg, GateQueryResponse, GateRequest, Permission};

// settings for pagination
const MAX_LIMIT: u64 = 30;
const DEFAULT_LIMIT: u64 = 10;

// --- ENTRY POINTS ---

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    OWNER.save(deps.storage, &info.sender)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RegisterAsset { asset, feeder } => {
            run_regiser_asset(deps, info.sender, asset, feeder)
        }
        ExecuteMsg::FeedPrice { asset, price } => run_feed_price(deps, info.sender, asset, price),
        ExecuteMsg::FeedRemotePrice { asset, chain } => {
            run_feed_remote_price(deps, info.sender, info.funds, asset, chain)
        }
        ExecuteMsg::RegisterGate { contract } => run_register_gate(deps, info.sender, contract),
        ExecuteMsg::GateSetPermission { contract, chain } => {
            run_gate_set_permission(deps, info.sender, contract, chain)
        }
        ExecuteMsg::ReceiveGateMsg(msg) => gate_receive_msg(deps, info, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Price { asset } => to_binary(&qy_price(deps, asset).unwrap()),
        QueryMsg::Prices { start_after, limit } => {
            to_binary(&qy_prices(deps, start_after, limit).unwrap())
        }
    }
}

// --- RUN ---

fn run_regiser_asset(
    deps: DepsMut,
    sender: Addr,
    asset: String,
    feeder: Addr,
) -> Result<Response, ContractError> {
    onlyowner(deps.storage, &sender)?;

    match ASSETS.load(deps.storage, asset.clone()) {
        Ok(_) => return Err(ContractError::AssetAlredyRegistered { asset }),
        Err(_) => ASSETS.save(
            deps.storage,
            asset.clone(),
            &Asset {
                feeder: feeder.clone(),
                price: None,
            },
        )?,
    };

    Ok(Response::new()
        .add_attribute("action", "register_asset")
        .add_attribute("asset", asset)
        .add_attribute("feeder", feeder))
}

fn run_feed_price(
    deps: DepsMut,
    sender: Addr,
    asset: String,
    price: Decimal,
) -> Result<Response, ContractError> {
    onlyfeeder(deps.storage, &sender, &asset)?;

    ASSETS.update(
        deps.storage,
        asset.clone(),
        |asset| -> Result<Asset, StdError> {
            let mut asset = asset.unwrap();

            asset.price = Some(price);

            Ok(asset)
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "feed_price")
        .add_attribute("asset", asset)
        .add_attribute("price", price.to_string()))
}

fn run_feed_remote_price(
    deps: DepsMut,
    sender: Addr,
    funds: Vec<Coin>,
    asset: String,
    chain: String,
) -> Result<Response, ContractError> {
    onlyfeeder(deps.storage, &sender, &asset)?;

    let remote_contract = CHAINS_CONTRACT.load(deps.storage, chain.clone())?;

    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: GATE.load(deps.storage)?.to_string(),

        msg: to_binary(&GateExecuteMsg::SendRequests {
            requests: vec![GateRequest::Query {
                queries: vec![QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: remote_contract,
                    msg: to_binary(&QueryMsg::Price {
                        asset: asset.clone(),
                    })?,
                })],
                callback_msg: None,
            }],
            chain: chain.clone(),
            timeout: None,
        })?,

        funds,
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "feed_remote_price")
        .add_attribute("chain", chain)
        .add_attribute("asset", asset))
}

fn run_register_gate(
    deps: DepsMut,
    sender: Addr,
    contract: Addr,
) -> Result<Response, ContractError> {
    onlyowner(deps.storage, &sender)?;

    GATE.save(deps.storage, &contract)?;

    Ok(Response::new()
        .add_attribute("action", "register_gate")
        .add_attribute("contract", contract))
}

fn run_gate_set_permission(
    deps: DepsMut,
    sender: Addr,
    contract: String,
    chain: String,
) -> Result<Response, ContractError> {
    onlyowner(deps.storage, &sender)?;

    CHAINS_CONTRACT.save(deps.storage, chain.clone(), &contract)?;

    let msg = CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
        contract_addr: GATE.load(deps.storage)?.to_string(),
        msg: to_binary(&GateExecuteMsg::SetPermission {
            permission: Permission::Permissioned {
                addresses: vec![contract.clone()],
            },
            chain,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "register_remote_contract")
        .add_attribute("value", contract))
}

// --- GATE ---

fn gate_receive_msg(
    deps: DepsMut,
    info: MessageInfo,
    msg: GateMsg,
) -> Result<Response, ContractError> {
    match msg {
        GateMsg::QueryResponse {
            queries,
            callback_msg,
        } => run_gate_query_response(deps, info.sender, queries, callback_msg),

        _ => Err(ContractError::Std(StdError::generic_err(format!(
            "{:?} not implemented on mock_deposit",
            msg
        )))),
    }
}

fn run_gate_query_response(
    deps: DepsMut,
    sender: Addr,
    queries: Vec<GateQueryResponse>,
    _callback_msg: Option<Binary>,
) -> Result<Response, ContractError> {
    onlygate(deps.storage, &sender)?;

    let mut attributes: Vec<Attribute> = vec![Attribute::new("action", "gate_query_response")];

    for query in queries {
        match query.request {
            QueryRequest::Wasm(WasmQuery::Smart { msg, .. }) => match from_binary(&msg)? {
                QueryMsg::Price { asset } => {
                    let price: Decimal = from_binary(&query.response)?;

                    ASSETS.update(
                        deps.storage,
                        asset.clone(),
                        |asset| -> Result<Asset, StdError> {
                            let mut asset = asset.unwrap();

                            asset.price = Some(price);

                            Ok(asset)
                        },
                    )?;

                    attributes.push(Attribute::new("asset", asset));
                    attributes.push(Attribute::new("price", price.to_string()))
                }
                _ => return Err(ContractError::Std(StdError::generic_err("Unimplemented"))),
            },

            _ => return Err(ContractError::Std(StdError::generic_err("Unimplemented"))),
        }
    }

    Ok(Response::new().add_attributes(attributes))
}

// --- QUERIES ---

fn qy_price(deps: Deps, asset: String) -> StdResult<Decimal> {
    let price = ASSETS.load(deps.storage, asset)?.price;

    match price {
        Some(price) => Ok(price),
        None => Err(StdError::generic_err("Price never feeded")),
    }
}

fn qy_prices(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u64>,
) -> StdResult<Vec<(String, Option<Decimal>)>> {
    let limit = match limit {
        Some(value) => min(value, MAX_LIMIT),
        None => DEFAULT_LIMIT,
    };

    let start: Option<Bound<String>> = start_after.map(Bound::exclusive);

    let prices: Vec<(String, Option<Decimal>)> = ASSETS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit.try_into().unwrap())
        .map(|item| {
            let (asset, info) = item.unwrap();
            (asset, info.price)
        })
        .collect();

    Ok(prices)
}

// --- FUNCTIONS ---

fn onlyowner(storage: &dyn Storage, address: &Addr) -> Result<(), ContractError> {
    if OWNER.load(storage)? != *address {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

fn onlyfeeder(storage: &dyn Storage, feeder: &Addr, asset: &String) -> Result<(), ContractError> {
    if ASSETS.load(storage, asset.to_owned())?.feeder != *feeder {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

fn onlygate(storage: &dyn Storage, address: &Addr) -> Result<(), ContractError> {
    if GATE.load(storage)? != *address {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}
