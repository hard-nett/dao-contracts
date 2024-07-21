use anybuf::{Anybuf, Bufany};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, Coin, CosmosMsg, Decimal, Deps, Empty, StdResult};

use crate::{
    get_coins_from_bytes,
    msg::{AdapterGovMsg, PossibleMsg, SubmissionMsg},
    new_amount_gauge_fraction,
};

#[cw_serde]
pub struct ParseGovPropSubmissionResponse {
    title: String,
    summary: String,
    messages: Vec<Vec<u8>>,
    metadata: String,
    deposit: Vec<Coin>,
}

pub fn parse_stargate_wire_gov(
    _deps: Deps,
    anybuf: Anybuf,
    dao: Addr,
    msg: SubmissionMsg,
    gov_msg: AdapterGovMsg,
    fraction: Decimal,
    _possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    match gov_msg {
        AdapterGovMsg::MsgSendGovProp() => {
            // get amount from binaryMsg
            let bufany: ParseGovPropSubmissionResponse = parse_gov_prop_msg_bufany(msg.msg);
            Ok(encode_gov_prop_msg_anybuf(
                anybuf,
                dao.to_string(),
                bufany.messages,
                bufany.deposit,
                bufany.title,
                bufany.metadata,
                bufany.summary,
                fraction.clone(),
            )?)
        }
    }
}

pub fn parse_gov_prop_msg_bufany(msg: Binary) -> ParseGovPropSubmissionResponse {
    let deserialized = Bufany::deserialize(&msg).unwrap();
    // v1 gov prop // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/gov/v1/tx.proto#L50C9-L50C26
    let msg_bytes = deserialized.repeated_bytes(1).unwrap();
    let coin_bytes = deserialized.repeated_bytes(2).unwrap();
    // let proposer = deserialized.string(3).unwrap(); // verify DAO addr?
    let metadata = deserialized.string(4).unwrap();
    let title = deserialized.string(5).unwrap();
    let summary = deserialized.string(6).unwrap();
    let coins = get_coins_from_bytes(coin_bytes);

    ParseGovPropSubmissionResponse {
        deposit: coins,
        messages: msg_bytes,
        metadata,
        title,
        summary,
    }
}

pub fn encode_gov_prop_msg_anybuf(
    anybuf: Anybuf,
    proposer: String,
    prop_msgs: Vec<Vec<u8>>,
    coins: Vec<Coin>,
    title: String,
    metadata: String,
    summary: String,
    fraction: Decimal,
) -> StdResult<CosmosMsg> {
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
        .append_repeated_bytes(1, &prop_msgs) // proposal messages
        .append_repeated_message(2, &anybuf_coins) // sets the recipient as value in submission msg
        .append_string(3, proposer)
        .append_string(4, metadata)
        .append_string(5, title)
        .append_string(6, summary)
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmos.gov.v1.MsgSubmitProposal".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}
