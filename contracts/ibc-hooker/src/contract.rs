use std::cmp::min;

#[cfg(not(feature = "library"))]
use cosmwasm_std::{entry_point, DepsMut, Env, MessageInfo, Response};
use cosmwasm_std::{
    to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, Order, Reply, StdError, StdResult,
    Storage, SubMsg, SubMsgResponse, SubMsgResult,
};
use cw_storage_plus::{Bound, KeyDeserialize, Map, PrimaryKey};
use prost::Message;
use schemars::_serde_json::to_string_pretty;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    msgs::{
        ExecuteMsg, Forward, ForwardField, IBCLifecycleComplete, InstantiateMsg, MemoField,
        MsgReplyID, QueryMsg, QueryResponse, SudoMsg, WasmField,
    },
    proto::{MsgTransfer, MsgTransferResponse},
    state::{
        ReceivedRequest, RequestInfo, ACK_FAILED, ACK_OK, ON_ACK_AWAIT, RESPONSE_OK,
        SENDED_PACKET_INFO,
    },
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, StdError> {
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::Request {
            to_fail,
            receiver_ibc_hooker,
            channel_id,
            to_address,
            forward,
        } => run_request(
            deps,
            env,
            info.sender,
            info.funds,
            channel_id,
            receiver_ibc_hooker,
            to_fail,
            to_address,
            forward,
        ),
        ExecuteMsg::ReceivedRequest {
            to_fail,
            from_address,
            to_address,
        } => run_received_request(
            deps,
            env,
            info.sender,
            info.funds,
            to_fail,
            from_address,
            to_address,
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => qy_state(deps),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> StdResult<Response> {
    match MsgReplyID::from_repr(reply.id) {
        Some(MsgReplyID::SendPacket) => reply_packet_sended(deps, reply),
        None => Err(StdError::generic_err(format!(
            "invalid reply id {}",
            reply.id
        ))),
    }
}

#[cfg_attr(not(feature = "imported"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> StdResult<Response> {
    match msg {
        SudoMsg::IBCLifecycleComplete(ack) => match ack {
            IBCLifecycleComplete::IBCAck {
                success,
                channel,
                sequence,
                ack,
            } => {
                if success {
                    on_ack_ok(deps, channel, sequence, ack)
                } else {
                    on_ack_failed(deps, channel, sequence, ack)
                }
            }
            IBCLifecycleComplete::IBCTimeout { channel, sequence } => {
                on_ack_failed(deps, channel, sequence, "timeout".to_string())
            }
        },
    }
}

// --- EXECUTE_MSG ---

#[allow(clippy::too_many_arguments)]
pub fn run_request(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    funds: Vec<Coin>,
    channel_id: String,
    receiver_ibc_hooker: String,
    to_fail: bool,
    to_address: String,
    forward: Option<Forward>,
) -> Result<Response, StdError> {
    let coin = only_one_coin(funds)?;

    let memo = match forward {
        Some(forward) => to_string_pretty(&MemoField {
            wasm: None,
            ibc_callback: Some(env.contract.address.to_string()),
            forward: Some(ForwardField::<ExecuteMsg> {
                receiver: forward.receiver.clone(),
                port: "transfer".to_string(),
                channel: forward.channel,
                // timeout: "10m".to_string(),
                // retries: 2,
                next: Some(Box::new(MemoField {
                    wasm: Some(WasmField {
                        contract: forward.receiver,
                        msg: ExecuteMsg::ReceivedRequest {
                            to_fail,
                            from_address: sender.to_string(),
                            to_address,
                        },
                    }),
                    ibc_callback: None,
                    forward: None,
                })), // next:None
            }),
        })
        .unwrap(),
        None => to_string_pretty(&MemoField {
            wasm: Some(WasmField {
                contract: receiver_ibc_hooker.clone(),
                msg: ExecuteMsg::ReceivedRequest {
                    to_fail,
                    from_address: sender.to_string(),
                    to_address,
                },
            }),
            ibc_callback: Some(env.contract.address.to_string()),
            forward: None,
        })
        .unwrap(),
    };

    let msg = MsgTransfer {
        source_port: "transfer".to_string(),
        source_channel: channel_id.clone(),
        token: Some(coin.clone().into()),
        sender: env.contract.address.to_string(),
        receiver: receiver_ibc_hooker,
        timeout_height: None,
        timeout_timestamp: Some(env.block.time.plus_seconds(604_800u64).nanos()),
        memo,
    };

    SENDED_PACKET_INFO.save(
        deps.storage,
        &RequestInfo {
            source_channel: channel_id,
            sender,
            coin,
        },
    )?;

    Ok(
        Response::new()
            .add_submessage(SubMsg::reply_on_success(msg, MsgReplyID::SendPacket.repr())),
    )
}

pub fn run_received_request(
    deps: DepsMut,
    _env: Env,
    sender: Addr,
    funds: Vec<Coin>,
    to_fail: bool,
    from_address: String,
    to_address: String,
) -> Result<Response, StdError> {
    if to_fail {
        return Err(StdError::generic_err("need_to_fail"));
    }

    let key = get_last_key(deps.storage, &RESPONSE_OK).unwrap_or(0) + 1;

    RESPONSE_OK.save(
        deps.storage,
        key,
        &ReceivedRequest {
            sender: sender.clone(),
            funds: funds.clone(),
            to_fail,
            from_address,
        },
    )?;

    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address,
        amount: funds,
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("received_request", "true")
        .add_attribute("sender", sender))
}

