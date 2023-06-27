use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin};
use cw_storage_plus::{Item, Map};

use crate::msgs::IBCLifecycleComplete;

pub const ACK_OK: Map<u64, IBCLifecycleComplete> = Map::new("ack_ok");
pub const ACK_FAILED: Map<u64, IBCLifecycleComplete> = Map::new("ack_failed");
pub const RESPONSE_OK: Map<u64, ReceivedRequest> = Map::new("response_ok");
pub const CONFIG: Item<Config> = Item::new("Config");

pub const SENDED_PACKET_INFO: Item<RequestInfo> = Item::new("sended_packet_info");

pub const ON_ACK_AWAIT: Map<(String, u64), RequestInfo> = Map::new("on_ack_await");

#[cw_serde]
pub struct RequestInfo {
    pub source_channel: String,
    pub sender: Addr,
    pub coin: Coin,
}

#[cw_serde]
pub struct Config {
    pub src_channel: String,
}

#[cw_serde]
pub struct ReceivedRequest {
    pub sender: Addr,
    pub funds: Vec<Coin>,
    pub to_fail: bool,
    pub from_address: String,
}
