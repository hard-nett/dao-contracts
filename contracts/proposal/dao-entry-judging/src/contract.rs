[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Reply, Response, StdResult,
    Storage, SubMsg, WasmMsg,
};

use cw2::set_contract_version;
use cw_hooks::Hooks;
use cw_storage_plus::Bound;
use cw_utils::{parse_reply_instantiate_data, Duration};
use dao_interface::voting::IsActiveResponse;
use dao_pre_entry_judging::contract::ExecuteMsg as PreProposeMsg;
use dao_proposal_hooks::{new_proposal_hooks, proposal_status_changed_hooks};
use dao_vote_hooks::new_vote_hooks;
use dao_voting::{
    entry_judging::{
        EntryJudgingOptions, EntryJudgingVote, EntryJudgingVotes, VotingStrategy,
    },
    pre_propose::{PreProposeInfo, ProposalCreationPolicy},
    proposal::{DEFAULT_LIMIT, MAX_PROPOSAL_SIZE},
    reply::{
        failed_pre_propose_module_hook_id, mask_proposal_execution_proposal_id, TaggedReplyId,
    },
    status::Status,
    voting::{get_total_power, get_voting_power, validate_voting_period},
};

use crate::{msg::MigrateMsg, state::CREATION_POLICY};
use crate::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    proposal::{EntryJudgingProposal, VoteResult},
    query::{ProposalListResponse, ProposalResponse, VoteInfo, VoteListResponse, VoteResponse},
    state::{
        Ballot, Config, BALLOTS, CONFIG, PROPOSALS, PROPOSAL_COUNT, PROPOSAL_HOOKS, VOTE_HOOKS,
    },
    ContractError,
};


pub const CONTRACT_NAME: &str = "crates.io:dao-entry-judging";
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Instantiate Contract
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    msg.voting_strategy.validate()?;

    let dao = info.sender;

    let (min_voting_period, max_voting_period) =
    validate_voting_period(msg.min_voting_period, msg.max_voting_period)?;

    // minimum & maximum voting range for each proposal choice
    let (min_voting_range, max_voting_range) =
    validate_voting_range(msg.min_voting_range, msg.max_voting_range)?;

    let (initial_policy, pre_propose_messages) = msg
        .pre_propose_info
        .into_initial_policy_and_messages(dao.clone())?;

    // configuration for entry judging contract
    let config = Config {
        voting_strategy: msg.voting_strategy,
        min_voting_period,
        max_voting_period,
        min_voting_range,
        max_voting_range,
        only_admins_propose: msg.only_admins_propose,
        only_members_execute: msg.only_members_execute,
        allow_revoting: msg.allow_revoting,
        allow_updating_proposal: msg.allow_updating_propsoal,
        dao,
        close_proposal_on_execution_failure: msg.close_proposal_on_execution_failure,
    };

        // Initialize proposal count to zero so that queries return zero
    // instead of None.
    PROPOSAL_COUNT.save(deps.storage, &0)?;
    CONFIG.save(deps.storage, &config)?;
    CREATION_POLICY.save(deps.storage, &initial_policy)?;

    Ok(Response::default()
    .add_submessages(pre_propose_messages)
    .add_attribute("action", "instantiate")
    .add_attribute("dao", config.dao))
}