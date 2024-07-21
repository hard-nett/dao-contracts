use anybuf::{Anybuf, Bufany};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, Empty, StdResult,
    Uint128,
};
use cw20::Expiration;

use crate::{
    get_coins_from_bytes,
    msg::{AdapterCw20Msgs, AdapterWasmMsg, PossibleMsg, SubmissionMsg},
    new_amount_gauge_fraction,
};

#[cw_serde]
pub struct ParseCw20Response {
    contract: String,
    sender: String,
    coins: Vec<Coin>,
    exec_msg: Binary,
}

#[cw_serde]
pub struct Cw20TransferMsg {
    pub recipient: String,
    pub amount: Uint128,
}
#[cw_serde]
pub struct Cw20SendMsg {
    pub contract: String,
    pub amount: Uint128,
    pub msg: Binary,
}
#[cw_serde]
pub struct Cw20Allowance {
    pub spender: String,
    pub amount: Uint128,
    pub expires: Option<Expiration>,
}
#[cw_serde]
pub struct Cw20MintMsg {
    pub recipient: String,
    pub amount: Uint128,
}

pub fn parse_stargate_wire_wasm(
    _deps: Deps,
    anybuf: Anybuf,
    dao: Addr,
    msg: SubmissionMsg,
    wasm_msg: AdapterWasmMsg,
    fraction: Decimal,
    _possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    match wasm_msg {
        AdapterWasmMsg::Cw20(cw20_msgs) => match cw20_msgs {
            AdapterCw20Msgs::Transfer() => {
                let bufany = parse_cw20_bufany(msg.msg);
                encode_cw20_transfer_anybuf(
                    anybuf,
                    bufany.contract,
                    dao.to_string(),
                    bufany.exec_msg,
                    bufany.coins,
                    fraction,
                )
            }
            AdapterCw20Msgs::Send() => {
                let bufany = parse_cw20_bufany(msg.msg);
                encode_cw20_send_anybuf(
                    anybuf,
                    bufany.contract,
                    bufany.sender,
                    bufany.exec_msg,
                    bufany.coins,
                    fraction,
                )
            }
            AdapterCw20Msgs::IncreaseAllowance() => {
                let bufany = parse_cw20_bufany(msg.msg);
                encode_cw20_allowance_anybuf(
                    anybuf,
                    bufany.contract,
                    bufany.sender,
                    bufany.exec_msg,
                    bufany.coins,
                    fraction,
                )
            }
            AdapterCw20Msgs::DecreaseAllowance() => {
                let bufany = parse_cw20_bufany(msg.msg);
                encode_cw20_allowance_anybuf(
                    anybuf,
                    bufany.contract,
                    bufany.sender,
                    bufany.exec_msg,
                    bufany.coins,
                    fraction,
                )
            }
            AdapterCw20Msgs::Mint() => {
                let bufany = parse_cw20_bufany(msg.msg);
                encode_cw20_mint_anybuf(
                    anybuf,
                    bufany.contract,
                    bufany.sender,
                    bufany.exec_msg,
                    bufany.coins,
                    fraction,
                )
            }
        },
    }
}

//// TRANSFER  ////
pub fn parse_cw20_bufany(msg: Binary) -> ParseCw20Response {
    let deserialized = Bufany::deserialize(&msg).unwrap();
    // msg metadata
    let sender = deserialized.string(1).unwrap();
    let contract = deserialized.string(2).unwrap();
    // defines cw20 msg
    let wasm_binary = deserialized.bytes(3).unwrap();
    let exec_msg = Binary::from(wasm_binary.clone());
    // defines coins
    let coin_bytes = deserialized.repeated_bytes(5).unwrap();
    let coins = get_coins_from_bytes(coin_bytes);

    ParseCw20Response {
        contract,
        sender,
        coins,
        exec_msg,
    }
}

pub fn encode_cw20_transfer_anybuf(
    anybuf: Anybuf,
    contract: String,
    sender: String,
    msg: Binary,
    _coins: Vec<Coin>,
    fraction: Decimal,
) -> StdResult<CosmosMsg> {
    let mut transfer_msg: Cw20TransferMsg = from_json(&msg)?;
    transfer_msg.amount = new_amount_gauge_fraction(transfer_msg.amount, fraction.clone())?;

    let proto = anybuf
        .append_string(1, sender.clone()) // sender
        .append_string(2, contract.clone()) // contract
        .append_bytes(3, to_json_binary(&transfer_msg)?) // binary of transfer msg
        .append_repeated_message::<Anybuf>(5, &[]) // empty native tokens sent for now.
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmwasm.v1.wasm.MsgExecuteContract".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}

//// SEND  ////

pub fn encode_cw20_send_anybuf(
    anybuf: Anybuf,
    contract: String,
    sender: String,
    msg: Binary,
    _coins: Vec<Coin>,
    fraction: Decimal,
) -> StdResult<CosmosMsg> {
    // unwraps the cw20 msg from binary
    let mut send_msg: Cw20SendMsg = from_json(&msg)?;
    // updates the amount with the gauge fraction
    send_msg.amount = new_amount_gauge_fraction(send_msg.amount, fraction.clone())?;

    let proto = anybuf
        .append_string(1, sender.clone()) // sender (dao)
        .append_string(2, contract.clone()) // cw20 contract
        .append_bytes(3, to_json_binary(&send_msg)?) // updated binary of transfer msg.
        .append_repeated_message::<Anybuf>(5, &[]) // empty native tokens sent for now.
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmwasm.v1.wasm.MsgExecuteContract".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}

//// INCREASE OR DECREASE ALLOWANCE ////
pub fn encode_cw20_allowance_anybuf(
    anybuf: Anybuf,
    contract: String,
    sender: String,
    msg: Binary,
    _coins: Vec<Coin>,
    fraction: Decimal,
) -> StdResult<CosmosMsg> {
    // unwraps the cw20 msg from binary
    let mut allowance: Cw20Allowance = from_json(&msg)?;
    // updates the amount with the gauge fraction
    allowance.amount = new_amount_gauge_fraction(allowance.amount, fraction.clone())?;

    let proto = anybuf
        .append_string(1, sender.clone()) // sender (DAO)
        .append_string(2, contract.clone()) // cw20 contract
        .append_bytes(3, to_json_binary(&allowance)?) // updated binary of transfer msg.
        .append_repeated_message::<Anybuf>(5, &[]) // empty native tokens sent for now.
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmwasm.v1.wasm.MsgExecuteContract".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}

//// MINT CW20 ////
pub fn encode_cw20_mint_anybuf(
    anybuf: Anybuf,
    contract: String,
    sender: String,
    msg: Binary,
    _coins: Vec<Coin>,
    fraction: Decimal,
) -> StdResult<CosmosMsg> {
    // unwraps the cw20 msg from binary
    let mut mint: Cw20MintMsg = from_json(&msg)?;
    // updates the amount with the gauge fraction
    mint.amount = new_amount_gauge_fraction(mint.amount, fraction.clone())?;

    let proto = anybuf
        .append_string(1, sender.clone()) // sender (DAO)
        .append_string(2, contract.clone()) // cw20 contract
        .append_bytes(3, to_json_binary(&mint)?) // updated binary of transfer msg.
        .append_repeated_message::<Anybuf>(5, &[]) // empty native tokens sent for now.
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmwasm.v1.wasm.MsgExecuteContract".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}
