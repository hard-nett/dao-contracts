use anybuf::Anybuf;
use cosmwasm_std::{coin, coins, to_json_binary};
use cw_denom::UncheckedDenom;
use cw_orch::{mock::MockBech32, prelude::*};

use crate::{
    msg::{
        AdapterAuthzMsg, AdapterDistributionMsg, AdapterQueryMsg, AdapterQueryMsgFns,
        AllOptionsResponse, AssetUnchecked, CheckOptionResponse, PossibleMsg, StargateWire,
        SubmissionMsg,
    },
    multitest::suite::{native_submission_helper, setup_gauge_adapter},
};

#[test]
fn test_distribution_anybuf_assertions() {
    let mock = MockBech32::new("mock");
    let adapter = setup_gauge_adapter(
        mock.clone(),
        Some(AssetUnchecked {
            denom: UncheckedDenom::Native("juno".into()),
            amount: 1_000u128.into(),
        }),
        Some(vec![PossibleMsg {
            stargate: StargateWire::Distribution(AdapterDistributionMsg::MsgFundCommunityPool()),
            max_amount: Some(1_000u128.into()),
        }]),
    );

    // verify there are 3 possible messages
    assert_eq!(adapter.available_messages().unwrap().len(), 3);

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
            stargate: crate::msg::StargateWire::Distribution(
                AdapterDistributionMsg::MsgFundCommunityPool(),
            ),
            msg: to_json_binary(
                &Anybuf::new()
                    .append_repeated_message(
                        1,
                        &vec![&Anybuf::new()
                            .append_string(1, "juno".to_string())
                            .append_string(2, "1000".to_string())],
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
            options: vec![einstein.to_string(), mock.addr_make("treasury").to_string(),]
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
