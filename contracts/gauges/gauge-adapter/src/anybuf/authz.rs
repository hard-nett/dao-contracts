use crate::msg::{AdapterAuthzMsg, PossibleMsg, SubmissionMsg};
use anybuf::{Anybuf, Bufany};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, CosmosMsg, Decimal, Deps, Empty, StdResult};

#[cw_serde]
pub struct ParseAuthzGrantSubmissionResponse {
    pub granter: String,
    pub grantee: String,
}
#[cw_serde]
pub struct ParseAuthzExecSubmissionResponse {
    pub grantee: String,
    pub messages: Vec<Vec<u8>>,
}

pub fn parse_stargate_wire_authz(
    _deps: Deps,
    anybuf: Anybuf,
    dao: Addr,
    msg: SubmissionMsg,
    authz_msg: AdapterAuthzMsg,
    fraction: Decimal,
    possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    match authz_msg {
        AdapterAuthzMsg::MsgExec() => {
            // get amount from binaryMsg
            let bufany: ParseAuthzExecSubmissionResponse = parse_authz_msg_bufany(msg.msg);
            Ok(encode_authz_exec_msg_anybuf(
                anybuf,
                dao.to_string(),
                bufany.messages,
                fraction.clone(),
                possible.clone(),
            )?)
        }
        AdapterAuthzMsg::MsgGrant() => {
            let bufany = parse_authz_grant_msg_bufany(msg.msg);

            Ok(encode_authz_grant_msg_anybuf(
                anybuf,
                dao.to_string(),
                bufany.grantee,
            )?)
        }
        AdapterAuthzMsg::MsgRevoke() => todo!(),
    }
}

pub fn parse_authz_msg_bufany(msg: Binary) -> ParseAuthzExecSubmissionResponse {
    let deserialized = Bufany::deserialize(&msg).unwrap();
    let grantee = deserialized.string(1).unwrap();
    let messages = deserialized.repeated_bytes(2).unwrap();
    ParseAuthzExecSubmissionResponse { grantee, messages }
}
pub fn parse_authz_grant_msg_bufany(msg: Binary) -> ParseAuthzGrantSubmissionResponse {
    let deserialized = Bufany::deserialize(&msg).unwrap();
    // authz  grant msg coin proto = n/a  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/authz/v1beta1/tx.proto#L37
    let granter = deserialized.string(1).unwrap();
    let grantee = deserialized.string(2).unwrap();
    ParseAuthzGrantSubmissionResponse { grantee, granter }
}

///// EXECUTE MSGS AS GRANTEE ////
pub fn encode_authz_exec_msg_anybuf(
    anybuf: Anybuf,
    grantee: String,
    msgs: Vec<Vec<u8>>,
    _fraction: Decimal,
    _possible: Vec<PossibleMsg>,
) -> StdResult<CosmosMsg> {
    // authz exec msg coin proto = n/a  // https://github.com/cosmos/cosmos-sdk/blob/v0.50.7/proto/cosmos/authz/v1beta1/tx.proto#L53
    let proto = anybuf
        .append_string(1, grantee)
        .append_repeated_bytes(2, &msgs)
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmos.authz.v1beta1.MsgExec".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}

///// CREATE GRANTEE /////
pub fn encode_authz_grant_msg_anybuf(
    anybuf: Anybuf,
    granter: String,
    grantee: String,
) -> StdResult<CosmosMsg> {
    let proto = anybuf
        .append_string(1, granter)
        .append_string(2, grantee)
        .into_vec();

    let msg: CosmosMsg<Empty> = CosmosMsg::Stargate {
        type_url: "/cosmos.authz.v1beta1.MsgGrant".to_string(),
        value: proto.into(),
    };
    Ok(msg)
}
