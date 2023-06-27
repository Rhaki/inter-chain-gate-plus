use std::{collections::HashMap, str::FromStr};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    QueryRequest, Response, StdError, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::{
    errors::ContractError,
    msgs::{
        is_native, BridgeMsgInfo, Cw20MsgType, ExecuteMsg, GateCollectMsgsAllowed, InstantiateMsg,
        MigrateMsg, Position, QueryMsg,
    },
    state::{CHAINS_CONTRACT, GATE, OWNER, POSITIONS},
};

use cw20_icg_pkg::ExecuteMsg as Cw20_icg_ExecuteMsg;
use gate_pkg::{ExecuteMsg as GateExecuteMsg, GateMsg, GateRequest, Permission, SendNativeInfo};

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
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => run_receive_cw20(deps, env, info.sender, msg),
        ExecuteMsg::RegisterGate { contract } => run_register_gate(deps, info.sender, contract),
        ExecuteMsg::GateSetPermission { contract, chain } => {
            run_gate_set_permission(deps, info.sender, contract, chain)
        }
        ExecuteMsg::Withdraw { denom, amount } => run_withdraw(deps, info.sender, denom, amount),
        ExecuteMsg::IncreaseLoan { amount } => run_increase_loan(deps, info.sender, amount),
        ExecuteMsg::RepayLoan { amount } => run_repay_loan(deps, info.sender, amount),
        // --- GATE MSGS ---
        ExecuteMsg::ReceiveGateMsg(msg) => gate_receive_msg(deps, info, msg),
        ExecuteMsg::Deposit {} => {
            let coin = onecoin(info.funds)?.unwrap();
            user_deposit(deps.storage, &info.sender, coin.amount, coin.denom)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Position { user } => to_binary(&qy_position(deps, user).unwrap()),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new())
}

// --- RUN ---

fn gate_receive_msg(
    deps: DepsMut,
    info: MessageInfo,
    msg: GateMsg,
) -> Result<Response, ContractError> {
    match msg {
        GateMsg::RequestFailed { request } => run_gate_revert_request(deps, info.sender, request),
        GateMsg::ReceivedMsg { sender, msg } => {
            run_gate_receive_msg(deps, info.sender, sender, msg)
        }
        GateMsg::CollectRequests { sender, msg } => {
            run_gate_collect_msgs(deps, info.funds, info.sender, sender, msg)
        }
        _ => Err(ContractError::Std(StdError::generic_err(format!(
            "{:?} not implemented on mock_market",
            msg
        )))),
    }
}

fn run_receive_cw20(
    deps: DepsMut,
    _env: Env,
    cw20_address: Addr,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_binary::<Cw20MsgType>(&cw20_msg.msg)? {
        Cw20MsgType::Deposit {} => user_deposit(
            deps.storage,
            &deps.api.addr_validate(cw20_msg.sender.as_str())?,
            cw20_msg.amount,
            cw20_address.to_string(),
        ),
    }
}

