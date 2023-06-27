use crate::msgs::Position;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

// --- CONSTANTS ---

pub const OWNER: Item<Addr> = Item::new("addr");

pub const GATE: Item<Addr> = Item::new("gate");
pub const POSITIONS: Map<Addr, Position> = Map::new("position");
pub const CHAINS_CONTRACT: Map<String, String> = Map::new("chains_contracts");
