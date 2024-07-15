use anybuf::{Anybuf, Bufany};
use cosmwasm_std::{coin, Binary, Coin, CosmosMsg, Decimal, Empty, StdError, StdResult};

pub fn parse_bank_submission_msg_bufany(msg: Binary) -> (Vec<Coin>, String) {
    let deserialized = Bufany::deserialize(&msg).unwrap();
    // bank msg coin proto = 3  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/bank/v1beta1/tx.proto#L48
    let coin_bytes = deserialized.repeated_bytes(3).unwrap();
    let to_addr = deserialized.string(2).unwrap();
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
    (coins, to_addr)
}

pub fn encode_bank_submission_msg_anybuf(
    anybuf: Anybuf,
    coins: Vec<Coin>,
    sender: String,
    recipient: String,
    fraction: Decimal,
) -> StdResult<CosmosMsg> {
    // bank msg coin proto = 3  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/bank/v1beta1/tx.proto#L48
    let mut anybuf_coins = vec![];

    for coin in coins {
        let token = Anybuf::new().append_string(1, coin.denom).append_string(
            2,
            coin.amount
                .checked_mul_floor(fraction)
                .map_err(|x| StdError::generic_err(x.to_string()))?
                .to_string(), // applies the gauge calculation to each token sent
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