fn run_register_gate(
    deps: DepsMut,
    sender: Addr,
    contract: Addr,
) -> Result<Response, ContractError> {
    onlyowner(deps.storage, &sender)?;

    GATE.save(deps.storage, &contract)?;

    Ok(Response::new())
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

fn run_gate_collect_msgs(
    deps: DepsMut,
    funds: Vec<Coin>,
    gate: Addr,
    sender: Addr,
    msg: Binary,
) -> Result<Response, ContractError> {
    onlygate(deps.storage, &gate)?;

    let mut send_native: Option<SendNativeInfo> = None;

    match from_binary(&msg)? {
        GateCollectMsgsAllowed::BridgePosition {
            to_remote_addr,
            chain,
            native_info,
        } => {
            let coin = onecoin(funds)?;

            let position = POSITIONS.load(deps.storage, sender.clone())?;

            position.is_bridgable(&deps.as_ref())?;

            // I want to equally divide the coin to send to gate contract for fee equally divide it for every collateral
            let mut fund_per_collateral: Vec<Coin> = vec![];
            let mut fund_bridge_position: Vec<Coin> = vec![];

            if let Some(coin) = coin {
                let mut qta_collaterals =
                    Uint128::from_str(position.collaterals.len().to_string().as_str())?;

                if native_info.is_some() {
                    qta_collaterals -= Uint128::one();
                };

                let amount_per_collateral = coin
                    .amount
                    .checked_div(qta_collaterals + Uint128::one())
                    .unwrap();
                fund_per_collateral.push(Coin {
                    denom: coin.denom.clone(),
                    amount: amount_per_collateral,
                });
                fund_bridge_position.push(Coin {
                    denom: coin.denom,
                    amount: coin
                        .amount
                        .checked_sub(amount_per_collateral * qta_collaterals)
                        .unwrap(),
                })
            }

            let mut msgs: Vec<CosmosMsg> = vec![];

            let mut remote_position = Position {
                loan: position.loan,
                collaterals: HashMap::new(),
            };

            for (denom, amount) in position.clone().collaterals {
                if is_native(&deps.as_ref(), &denom) {
                    match fund_bridge_position.first_mut() {
                        Some(coin) => {
                            if coin.denom == denom {
                                coin.amount += amount
                            } else {
                                fund_bridge_position.push(Coin {
                                    denom: denom.clone(),
                                    amount,
                                })
                            }
                        }
                        None => fund_bridge_position.push(Coin {
                            denom: denom.clone(),
                            amount,
                        }),
                    }

                    let native_info = native_info.clone().unwrap();

                    remote_position
                        .collaterals
                        .insert(native_info.dest_denom.clone(), amount);

                    send_native = Some(SendNativeInfo {
                        coin: Coin { denom, amount },
                        path_middle_forward: native_info.path_middle_forward,
                        dest_denom: native_info.dest_denom,
                        channel_id: native_info.channel_id,
                        timeout: native_info.timeout,
                    });
                } else {
                    let remote_contract_addr: String =
                        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                            contract_addr: denom.clone(),
                            msg: to_binary(&cw20_icg_pkg::QueryMsg::RemoteContract {
                                chain: chain.clone(),
                            })?,
                        }))?;

                    remote_position
                        .collaterals
                        .insert(remote_contract_addr, amount);

                    msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: denom,
                        msg: to_binary(&Cw20_icg_ExecuteMsg::GateBridge {
                            chain: chain.clone(),
                            remote_receiver: CHAINS_CONTRACT.load(deps.storage, chain.clone())?,
                            amount,
                        })?,
                        funds: fund_per_collateral.clone(),
                    }))
                }
            }

            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: GATE.load(deps.storage)?.to_string(),
                msg: to_binary(&GateExecuteMsg::SendRequests {
                    requests: vec![GateRequest::SendMsg {
                        msg: to_binary(&BridgeMsgInfo {
                            sender: sender.to_string(),
                            receiver: to_remote_addr,
                            src_position: position,
                            dest_position: remote_position,
                        })?,
                        to_contract: CHAINS_CONTRACT.load(deps.storage, chain.clone())?,
                        send_native,
                    }],
                    chain,
                    timeout: None,
                })?,
                funds: fund_bridge_position,
            }));

            POSITIONS.remove(deps.storage, sender);

            Ok(Response::new().add_messages(msgs))
        }
    }
}

fn run_withdraw(
    deps: DepsMut,
    user: Addr,
    denom: String,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let withdraw_amount = user_withdraw(deps.storage, &user, amount, denom.clone())?;

    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: denom.clone(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: user.to_string(),
            amount: withdraw_amount,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("withdraw_denom", denom)
        .add_attribute("withdraw_amount", withdraw_amount))
}

fn run_increase_loan(
    deps: DepsMut,
    user: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    user_increase_loan(deps.storage, &user, amount)?;

    Ok(Response::new()
        .add_attribute("action", "loan_increased")
        .add_attribute("user", user.to_string())
        .add_attribute("amount", amount.to_string()))
}

fn run_repay_loan(
    deps: DepsMut,
    user: Addr,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    user_decrease_loan(deps.storage, &user, amount)?;

    Ok(Response::new()
        .add_attribute("action", "loan_increased")
        .add_attribute("user", user.to_string()))
}

fn run_gate_revert_request(
    deps: DepsMut,
    gate: Addr,
    request: GateRequest,
) -> Result<Response, ContractError> {
    onlygate(deps.storage, &gate)?;

    if let GateRequest::SendMsg { msg, .. } = request {
        let bridge_msg: BridgeMsgInfo = from_binary(&msg)?;

        let user_addr = deps.api.addr_validate(bridge_msg.sender.as_str())?;

        user_increase_loan(deps.storage, &user_addr, bridge_msg.src_position.loan)?;

        for (denom, amount) in bridge_msg.src_position.collaterals {
            user_deposit(deps.storage, &user_addr, amount, denom)?;
        }

        Ok(Response::new()
            .add_attribute("action", "bridge_reverted")
            .add_attribute("sender", bridge_msg.sender)
            .add_attribute("receiver", user_addr.to_string()))
    } else {
        Err(ContractError::Std(StdError::generic_err(
            "Request not handled".to_string(),
        )))
    }
}

fn run_gate_receive_msg(
    deps: DepsMut,
    gate: Addr,
    _remote_contract: String,
    msg: Binary,
) -> Result<Response, ContractError> {
    onlygate(deps.storage, &gate)?;

    let bridge_msg: BridgeMsgInfo = from_binary(&msg)?;

    let user_addr = deps.api.addr_validate(bridge_msg.receiver.as_str())?;

    user_increase_loan(deps.storage, &user_addr, bridge_msg.dest_position.loan)?;

    for (denom, amount) in bridge_msg.dest_position.collaterals {
        user_deposit(deps.storage, &user_addr, amount, denom)?;
    }

    Ok(Response::new()
        .add_attribute("action", "bridge_received")
        .add_attribute("sender", bridge_msg.sender)
        .add_attribute("receiver", user_addr.to_string()))
}

