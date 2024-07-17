use anybuf::{Anybuf, Bufany};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, from_json, from_slice, to_json_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps,
    Empty, StdError, StdResult, Uint128,
};

use crate::{
    get_coin_from_bytes,
    msg::{AdapterBankMsg, AdapterCw20Msgs, AdapterWasmMsg, PossibleMsg, SubmissionMsg},
    state::CONFIG,
};

#[cw_serde]
pub struct ParseCw20TransferResponse {
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

pub fn parse_stargate_wire_wasm(
    deps: Deps,
    anybuf: Anybuf,
    dao: Addr,
    msg: SubmissionMsg,
    wasm_msg: AdapterWasmMsg,
    fraction: Decimal,
    possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    match wasm_msg {
        AdapterWasmMsg::Cw20(cw20_msgs) => {
            match cw20_msgs {
                AdapterCw20Msgs::Transfer() => {
                    let bufany = parse_cw20_transfer_bufany(msg.msg);
                    encode_cw20_transfer_anybuf(
                        deps,
                        anybuf,
                        bufany.contract,
                        bufany.sender,
                        bufany.exec_msg,
                        bufany.coins,
                        fraction,
                    )
                } // AdapterCw20Msgs::Send() => parse_cw20_send_bufany(msg.msg),
            }
        }
    }
}

pub fn parse_cw20_transfer_bufany(msg: Binary) -> ParseCw20TransferResponse {
    let deserialized = Bufany::deserialize(&msg).unwrap();

    let sender = deserialized.string(1).unwrap();
    let contract = deserialized.string(2).unwrap();
    let coin_bytes = deserialized.repeated_bytes(5).unwrap();
    let coins = get_coin_from_bytes(coin_bytes);
    // define cw20 msg
    let wasm_binary = deserialized.bytes(3).unwrap();
    let exec_msg = Binary::from(wasm_binary.clone());

    ParseCw20TransferResponse {
        contract,
        sender,
        coins,
        exec_msg,
    }
}

pub fn encode_cw20_transfer_anybuf(
    deps: Deps,
    anybuf: Anybuf,
    contract: String,
    sender: String,
    msg: Binary,
    coins: Vec<Coin>,
    fraction: Decimal,
) -> StdResult<CosmosMsg> {
    let mut anybuf_coins = vec![];
    let mut transfer_msg: Cw20TransferMsg = from_json(&msg)?;
    transfer_msg.amount = transfer_msg
        .amount
        .checked_mul_floor(fraction)
        .map_err(|x| StdError::generic_err(x.to_string()))?;

    for coin in coins {
        let amount = coin
            .amount
            .checked_mul_floor(fraction)
            .map_err(|x| StdError::generic_err(x.to_string()))?;

        let token = Anybuf::new().append_string(1, coin.denom).append_string(
            2,
            amount.to_string(), // applies the gauge calculation to each token sent
        );
        anybuf_coins.push(token)
    }

    let proto = anybuf
        .append_string(1, sender.clone()) // sender
        .append_string(2, contract.clone()) // contract
        .append_bytes(3, to_json_binary(&transfer_msg)?) // binary of transfer msg
        .append_repeated_message(5, &anybuf_coins)
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmwasm.v1.wasm.MsgExecuteContract".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}

// pub fn encode_wasm_submission_init_anybuf(
//     deps: Deps,
//     anybuf: Anybuf,
//     id: u64,
//     msg: Vec<u8>,
//     coins: Vec<Coin>,
//     sender: String,
//     fraction: Decimal,
//     possible: Vec<PossibleMsg>,
// ) -> StdResult<CosmosMsg> {
//     // bank msg coin proto = 3  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/bank/v1beta1/tx.proto#L48
//     let mut anybuf_coins = vec![];

//     for coin in coins {
//         let amount = coin
//             .amount
//             .checked_mul_floor(fraction)
//             .map_err(|x| StdError::generic_err(x.to_string()))?;

//         for a in &possible {
//             if let Some(b) = a.max_amount {
//                 if !amount.lt(&b) {
//                     return Err(StdError::GenericErr { msg: "invalid message. amount is more than maximum permitted for this gauge adapter".into() });
//                 };
//             }
//         }

//         let token = Anybuf::new().append_string(1, coin.denom).append_string(
//             2,
//             amount.to_string(), // applies the gauge calculation to each token sent
//         );
//         anybuf_coins.push(token)
//     }

//     let proto = anybuf
//         // sender
//         .append_string(1, sender.clone())
//         // admin
//         .append_string(2, sender.clone()) // sets the recipient as value in submission msg
//         // code_id
//         .append_uint64(3, id)
//         // contract label
//         .append_string(4, "contract created via gauge orch".to_string())
//         // raw init msg
//         .append_bytes(5, msg)
//         .append_repeated_message(6, &anybuf_coins)
//         .into_vec();

//     let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
//         type_url: "/cosmwasm.v1.wasm.MsgSend".to_string(),
//         value: proto.into(),
//     };
//     Ok(msg)
// }

// pub fn parse_wasm_submission_init_bufany(msg: Binary) -> ParseWasmSubmissionInitResponse {

//     let mut coins = vec![];
//     // wasm instantiate msg coin proto = 6 https://github.com/CosmWasm/wasmd/blob/main/proto/cosmwasm/wasm/v1/tx.proto#L113C9-L113C31
//     let coin_bytes = deserialized.repeated_bytes(6).unwrap();
//     let code_id = deserialized.uint64(3).unwrap();
//     let init_msg = deserialized.bytes(5).unwrap();

//     for bytes in coin_bytes {
//         let this = bytes.clone().to_vec();
//         let repeated = Bufany::deserialize(&this).unwrap();
//         let coin = coin(
//             u128::from_str_radix(&repeated.string(2).clone().unwrap(), 10).unwrap(),
//             repeated.string(1).clone().unwrap(),
//         );
//         coins.push(coin)
//     }
//     ParseWasmSubmissionResponse {
//         code_id,
//         coins,
//         init_msg,
//     }
// }
