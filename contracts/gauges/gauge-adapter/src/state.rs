use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, CosmosMsg, Uint128};
use cw_denom::CheckedDenom;
use cw_storage_plus::{Item, Map};

use crate::msg::{PossibleMsg, SubmissionMsg};

#[cw_serde]
pub struct Config {
    /// Address that is allowed to return deposits.
    pub admin: Addr,
    /// Deposit required for valid submission.
    pub required_deposit: Option<Asset>,
    /// Address of contract where each deposit is transferred.
    pub community_pool: Addr,
    /// Total reward amount.
    pub reward: Asset,
}

pub const CONFIG: Item<Config> = Item::new("c");
pub const POSSIBLE_MESSAGES: Item<Vec<PossibleMsg>> = Item::new("pm");

// All submissions mapped by fund destination address.
pub const SUBMISSIONS: Map<Addr, Submission> = Map::new("s");

#[cw_serde]
pub struct Asset {
    pub denom: CheckedDenom,
    pub amount: Uint128,
}

#[cw_serde]
pub struct Submission {
    pub sender: Addr,
    pub name: String,
    pub url: String,
    pub msg: SubmissionMsg,
}