// --- QUERIES ---

fn qy_position(deps: Deps, user: Addr) -> StdResult<Position> {
    let position = POSITIONS.load(deps.storage, user).unwrap();

    Ok(position)
}

// --- FUNCTIONS ---

fn onlyowner(storage: &dyn Storage, address: &Addr) -> Result<(), ContractError> {
    if OWNER.load(storage)? != *address {
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

fn onecoin(coins: Vec<Coin>) -> Result<Option<Coin>, ContractError> {
    if coins.len() == 1 {
        return Ok(Some(coins.first().unwrap().to_owned()));
    }

    if coins.is_empty() {
        Ok(None)
    } else {
        Err(ContractError::Std(StdError::generic_err(
            "Only one coin supported",
        )))
    }
}

fn user_deposit(
    storage: &mut dyn Storage,
    user: &Addr,
    amount: Uint128,
    denom: String,
) -> Result<Response, ContractError> {
    match POSITIONS.load(storage, user.to_owned()) {
        Ok(position) => {
            let coll_amount = position.collaterals.get(&denom);

            match coll_amount {
                // There is alredy an amount deposited
                Some(coll_amount) => {
                    let new_amount = coll_amount.to_owned() + amount;

                    POSITIONS.update(
                        storage,
                        user.to_owned(),
                        |position| -> Result<Position, ContractError> {
                            let mut position = position.unwrap();

                            position.collaterals.insert(denom.clone(), new_amount);

                            Ok(position)
                        },
                    )?;
                }
                None => {
                    POSITIONS.update(
                        storage,
                        user.to_owned(),
                        |position| -> Result<Position, ContractError> {
                            let mut position: Position = position.unwrap();

                            position.collaterals.insert(denom.clone(), amount);

                            Ok(position)
                        },
                    )?;
                }
            }
        }
        Err(_) => {
            let mut collaterals: HashMap<String, Uint128> = HashMap::new();
            collaterals.insert(denom.clone(), amount);

            POSITIONS.save(
                storage,
                user.to_owned(),
                &Position {
                    loan: Uint128::zero(),
                    collaterals,
                },
            )?;
        }
    }

    Ok(Response::new()
        .add_attribute("action", "deposit")
        .add_attribute("denom", denom)
        .add_attribute("amount", amount.to_string()))
}

fn user_withdraw(
    storage: &mut dyn Storage,
    user: &Addr,
    amount: Option<Uint128>,
    token_contract: String,
) -> Result<Uint128, ContractError> {
    match POSITIONS.load(storage, user.to_owned()) {
        Ok(position) => {
            let coll_amount = position.collaterals.get(&token_contract);

            match coll_amount {
                Some(coll_amount) => match amount {
                    Some(amount) => {
                        let new_amount = coll_amount.to_owned().checked_sub(amount).unwrap();

                        POSITIONS.update(
                            storage,
                            user.to_owned(),
                            |position| -> Result<Position, ContractError> {
                                let mut position: Position = position.unwrap();

                                position.collaterals.insert(token_contract, new_amount);
                                Ok(position)
                            },
                        )?;

                        Ok(coll_amount.to_owned())
                    }
                    None => {
                        POSITIONS.update(
                            storage,
                            user.to_owned(),
                            |position| -> Result<Position, ContractError> {
                                let mut position: Position = position.unwrap();

                                position.collaterals.remove(&token_contract).unwrap();

                                Ok(position)
                            },
                        )?;

                        Ok(coll_amount.to_owned())
                    }
                },
                None => Err(ContractError::CollateralNotFound {}),
            }
        }
        Err(_) => Err(ContractError::UserNotFound {}),
    }
}

fn user_increase_loan(
    storage: &mut dyn Storage,
    user: &Addr,
    amount: Uint128,
) -> Result<(), ContractError> {
    match POSITIONS.load(storage, user.to_owned()) {
        Ok(_) => {
            POSITIONS.update(
                storage,
                user.to_owned(),
                |position| -> Result<Position, ContractError> {
                    let mut position = position.unwrap();

                    position.loan += amount;

                    Ok(position)
                },
            )?;
        }
        Err(_) => {
            POSITIONS.save(
                storage,
                user.to_owned(),
                &Position {
                    loan: amount,
                    collaterals: HashMap::new(),
                },
            )?;
        }
    }

    Ok(())
}

fn user_decrease_loan(
    storage: &mut dyn Storage,
    user: &Addr,
    amount: Option<Uint128>,
) -> Result<(), ContractError> {
    match POSITIONS.load(storage, user.to_owned()) {
        Ok(pos) => {
            let amount = amount.unwrap_or(pos.loan);
            POSITIONS.update(
                storage,
                user.to_owned(),
                |position| -> Result<Position, ContractError> {
                    let mut position = position.unwrap();

                    position.loan = position.loan.checked_sub(amount).unwrap();

                    Ok(position)
                },
            )?;
        }
        Err(_) => return Err(ContractError::UserNotFound {}),
    }

    Ok(())
}
