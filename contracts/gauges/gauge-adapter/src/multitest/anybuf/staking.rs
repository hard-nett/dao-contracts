use anybuf::Anybuf;
use cosmwasm_std::{coin, coins, to_json_binary};
use cw_denom::UncheckedDenom;
use cw_orch::{mock::MockBech32, prelude::*};

use crate::{
    msg::{
        AdapterAuthzMsg, AdapterQueryMsg, AdapterQueryMsgFns, AdapterStakingMsg,
        AllOptionsResponse, AssetUnchecked, CheckOptionResponse, PossibleMsg, StargateWire,
        SubmissionMsg,
    },
    multitest::suite::{native_submission_helper, setup_gauge_adapter},
};

#[test]
fn test_staking_anybuf_assertions() {
    let mock = MockBech32::new("mock");
    let adapter = setup_gauge_adapter(
        mock.clone(),
        Some(AssetUnchecked {
            denom: UncheckedDenom::Native("juno".into()),
            amount: 1_000u128.into(),
        }),
        Some(vec![
            PossibleMsg {
                stargate: StargateWire::Staking(AdapterStakingMsg::MsgDelegate()),
                max_amount: Some(1_000u128.into()),
            },
            PossibleMsg {
                stargate: StargateWire::Staking(AdapterStakingMsg::MsgRedelegate()),
                max_amount: Some(1_000u128.into()),
            },
        ]),
    );

    // verify there are 4 possible messages
    assert_eq!(adapter.available_messages().unwrap().len(), 4);

    // submit invalid submission msg.
    let grantee = mock.addr_make("grantee");
    native_submission_helper(
        adapter.clone(),
        mock.sender.clone(),
        mock.sender.clone(),
        Some(coin(1_000u128, "juno")),
        SubmissionMsg {
            stargate: StargateWire::Authz(AdapterAuthzMsg::MsgGrant()),
            msg: to_json_binary(
                &Anybuf::new()
                    .append_string(1, mock.sender.to_string())
                    .append_string(2, grantee.to_string())
                    .into_vec(),
            )
            .unwrap(),
        },
    )
    .unwrap_err();

    //  good delegate submission
    let einstein = mock
        .addr_make_with_balance("einstein", coins(1_000u128, "juno"))
        .unwrap();
    native_submission_helper(
        adapter.clone(),
        einstein.clone(),
        einstein.clone(),
        Some(coin(1_000u128, "juno")),
        SubmissionMsg {
            stargate: crate::msg::StargateWire::Staking(AdapterStakingMsg::MsgDelegate()),
            msg: to_json_binary(
                &Anybuf::new()
                    .append_string(1, einstein.to_string())
                    .append_message(
                        2,
                        &Anybuf::new()
                            .append_string(1, "juno".to_string())
                            .append_string(2, "1000".to_string()),
                    )
                    .into_vec(),
            )
            .unwrap(),
        },
    )
    .unwrap();

    //  good redelegate submission
    let newton = mock
        .addr_make_with_balance("newton", coins(1_000u128, "juno"))
        .unwrap();
    native_submission_helper(
        adapter.clone(),
        newton.clone(),
        newton.clone(),
        Some(coin(1_000u128, "juno")),
        SubmissionMsg {
            stargate: crate::msg::StargateWire::Staking(AdapterStakingMsg::MsgRedelegate()),
            msg: to_json_binary(
                &Anybuf::new()
                    .append_string(1, newton.to_string())
                    .append_message(
                        2,
                        &Anybuf::new()
                            .append_string(1, newton.to_string())
                            .append_string(2, mock.addr_make("oldval"))
                            .append_string(3, mock.addr_make("newval"))
                            .append_message(
                                4,
                                &Anybuf::new()
                                    .append_string(1, "juno")
                                    .append_string(2, "1000".to_string()),
                            ),
                    )
                    .into_vec(),
            )
            .unwrap(),
        },
    )
    .unwrap();

    let options: AllOptionsResponse = adapter.query(&AdapterQueryMsg::AllOptions {}).unwrap();

    println!("{:#?}", options);
    assert_eq!(
        options,
        AllOptionsResponse {
            options: vec![
                einstein.to_string(),
                newton.to_string(),
                mock.addr_make("treasury").to_string(),
            ]
        },
    );

    let option: CheckOptionResponse = adapter
        .query(&AdapterQueryMsg::CheckOption {
            option: einstein.to_string(),
        })
        .unwrap();
    assert!(option.valid);

    let option: CheckOptionResponse = adapter
        .query(&AdapterQueryMsg::CheckOption {
            option: mock.sender.to_string(),
        })
        .unwrap();
    assert!(!option.valid);
}
