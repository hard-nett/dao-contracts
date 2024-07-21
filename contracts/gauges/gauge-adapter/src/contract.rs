#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, from_json, to_json_binary, Addr, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Empty, Env,
    MessageInfo, Order, Response, StdResult, Storage, Uint128,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_denom::UncheckedDenom;
use cw_utils::{one_coin, PaymentError};

use crate::{
    error::ContractError,
    msg::{
        AdapterCw20Msgs, AdapterQueryMsg, AssetUnchecked, ExecuteMsg, InstantiateMsg, MigrateMsg,
        ReceiveMsg, StargateWire, SubmissionMsg,
    },
    state::{Config, Submission, CONFIG, POSSIBLE_MESSAGES, SUBMISSIONS},
};

// Version info for migration info.
const CONTRACT_NAME: &str = "crates.io:gauge-adapter-single";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let denom = msg.reward.denom.clone();
    let amount = msg.reward.amount.clone();
    let treasury = deps.api.addr_validate(&msg.treasury)?;

    initialize_submissions(
        deps.storage,
        env.contract.address,
        treasury.clone(),
        denom.clone(),
        amount.clone(),
    )?;

    let config = Config {
        admin: deps.api.addr_validate(&msg.admin)?,
        required_deposit: msg
            .required_deposit
            .map(|x| x.into_checked(deps.as_ref()))
            .transpose()?,
        treasury: treasury.clone(),
        possible_msg: msg.possible_msgs,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}

/// sets up contracts internal state for possible msgs to be permitted for submission msgs
fn initialize_submissions(
    store: &mut dyn Storage,
    adapter: Addr,
    treasury: Addr,
    denom: UncheckedDenom,
    amount: Uint128,
) -> Result<(), ContractError> {
    let sub_msg: SubmissionMsg = match denom {
        UncheckedDenom::Native(d) => SubmissionMsg {
            stargate: StargateWire::Bank(crate::msg::AdapterBankMsg::MsgSend()),
            msg: to_json_binary(&CosmosMsg::<Empty>::Bank(BankMsg::Send {
                to_address: treasury.to_string(),
                amount: coins(amount.into(), d),
            }))?,
        },
        UncheckedDenom::Cw20(c) => SubmissionMsg {
            stargate: StargateWire::Wasm(crate::msg::AdapterWasmMsg::Cw20(
                AdapterCw20Msgs::Transfer(),
            )),
            msg: to_json_binary(&CosmosMsg::<Empty>::Wasm(cosmwasm_std::WasmMsg::Execute {
                contract_addr: c,
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: treasury.to_string(),
                    amount: amount.clone(),
                })?,
                funds: vec![],
            }))?,
        },
    };

    SUBMISSIONS.save(
        store,
        treasury.clone(),
        &Submission {
            sender: adapter,
            name: "Unimpressed".to_owned(),
            url: "Those funds go back to the community pool".to_owned(),
            msg: sub_msg,
        },
    )?;
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20_message(deps, info, msg),
        ExecuteMsg::CreateSubmission {
            name,
            url,
            address,
            message,
        } => {
            let received = match one_coin(&info) {
                Ok(coin) => Ok(Some(coin)),
                Err(PaymentError::NoFunds {}) => Ok(None),
                Err(error) => Err(error),
            }?
            .map(|x| AssetUnchecked {
                denom: UncheckedDenom::Native(x.denom),
                amount: x.amount,
            });

            execute::create_submission(deps, info.sender, name, url, address, received, message)
        }
        ExecuteMsg::ReturnDeposits {} => execute::return_deposits(deps, info.sender),
    }
}

fn receive_cw20_message(
    deps: DepsMut,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_json(&msg.msg)? {
        ReceiveMsg::CreateSubmission {
            name,
            url,
            address,
            message,
        } => execute::create_submission(
            deps,
            Addr::unchecked(msg.sender),
            name,
            url,
            address,
            Some(AssetUnchecked::new_cw20(
                info.sender.as_str(),
                msg.amount.u128(),
            )),
            message,
        ),
    }
}

pub mod execute {
    use crate::msg::SubmissionMsg;

    use super::*;

    use cosmwasm_std::{ensure_eq, CosmosMsg};

