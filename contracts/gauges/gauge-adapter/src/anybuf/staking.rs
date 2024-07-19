use anybuf::{Anybuf, Bufany};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, Empty, StdError, StdResult,
};

use crate::{
    get_coin_from_bytes,
    msg::{AdapterBankMsg, AdapterStakingMsg, PossibleMsg, SubmissionMsg},
    state::CONFIG,
};

#[cw_serde]
pub struct ParseStakingSubmissionResponse {
    pub amount: Coin,
    pub recipient: String,
}
#[cw_serde]
pub struct ParseRedelegationSubmissionResponse {
    pub amount: Coin,
    pub old: String,
    pub new: String,
}

pub fn parse_stargate_wire_staking(
    deps: Deps,
    anybuf: Anybuf,
    dao: Addr,
    msg: SubmissionMsg,
    stake_msg: AdapterStakingMsg,
    fraction: Decimal,
    possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    match stake_msg {
        AdapterStakingMsg::MsgDelegate() => {
            let bufany = parse_delegate_msg_bufany(msg.msg);
            Ok(encode_delegate_msg_anybuf(
                deps.clone(),
                anybuf,
                bufany.amount,
                dao.to_string(),
                bufany.recipient,
                fraction.clone(),
                possible.clone(),
            )?)
        }
        AdapterStakingMsg::MsgRedelegate() => {
            let bufany = parse_redelegate_msg_bufany(msg.msg);
            Ok(encode_redelegate_msg_anybuf(
                deps.clone(),
                anybuf,
                bufany.amount,
                dao.to_string(),
                bufany.old,
                bufany.new,
                fraction,
                possible.clone(),
            )?)
        }
    }
}

pub fn parse_redelegate_msg_bufany(msg: Binary) -> ParseRedelegationSubmissionResponse {
    let deserialized = Bufany::deserialize(&msg).unwrap();
    // staking msg coin proto = 4  // https://github.com/cosmos/cosmos-sdk/blob/v0.47.8/proto/cosmos/staking/v1beta1/tx.proto#L121
    let delegator = deserialized.string(1).unwrap(); // we can add validation 
    let old_val = deserialized.string(2).unwrap();
    let new_val = deserialized.string(3).unwrap();
    let coin_bytes = deserialized.bytes(4).unwrap();
    let amount = get_coin_from_bytes(coin_bytes);

    ParseRedelegationSubmissionResponse {
        amount,
        old: old_val,
        new: new_val,
    }
}

pub fn parse_delegate_msg_bufany(msg: Binary) -> ParseStakingSubmissionResponse {
    let deserialized = Bufany::deserialize(&msg).unwrap();
    // staking msg coin proto = 3  //https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/staking/v1beta1/tx.proto#L104
    let recipient = deserialized.string(2).unwrap();
    let coin_bytes = deserialized.bytes(3).unwrap();
    let bufany_coin_bytes = Bufany::deserialize(&coin_bytes).unwrap();
    let amount = coin(
        u128::from_str_radix(&bufany_coin_bytes.string(2).clone().unwrap(), 10).unwrap(),
        bufany_coin_bytes.string(1).clone().unwrap(),
    );

    ParseStakingSubmissionResponse { amount, recipient }
}

pub fn encode_delegate_msg_anybuf(
    deps: Deps,
    anybuf: Anybuf,
    coin: Coin,
    sender: String,
    validator: String,
    fraction: Decimal,
    possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    // staking msg coin proto = 3  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/bank/v1beta1/tx.proto#L48

    let amount = coin
        .amount
        .checked_mul_floor(fraction)
        .map_err(|x| StdError::generic_err(x.to_string()))?;

    let token = Anybuf::new().append_string(1, coin.denom).append_string(
        2,
        amount.to_string(), // applies the gauge calculation to each token sent
    );

    let proto = anybuf
        .append_string(1, sender)
        .append_string(2, validator) // sets the recipient as value in submission msg
        .append_message(3, &token)
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmos.staking.v1beta1.MsgDelegate".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}

pub fn encode_redelegate_msg_anybuf(
    deps: Deps,
    anybuf: Anybuf,
    coin: Coin,
    sender: String,
    old_val: String,
    new_val: String,
    fraction: Decimal,
    possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    // staking msg coin proto = 3  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/bank/v1beta1/tx.proto#L48

    let amount = coin
        .amount
        .checked_mul_floor(fraction)
        .map_err(|x| StdError::generic_err(x.to_string()))?;

    let token = Anybuf::new().append_string(1, coin.denom).append_string(
        2,
        amount.to_string(), // applies the gauge calculation to each token sent
    );

    let proto = anybuf
        .append_string(1, sender) // DAO
        .append_string(2, old_val) // val delegating from
        .append_message(3, &token) // val delegating to
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmos.staking.v1beta1.MsgReDelegate".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}
