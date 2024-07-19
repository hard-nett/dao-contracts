use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary, CosmosMsg, Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_denom::UncheckedDenom;

#[cw_serde]
pub struct InstantiateMsg {
    /// Address that is allowed to return deposits.
    pub admin: String,
    /// Deposit required for valid submission. This option allows to reduce spam.
    pub required_deposit: Option<AssetUnchecked>,
    /// Address of contract where each deposit is transferred.
    pub treasury: String,
    /// Total reward amount.
    pub reward: AssetUnchecked,
    /// Possible messages submission can include.
    pub possible_msgs: Vec<PossibleMsg>,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Implements the Cw20 receiver interface.
    Receive(Cw20ReceiveMsg),
    /// Save info about team that wants to participate.
    /// Only for native tokens as required deposit.
    CreateSubmission {
        name: String,
        url: String,
        address: String,
        message: SubmissionMsg,
    },
    /// Sends back all deposit to senders.
    ReturnDeposits {},
}

#[cw_serde]
pub enum ReceiveMsg {
    /// Save info about team that wants to participate.
    /// Only for CW20 tokens as required deposit.
    CreateSubmission {
        name: String,
        url: String,
        address: String,
        message: SubmissionMsg,
    },
}

#[cw_serde]
pub enum MigrateMsg {}

// Queries copied from gauge-orchestrator for now (we could use a common crate for this).
/// Queries the gauge requires from the adapter contract in order to function.
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum AdapterQueryMsg {
    #[returns(crate::state::Config)]
    Config {},
    #[returns(AllOptionsResponse)]
    AllOptions {},
    #[returns(CheckOptionResponse)]
    CheckOption { option: String },
    #[returns(SampleGaugeMsgsResponse)]
    SampleGaugeMsgs {
        /// Option along with weight.
        /// Sum of all weights should be 1.0 (within rounding error).
        selected: Vec<(String, Decimal)>,
    },

    // Marketing-gauge specific queries to help on frontend
    #[returns(SubmissionResponse)]
    Submission { address: String },
    #[returns(AllSubmissionsResponse)]
    AllSubmissions {},
}

#[cw_serde]
pub struct AllOptionsResponse {
    pub options: Vec<String>,
}

#[cw_serde]
pub struct CheckOptionResponse {
    pub valid: bool,
}

#[cw_serde]
pub struct SampleGaugeMsgsResponse {
    pub execute: Vec<CosmosMsg>,
}

#[cw_serde]
pub struct SubmissionResponse {
    pub sender: Addr,
    pub name: String,
    pub url: String,
    pub address: Addr,
}

#[cw_serde]
pub struct AllSubmissionsResponse {
    pub submissions: Vec<SubmissionResponse>,
}

#[cw_serde]
pub struct AssetUnchecked {
    pub denom: UncheckedDenom,
    pub amount: Uint128,
}

#[cw_serde]
pub struct PossibleMsg {
    pub stargate: StargateWire,
    pub max_amount: Option<Uint128>,
}
#[cw_serde]
pub struct SubmissionMsg {
    pub stargate: StargateWire,
    pub msg: Binary,
}

#[cw_serde]
pub enum StargateWire {
    Bank(AdapterBankMsg),
    Distribution(AdapterDistributionMsg),
    // Gov(),
    // Ibc(),
    Staking(AdapterStakingMsg),
    Wasm(AdapterWasmMsg),
}

#[cw_serde]
pub enum AdapterBankMsg {
    // MsgBurn(),
    MsgSend(),
}
#[cw_serde]
pub enum AdapterDistributionMsg {
    MsgFundCommunityPool(),
}
#[cw_serde]
pub enum AdapterStakingMsg {
    MsgDelegate(),
    MsgRedelegate(),
}
#[cw_serde]
pub enum AdapterWasmMsg {
    Cw20(AdapterCw20Msgs),
}

#[cw_serde]
pub enum AdapterCw20Msgs {
    Transfer(),
    Send(),
    IncreaseAllowance(),
    DecreaseAllowance(),
    Mint(),
}