    pub fn create_submission(
        deps: DepsMut,
        sender: Addr,
        name: String,
        url: String,
        address: String,
        received: Option<AssetUnchecked>,
        msg: SubmissionMsg,
    ) -> Result<Response, ContractError> {
        let address = deps.api.addr_validate(&address)?;

        let Config {
            required_deposit,
            treasury: _,
            admin: _,
            possible_msg,
        } = CONFIG.load(deps.storage)?;
        if let Some(required_deposit) = required_deposit {
            if let Some(received) = received {
                let received_denom = received.denom.into_checked(deps.as_ref())?;

                if required_deposit.denom != received_denom {
                    return Err(ContractError::InvalidDepositType {});
                }
                if received.amount != required_deposit.amount {
                    return Err(ContractError::InvalidDepositAmount {
                        correct_amount: required_deposit.amount,
                    });
                }
            } else {
                return Err(ContractError::PaymentError(PaymentError::NoFunds {}));
            }
        } else if let Some(received) = received {
            // If no deposit is required, then any deposit invalidates a submission.
            if !received.amount.is_zero() {
                return Err(ContractError::InvalidDepositAmount {
                    correct_amount: Uint128::zero(),
                });
            }
        }

        // allow to overwrite submission by the same author
        if let Some(old_submission) = SUBMISSIONS.may_load(deps.storage, address.clone())? {
            if old_submission.sender != sender {
                return Err(ContractError::UnauthorizedSubmission {});
            }
        }

        // confirm msg is one of possible_msgs
        // let pos = POSSIBLE_MESSAGES.load(deps.storage)?;

        if possible_msg
            .into_iter()
            .find(|c| c.stargate == msg.stargate)
            .is_none()
        {
            return Err(ContractError::IncorrectMessage {});
        }

        SUBMISSIONS.save(
            deps.storage,
            address,
            &Submission {
                sender,
                name,
                url,
                msg,
            },
        )?;
        Ok(Response::new().add_attribute("create", "submission"))
    }

    pub fn return_deposits(deps: DepsMut, sender: Addr) -> Result<Response, ContractError> {
        let Config {
            admin,
            required_deposit,
            treasury: _,
            possible_msg,
        } = CONFIG.load(deps.storage)?;

        // No refund if no deposit was required.
        let required_deposit = required_deposit.ok_or(ContractError::NoDepositToRefund {})?;

        ensure_eq!(sender, admin, ContractError::Unauthorized {});

        let msgs = SUBMISSIONS
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| {
                let (_submission_recipient, submission) = item?;

                required_deposit
                    .denom
                    .get_transfer_to_message(&submission.sender, required_deposit.amount)
            })
            .collect::<StdResult<Vec<CosmosMsg>>>()?;

        Ok(Response::new().add_messages(msgs))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: AdapterQueryMsg) -> StdResult<Binary> {
    match msg {
        AdapterQueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
        AdapterQueryMsg::AllOptions {} => to_json_binary(&query::all_options(deps)?),
        AdapterQueryMsg::CheckOption { option } => {
            to_json_binary(&query::check_option(deps, option)?)
        }
        AdapterQueryMsg::SampleGaugeMsgs { selected } => {
            to_json_binary(&query::sample_gauge_msgs(deps, selected)?)
        }
        AdapterQueryMsg::Submission { address } => {
            to_json_binary(&query::submission(deps, address)?)
        }
        AdapterQueryMsg::AllSubmissions {} => {
            to_json_binary(&query::all_submissions(deps)?.submissions)
        }
        AdapterQueryMsg::AvailableMessages {} => {
            to_json_binary(&CONFIG.load(deps.storage)?.possible_msg)
        }
    }
}

mod query {
    use cosmwasm_std::{CosmosMsg, Decimal};

    use crate::{
        msg::{
            AllOptionsResponse, AllSubmissionsResponse, CheckOptionResponse,
            SampleGaugeMsgsResponse, SubmissionResponse,
        },
        stargate_to_anybuf,
    };

    use super::*;

    pub fn all_options(deps: Deps) -> StdResult<AllOptionsResponse> {
        Ok(AllOptionsResponse {
            options: SUBMISSIONS
                .keys(deps.storage, None, None, Order::Ascending)
                .map(|key| Ok(key?.to_string()))
                .collect::<StdResult<Vec<String>>>()?,
        })
    }

