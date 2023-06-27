use cosmwasm_schema::cw_serde;
use enum_repr::EnumRepr;
use serde::Serialize;

use crate::state::ReceivedRequest;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    Request {
        channel_id: String,
        to_fail: bool,
        receiver_ibc_hooker: String,
        to_address: String,
        forward: Option<Forward>,
    },
    ReceivedRequest {
        from_address: String,
        to_fail: bool,
        to_address: String,
    },
}

#[cw_serde]
pub enum QueryMsg {
    State {},
}

#[cw_serde]
pub enum QueryResponse {
    State {
        response_ok: Vec<(u64, ReceivedRequest)>,
        ack_ok: Vec<(u64, IBCLifecycleComplete)>,
        ack_failed: Vec<(u64, IBCLifecycleComplete)>,
    },
}

/// Message type for `sudo` entry_point
#[cw_serde]
pub enum SudoMsg {
    #[serde(rename = "ibc_lifecycle_complete")]
    IBCLifecycleComplete(IBCLifecycleComplete),
}

#[cw_serde]
pub struct MemoField<T: Serialize> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forward: Option<ForwardField<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm: Option<WasmField<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ibc_callback: Option<String>,
}

#[cw_serde]
pub struct ForwardField<T: Serialize> {
    pub receiver: String,
    pub port: String,
    pub channel: String,
    // pub timeout: String,
    // pub retries: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Box<MemoField<T>>>,
}

#[cw_serde]
pub struct WasmField<T: Serialize> {
    pub contract: String,
    pub msg: T,
}

#[cw_serde]
pub struct Forward {
    pub receiver: String,
    pub channel: String,
}

#[cw_serde]
pub enum IBCLifecycleComplete {
    #[serde(rename = "ibc_ack")]
    IBCAck {
        /// The source channel (osmosis side) of the IBC packet
        channel: String,
        /// The sequence number that the packet was sent with
        sequence: u64,
        /// String encoded version of the ack as seen by OnAcknowledgementPacket(..)
        ack: String,
        /// Weather an ack is a success of failure according to the transfer spec
        success: bool,
    },
    #[serde(rename = "ibc_timeout")]
    IBCTimeout {
        /// The source channel (osmosis side) of the IBC packet
        channel: String,
        /// The sequence number that the packet was sent with
        sequence: u64,
    },
}

#[EnumRepr(type = "u64")]
pub enum MsgReplyID {
    SendPacket = 1,
}