// --- QUERY ----

pub fn qy_state(deps: Deps) -> StdResult<Binary> {
    let ack_ok = get_last_items(deps.storage, &ACK_OK, None, None)?;
    let ack_failed = get_last_items(deps.storage, &ACK_FAILED, None, None)?;
    let response_ok = get_last_items(deps.storage, &RESPONSE_OK, None, None)?;

    Ok(to_binary(&QueryResponse::State {
        response_ok,
        ack_ok,
        ack_failed,
    })
    .unwrap())
}

// --- REPLY ---

fn reply_packet_sended(deps: DepsMut, reply: Reply) -> StdResult<Response> {
    let SubMsgResult::Ok(SubMsgResponse { data: Some(b), .. }) = reply.result else {
        return Err(StdError::generic_err( format!("failed reply: {:?}", reply.result) ))
    };

    let response = MsgTransferResponse::decode(&b[..])
        .map_err(|_e| StdError::generic_err(format!("could not decode response: {b}")))?;

    let request_info = SENDED_PACKET_INFO.load(deps.storage)?;
    ON_ACK_AWAIT.save(
        deps.storage,
        (request_info.clone().source_channel, response.sequence),
        &request_info,
    )?;

    Ok(Response::new().add_attribute("reply_result", response.sequence.to_string()))
}

// --- ACK ---

pub fn on_ack_ok(
    deps: DepsMut,
    channel: String,
    sequence: u64,
    ack: String,
) -> StdResult<Response> {
    let key = get_last_key(deps.storage, &ACK_OK).unwrap_or(0) + 1;
    ACK_OK.save(
        deps.storage,
        key,
        &IBCLifecycleComplete::IBCAck {
            channel: channel.clone(),
            sequence,
            ack,
            success: true,
        },
    )?;

    ON_ACK_AWAIT.remove(deps.storage, (channel, sequence));

    Ok(Response::new().add_attribute("ack_status", "ok"))
}

pub fn on_ack_failed(
    deps: DepsMut,
    channel: String,
    sequence: u64,
    ack: String,
) -> StdResult<Response> {
    let key = get_last_key(deps.storage, &ACK_FAILED).unwrap_or(0) + 1;
    ACK_FAILED.save(
        deps.storage,
        key,
        &IBCLifecycleComplete::IBCAck {
            channel: channel.clone(),
            sequence,
            ack,
            success: false,
        },
    )?;

    let original_request = ON_ACK_AWAIT.load(deps.storage, (channel.clone(), sequence))?;

    ON_ACK_AWAIT.remove(deps.storage, (channel, sequence));

    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: original_request.sender.to_string(),
        amount: vec![original_request.coin],
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("ack_status", "ok"))
}

// --- FUNCTIONS ---

fn only_one_coin(coins: Vec<Coin>) -> StdResult<Coin> {
    if coins.len() == 1 {
        Ok(coins.first().unwrap().to_owned())
    } else {
        Err(StdError::generic_err("Not one coin"))
    }
}

fn get_last_key<
    'a,
    T: Serialize + DeserializeOwned,
    K: PrimaryKey<'a> + KeyDeserialize + 'static,
>(
    storage: &dyn Storage,
    map: &Map<'a, K, T>,
) -> Option<K::Output> {
    map.range(storage, None, None, Order::Descending)
        .take(1)
        .last()
        .map(|v| v.unwrap().0)
}

const DEFAULT_LIMIT: u64 = 10;
const MAX_LIMIT: u64 = 30;

fn get_last_items<
    'a,
    T: Serialize + DeserializeOwned,
    K: PrimaryKey<'a> + KeyDeserialize + 'static,
>(
    storage: &dyn Storage,
    map: &Map<'a, K, T>,
    limit: Option<u64>,
    start_after: Option<K>,
) -> StdResult<Vec<(K::Output, T)>> {
    let limit = usize::try_from(min(MAX_LIMIT, limit.unwrap_or(DEFAULT_LIMIT))).unwrap();

    let start_after = start_after.map(Bound::exclusive);

    Ok(map
        .range(storage, None, start_after, Order::Descending)
        .take(limit)
        .map(|item| item.unwrap())
        .collect())
}