    pub fn check_option(deps: Deps, option: String) -> StdResult<CheckOptionResponse> {
        Ok(CheckOptionResponse {
            valid: SUBMISSIONS.has(deps.storage, deps.api.addr_validate(&option)?),
        })
    }

    pub fn sample_gauge_msgs(
        deps: Deps,
        winners: Vec<(String, Decimal)>,
    ) -> StdResult<SampleGaugeMsgsResponse> {
        let execute = winners
            .into_iter()
            .map(|(winner, fraction)| {
                // Gauge already sends chosen tally to this query by using results we send in
                // all_options query; they are already validated
                stargate_to_anybuf(deps, deps.api.addr_validate(&winner)?, fraction)
            })
            .collect::<StdResult<Vec<CosmosMsg>>>()?;
        Ok(SampleGaugeMsgsResponse { execute })
    }

    pub fn submission(deps: Deps, address: String) -> StdResult<SubmissionResponse> {
        let address = deps.api.addr_validate(&address)?;
        let submission = SUBMISSIONS.load(deps.storage, address.clone())?;
        Ok(SubmissionResponse {
            sender: submission.sender,
            name: submission.name,
            url: submission.url,
            address,
        })
    }

    pub fn all_submissions(deps: Deps) -> StdResult<AllSubmissionsResponse> {
        Ok(AllSubmissionsResponse {
            submissions: SUBMISSIONS
                .range(deps.storage, None, None, Order::Ascending)
                .map(|s| {
                    let (address, submission) = s?;
                    Ok(SubmissionResponse {
                        sender: submission.sender,
                        name: submission.name,
                        url: submission.url,
                        address,
                    })
                })
                .collect::<StdResult<Vec<SubmissionResponse>>>()?,
        })
    }
}

