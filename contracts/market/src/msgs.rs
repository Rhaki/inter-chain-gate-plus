use std::collections::HashMap;

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Deps, StdError, StdResult, Uint128};
use cw20::Cw20ReceiveMsg;
use gate_pkg::{GateMsg, PacketPath};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    RegisterGate {
        contract: Addr,
    },
    GateSetPermission {
        contract: String,
        chain: String,
    },
    Withdraw {
        denom: String,
        amount: Option<Uint128>,
    },
    IncreaseLoan {
        amount: Uint128,
    },
    RepayLoan {
        amount: Option<Uint128>,
    },
    // Gate msg receive implementation
    ReceiveGateMsg(GateMsg),

    Deposit {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Position)]
    Position { user: Addr },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub enum Cw20MsgType {
    Deposit {},
}

#[cw_serde]
pub struct Position {
    pub loan: Uint128,
    pub collaterals: HashMap<String, Uint128>,
}

impl Position {
    pub fn is_bridgable(&self, deps: &Deps) -> StdResult<()> {
        let mut counter_native = 0;
        for denom in self.collaterals.keys() {
            if is_native(deps, denom) {
                counter_native += 1;
            }

            if counter_native > 1 {
                return Err(StdError::generic_err("More than one native coin detected"));
            }
        }

        Ok(())
    }
}

#[cw_serde]
pub enum GateCollectMsgsAllowed {
    BridgePosition {
        to_remote_addr: String,
        chain: String,
        native_info: Option<NativeInfo>,
    },
}

#[cw_serde]
pub struct BridgeMsgInfo {
    pub sender: String,
    pub receiver: String,
    pub src_position: Position,
    pub dest_position: Position,
}

pub fn is_native(deps: &Deps, contract: &str) -> bool {
    deps.api.addr_validate(contract).is_err()
}

#[cw_serde]
pub struct NativeInfo {
    pub path_middle_forward: Vec<PacketPath>,
    pub dest_denom: String,
    pub channel_id: String,
    pub timeout: Option<u64>,
}
