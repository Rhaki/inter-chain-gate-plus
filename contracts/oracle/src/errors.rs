use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Asset {asset} alredy registered")]
    AssetAlredyRegistered { asset: String },

    #[error("Price never feeded")]
    PriceNeverFeeded {},
}
