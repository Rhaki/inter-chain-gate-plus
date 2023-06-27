use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::{Item, Map};

// --- CONSTANTS ---
pub const OWNER: Item<Addr> = Item::new("owner");
pub const ASSETS: Map<String, Asset> = Map::new("assets");

pub const GATE: Item<Addr> = Item::new("gate");
pub const CHAINS_CONTRACT: Map<String, String> = Map::new("chains_contracts");

#[cw_serde]
pub struct Asset {
    pub feeder: Addr,
    pub price: Option<Decimal>,
}
