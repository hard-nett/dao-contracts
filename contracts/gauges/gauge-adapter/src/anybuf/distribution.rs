use anybuf::{Anybuf, Bufany};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, Empty, StdError, StdResult,
};

use crate::{
    get_coin_from_bytes, get_coins_from_bytes, msg::{ AdapterDistributionMsg, PossibleMsg, SubmissionMsg}, state::CONFIG
};

#[cw_serde]
pub struct ParseDistrSubmissionResponse {
    pub coins: Vec<Coin>,
}

pub fn parse_stargate_wire_distribution(
    deps: Deps,
    anybuf: Anybuf,
    dao: Addr,
    msg: SubmissionMsg,
    distr_msg: AdapterDistributionMsg,
    fraction: Decimal,
    possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    match distr_msg {
        AdapterDistributionMsg::MsgFundCommunityPool() => {
            // get amount from binaryMsg
            let bufany: ParseDistrSubmissionResponse = parse_distr_submission_msg_bufany(msg.msg);
            Ok(encode_fund_community_pool_anybuf(
                deps.clone(),
                anybuf,
                bufany.coins,
                dao.to_string(),
                fraction.clone(),
                possible.clone(),
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
    deps: Deps,
    anybuf: Anybuf,
    coins: Vec<Coin>,
    sender: String,
    fraction: Decimal,
    possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    // bank msg coin proto = 1  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/bank/v1beta1/tx.proto#L48
    let mut anybuf_coins = vec![];

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

    let proto = anybuf.append_repeated_message(1, &anybuf_coins).into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmos.distribution.v1beta1.MsgFundCommunityPool".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}
