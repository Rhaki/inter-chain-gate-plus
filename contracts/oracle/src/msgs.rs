use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal};
use gate_pkg::GateMsg;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    RegisterAsset { asset: String, feeder: Addr },

    FeedPrice { asset: String, price: Decimal },

    // Remote iteration
    FeedRemotePrice { asset: String, chain: String },

    // Gate permission and registration
    RegisterGate { contract: Addr },

    GateSetPermission { contract: String, chain: String },

    // Gate msg receive implementation
    ReceiveGateMsg(GateMsg),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Decimal)]
    Price { asset: String },
    #[returns(Vec<(String, Decimal)>)]
    Prices {
        start_after: Option<String>,
        limit: Option<u64>,
    },
}
