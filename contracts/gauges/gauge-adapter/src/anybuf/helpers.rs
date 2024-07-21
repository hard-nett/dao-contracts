use anybuf::{Anybuf, Bufany};
use cosmwasm_std::{coin, Addr, Coin, CosmosMsg, Decimal, Deps, StdError, StdResult, Uint128};

use crate::{
    msg::StargateWire,
    parse_stargate_wire_authz, parse_stargate_wire_bank, parse_stargate_wire_distribution,
    parse_stargate_wire_staking, parse_stargate_wire_wasm,
    state::{Submission, CONFIG, POSSIBLE_MESSAGES, SUBMISSIONS},
};

/// Primary router for gauge orchestrator.
pub fn stargate_to_anybuf(deps: Deps, winner: Addr, fraction: Decimal) -> StdResult<CosmosMsg> {
    let anybuf: Anybuf = Anybuf::new();
    // get msgs from winners submission
    let Submission { msg, .. } = SUBMISSIONS.load(deps.storage, winner)?;
    //
    let dao = CONFIG.load(deps.storage)?.admin;
    let possible = POSSIBLE_MESSAGES.load(deps.storage)?;

    match msg.stargate.clone() {
        // Bank module actions
        StargateWire::Bank(b) => parse_stargate_wire_bank(
            deps.clone(),
            anybuf,
            dao.clone(),
            msg.clone(),
            b.clone(),
            fraction.clone(),
            possible.clone(),
        ),
        // Wasm message actions
        StargateWire::Wasm(wasm_msg) => {
            parse_stargate_wire_wasm(deps, anybuf, dao, msg, wasm_msg, fraction, possible)
        }
        //
        StargateWire::Distribution(distr_msg) => {
            parse_stargate_wire_distribution(deps, anybuf, dao, msg, distr_msg, fraction, possible)
        }
        StargateWire::Staking(stake_msg) => {
            parse_stargate_wire_staking(deps, anybuf, dao, msg, stake_msg, fraction, possible)
        }
        StargateWire::Authz(authz_msg) => {
            parse_stargate_wire_authz(deps, anybuf, dao, msg, authz_msg, fraction, possible)
        }
        StargateWire::Gov(_) => todo!(),
    }
}

// creates coins from bufany bytes
pub fn get_coins_from_bytes(coin_bytes: Vec<Vec<u8>>) -> Vec<Coin> {
    let mut coins = vec![];
    for bytes in coin_bytes {
        let this = bytes.clone().to_vec();
        let repeated = Bufany::deserialize(&this).unwrap();
        let coin = coin(
            u128::from_str_radix(&repeated.string(2).clone().unwrap(), 10).unwrap(),
            repeated.string(1).clone().unwrap(),
        );
        coins.push(coin)
    }
    coins
}
// creates coins from bufany bytes
pub fn get_coin_from_bytes(coin_bytes: Vec<u8>) -> Coin {
    let bufany_token = Bufany::deserialize(&coin_bytes).unwrap();
    let coin = coin(
        u128::from_str_radix(&bufany_token.string(2).clone().unwrap(), 10).unwrap(),
        bufany_token.string(1).clone().unwrap(),
    );
    coin
}

pub fn new_amount_gauge_fraction(amnt: Uint128, fraction: Decimal) -> StdResult<Uint128> {
    Ok(amnt
        .checked_mul_floor(fraction)
        .map_err(|x| StdError::generic_err(x.to_string()))?)
}
