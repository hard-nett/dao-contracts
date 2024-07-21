use anybuf::{Anybuf, Bufany};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, Coin, CosmosMsg, Decimal, Deps, Empty, StdResult};

use crate::{
    get_coins_from_bytes,
    msg::{AdapterDistributionMsg, PossibleMsg, SubmissionMsg},
    new_amount_gauge_fraction,
};

#[cw_serde]
pub struct ParseDistrSubmissionResponse {
    pub coins: Vec<Coin>,
}

pub fn parse_stargate_wire_distribution(
    _deps: Deps,
    anybuf: Anybuf,
    dao: Addr,
    msg: SubmissionMsg,
    distr_msg: AdapterDistributionMsg,
    fraction: Decimal,
    _possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    match distr_msg {
        AdapterDistributionMsg::MsgFundCommunityPool() => {
            // get amount from binaryMsg
            let bufany: ParseDistrSubmissionResponse = parse_distr_submission_msg_bufany(msg.msg);
            Ok(encode_fund_community_pool_anybuf(
                anybuf,
                bufany.coins,
                dao.to_string(),
                fraction.clone(),
            )?)
        }
    }
}

pub fn parse_distr_submission_msg_bufany(msg: Binary) -> ParseDistrSubmissionResponse {
    let deserialized = Bufany::deserialize(&msg).unwrap();
    // distribution msg coin proto = 1  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/distribution/v1beta1/tx.proto#L123
    let coin_bytes = deserialized.repeated_bytes(1).unwrap();
    let coins = get_coins_from_bytes(coin_bytes);

    ParseDistrSubmissionResponse { coins }
}

pub fn encode_fund_community_pool_anybuf(
    anybuf: Anybuf,
    coins: Vec<Coin>,
    sender: String,
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
        .append_repeated_message(1, &anybuf_coins)
        .append_string(2, &sender)
        .into_vec();
    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmos.distribution.v1beta1.MsgFundCommunityPool".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}
