use abstract_cw_plus_interface::cw20_base::Cw20Base;
use cosmwasm_std::{coins, Addr, Coin, Uint128};
use cw_orch::{interface, mock::cw_multi_test::AppResponse, prelude::*};
use cw_orch_core::CwEnvError;

use abstract_cw20::{Cw20Coin as AbsCw20Coin, MinterResponse};

use crate::{
    contract::{execute, instantiate, migrate, query},
    msg::{
        AdapterBankMsg, AdapterCw20Msgs, AdapterQueryMsg as QueryMsg, AdapterWasmMsg,
        AssetUnchecked, ExecuteMsg, InstantiateMsg, MigrateMsg, PossibleMsg, StargateWire,
        SubmissionMsg,
    },
};

// Store the marketing gauge adapter contract and returns the code id.
#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
pub struct GaugeAdapter;

impl<Chain> Uploadable for GaugeAdapter<Chain> {
    /// Return the path to the wasm file corresponding to the contract
    fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
        artifacts_dir_from_workspace!()
            .find_wasm_path("gauge_adapter")
            .unwrap()
    }
    /// Returns a CosmWasm contract wrapper
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query).with_migrate(migrate))
    }
}

pub fn setup_gauge_adapter(
    mock: MockBech32,
    required_deposit: Option<AssetUnchecked>,
    possible_msgs: Option<Vec<PossibleMsg>>,
) -> GaugeAdapter<MockBech32> {
    mock.add_balance(&mock.sender, coins(1_000_000, "juno"))
        .unwrap();
    let mut msgs = vec![
        PossibleMsg {
            stargate: StargateWire::Bank(AdapterBankMsg::MsgSend()),
            max_amount: Some(Uint128::from(1_000u128)),
        },
        PossibleMsg {
            stargate: StargateWire::Wasm(AdapterWasmMsg::Cw20(AdapterCw20Msgs::Send())),
            max_amount: Some(Uint128::from(1_000u128)),
        },
    ];
    if let Some(msg) = possible_msgs {
        msgs.extend(msg);
    }

    let adapter = GaugeAdapter::new("gauge_adapter", mock.clone());
    adapter.upload().unwrap();

    let instantiate = InstantiateMsg {
        admin: mock.sender_addr().to_string(),
        required_deposit,
        reward: AssetUnchecked::new_native("juno", 1_000_000),
        treasury: mock.addr_make("treasury").to_string(),
        possible_msgs: msgs,
    };
    adapter.instantiate(&instantiate, None, None).unwrap();
    adapter
}

//
pub fn native_submission_helper(
    adapter: GaugeAdapter<MockBech32>,
    sender: Addr,
    recipient: Addr,
    native_tokens: Option<Coin>,
    msg: SubmissionMsg,
) -> Result<AppResponse, CwEnvError> {
    if let Some(assets) = native_tokens.clone() {
        let res = adapter.call_as(&sender).execute(
            &crate::msg::ExecuteMsg::CreateSubmission {
                name: "DAOers".to_string(),
                url: "https://daodao.zone".to_string(),
                address: recipient.to_string(),
                message: msg.clone(),
            },
            Some(&[assets]),
        );
        res
    } else {
        let res = adapter.call_as(&sender).execute(
            &crate::msg::ExecuteMsg::CreateSubmission {
                name: "DAOers".to_string(),
                url: "https://daodao.zone".to_string(),
                address: recipient.to_string(),
                message: msg,
            },
            None,
        );
        res
    }
}

pub fn cw20_helper(mock: MockBech32) -> Cw20Base<MockBech32> {
    let cw20 = Cw20Base::new("cw20", mock.clone());
    cw20.upload().unwrap();
    init_cw20(cw20.clone(), mock.sender.to_string());
    cw20
}

pub fn init_cw20(cw20: Cw20Base<MockBech32>, minter: String) -> String {
    let init_msg = abstract_cw20_base::msg::InstantiateMsg {
        name: "test".to_string(),
        symbol: "TEST".to_string(),
        decimals: 6u8,
        initial_balances: vec![AbsCw20Coin {
            address: minter.clone(),
            amount: Uint128::from(1_000_000u128),
        }],
        mint: Some(MinterResponse { minter, cap: None }),
        marketing: None,
    };
    cw20.instantiate(&init_msg, None, None).unwrap();
    let addr = cw20.address().unwrap();
    println!("correct cw20 addr: {:#?}", addr.clone());
    addr.to_string()
}