/// Manages the contract migration.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::{
        coins,
        testing::{mock_dependencies, mock_env, mock_info},
        BankMsg, CosmosMsg, Decimal, Uint128, WasmMsg,
    };
    use cw20::Cw20ExecuteMsg;
    use cw_denom::CheckedDenom;

    use crate::{
        msg::{AdapterBankMsg, AssetUnchecked, PossibleMsg},
        state::Asset,
    };

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: "admin".to_owned(),
            required_deposit: Some(AssetUnchecked::new_cw20("juno", 10_000_000)),
            treasury: "treasury".to_owned(),
            reward: AssetUnchecked::new_native("ujuno", 150_000_000_000),
            possible_msgs: vec![PossibleMsg {
                stargate: StargateWire::Bank(AdapterBankMsg::MsgSend()),
                max_amount: None,
            }],
        };
        instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &[]),
            msg.clone(),
        )
        .unwrap();

        // Check if the config is stored.
        let config = CONFIG.load(deps.as_ref().storage).unwrap();
        assert_eq!(config.admin, Addr::unchecked("admin"));
        assert_eq!(
            config.required_deposit,
            Some(Asset {
                denom: CheckedDenom::Cw20(Addr::unchecked("juno")),
                amount: Uint128::new(10_000_000)
            })
        );
        assert_eq!(config.treasury, "treasury".to_owned());

        let msg = InstantiateMsg {
            reward: AssetUnchecked::new_native("ujuno", 10_000_000),
            ..msg
        };
        instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &[]),
            msg.clone(),
        )
        .unwrap();

        let msg = InstantiateMsg {
            required_deposit: None,
            ..msg
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("user", &[]), msg).unwrap();
        let config = CONFIG.load(deps.as_ref().storage).unwrap();
        assert_eq!(config.required_deposit, None);
    }

    #[test]
    fn sample_gauge_msgs_native() {
        let mut deps = mock_dependencies();

        let reward = Uint128::new(150_000_000_000);
        let msg = InstantiateMsg {
            admin: "admin".to_owned(),
            required_deposit: Some(AssetUnchecked::new_cw20("juno", 10_000_000)),
            treasury: "treasury".to_owned(),
            reward: AssetUnchecked::new_native("ujuno", reward.into()),
            possible_msgs: vec![PossibleMsg {
                stargate: StargateWire::Bank(AdapterBankMsg::MsgSend()),
                max_amount: None,
            }],
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("user", &[]), msg).unwrap();

        let selected = vec![
            (
                "juno1t8ehvswxjfn3ejzkjtntcyrqwvmvuknzy3ajxy".to_string(),
                Decimal::percent(41),
            ),
            (
                "juno196ax4vc0lwpxndu9dyhvca7jhxp70rmcl99tyh".to_string(),
                Decimal::percent(33),
            ),
            (
                "juno1y0us8xvsvfvqkk9c6nt5cfyu5au5tww23dmh40".to_string(),
                Decimal::percent(26),
            ),
        ];
        let res = query::sample_gauge_msgs(deps.as_ref(), selected).unwrap();
        assert_eq!(res.execute.len(), 3);
        assert_eq!(
            res.execute,
            [
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: "juno1t8ehvswxjfn3ejzkjtntcyrqwvmvuknzy3ajxy".to_string(),
                    amount: coins((reward * Decimal::percent(41)).u128(), "ujuno")
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: "juno196ax4vc0lwpxndu9dyhvca7jhxp70rmcl99tyh".to_string(),
                    amount: coins((reward * Decimal::percent(33)).u128(), "ujuno")
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: "juno1y0us8xvsvfvqkk9c6nt5cfyu5au5tww23dmh40".to_string(),
                    amount: coins((reward * Decimal::percent(26)).u128(), "ujuno")
                }),
            ]
        );
    }

    #[test]
    fn sample_gauge_msgs_cw20() {
        let mut deps = mock_dependencies();

        let reward = Uint128::new(150_000_000_000);
        let msg = InstantiateMsg {
            admin: "admin".to_owned(),
            required_deposit: Some(AssetUnchecked::new_cw20("juno", 10_000_000)),
            treasury: "treasury".to_owned(),
            reward: AssetUnchecked::new_cw20("juno", reward.into()),
            possible_msgs: vec![PossibleMsg {
                stargate: StargateWire::Bank(AdapterBankMsg::MsgSend()),
                max_amount: None,
            }],
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("user", &[]), msg).unwrap();

        let selected = vec![
            (
                "juno1t8ehvswxjfn3ejzkjtntcyrqwvmvuknzy3ajxy".to_string(),
                Decimal::percent(41),
            ),
            (
                "juno196ax4vc0lwpxndu9dyhvca7jhxp70rmcl99tyh".to_string(),
                Decimal::percent(33),
            ),
            (
                "juno1y0us8xvsvfvqkk9c6nt5cfyu5au5tww23dmh40".to_string(),
                Decimal::percent(26),
            ),
        ];
        let res = query::sample_gauge_msgs(deps.as_ref(), selected).unwrap();
        assert_eq!(res.execute.len(), 3);
        assert_eq!(
            res.execute,
            [
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "juno".to_owned(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "juno1t8ehvswxjfn3ejzkjtntcyrqwvmvuknzy3ajxy".to_string(),
                        amount: reward * Decimal::percent(41)
                    })
                    .unwrap(),
                    funds: vec![]
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "juno".to_owned(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "juno196ax4vc0lwpxndu9dyhvca7jhxp70rmcl99tyh".to_string(),
                        amount: reward * Decimal::percent(33)
                    })
                    .unwrap(),
                    funds: vec![]
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "juno".to_owned(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "juno1y0us8xvsvfvqkk9c6nt5cfyu5au5tww23dmh40".to_string(),
                        amount: reward * Decimal::percent(26)
                    })
                    .unwrap(),
                    funds: vec![]
                }),
            ]
        );
    }

    #[test]
    fn return_deposits_authorization() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: "admin".to_owned(),
            required_deposit: None,
            treasury: "treasury".to_owned(),
            reward: AssetUnchecked::new_native("ujuno", 150_000_000_000),
            possible_msgs: vec![PossibleMsg {
                stargate: StargateWire::Bank(AdapterBankMsg::MsgSend()),
                max_amount: None,
            }],
        };
        instantiate(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &[]),
            msg.clone(),
        )
        .unwrap();

        let err = execute::return_deposits(deps.as_mut(), Addr::unchecked("user")).unwrap_err();
        assert_eq!(err, ContractError::NoDepositToRefund {});

        let msg = InstantiateMsg {
            required_deposit: Some(AssetUnchecked::new_native("ujuno", 10_000_000)),
            ..msg
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("user", &[]), msg).unwrap();

        let err = execute::return_deposits(deps.as_mut(), Addr::unchecked("user")).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }
}
