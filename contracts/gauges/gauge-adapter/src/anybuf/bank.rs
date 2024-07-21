use anybuf::{Anybuf, Bufany};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{ Addr, Binary, Coin, CosmosMsg, Decimal, Deps, Empty, StdResult};

use crate::{
    get_coins_from_bytes,
    msg::{AdapterBankMsg, PossibleMsg, SubmissionMsg},
    new_amount_gauge_fraction,
};

#[cw_serde]
pub struct ParseBankSubmissionResponse {
    pub coins: Vec<Coin>,
    pub recipient: String,
}

pub fn parse_stargate_wire_bank(
    _deps: Deps,
    anybuf: Anybuf,
    dao: Addr,
    msg: SubmissionMsg,
    bank_msg: AdapterBankMsg,
    fraction: Decimal,
    _possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    match bank_msg {
        AdapterBankMsg::MsgSend() => {
            // get amount from binaryMsg
            let bufany: ParseBankSubmissionResponse = parse_bank_transfer_msg_bufany(msg.msg);
            Ok(encode_bank_transfer_msg_anybuf(
                anybuf,
                dao.to_string(),
                bufany.recipient,
                bufany.coins,
                fraction.clone(),
            )?)
        } // AdapterBankMsg::MsgMultiSend() => todo!(),
          // todo: add msg in v0.50 feature
          // AdapterBankMsg::MsgBurn() => todo!(),
    }
}

pub fn parse_bank_transfer_msg_bufany(msg: Binary) -> ParseBankSubmissionResponse {
    let deserialized = Bufany::deserialize(&msg).unwrap();
    // bank msg coin proto = 3  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/bank/v1beta1/tx.proto#L48
    let coin_bytes = deserialized.repeated_bytes(3).unwrap();
    let recipient = deserialized.string(2).unwrap();
    let coins = get_coins_from_bytes(coin_bytes);

    ParseBankSubmissionResponse { coins, recipient }
}

pub fn encode_bank_transfer_msg_anybuf(
    anybuf: Anybuf,
    sender: String,
    recipient: String,
    coins: Vec<Coin>,
    fraction: Decimal,
) -> StdResult<CosmosMsg> {
    // bank msg coin proto = 3  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/bank/v1beta1/tx.proto#L48
    let mut anybuf_coins = vec![];

    for coin in coins {
        let amount = new_amount_gauge_fraction(coin.amount, fraction)?;
        let token = Anybuf::new().append_string(1, coin.denom).append_string(
            2,
            amount.to_string(), // applies the gauge calculation to each token sent
        );
        anybuf_coins.push(token)
    }

    let proto = anybuf
        .append_string(1, sender)
        .append_string(2, recipient) // sets the recipient as value in submission msg
        .append_repeated_message(3, &anybuf_coins)
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmos.bank.v1beta1.MsgSend".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}
