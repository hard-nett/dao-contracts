#[cfg(not(feature = "library"))]
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
    voting::{get_total_power, get_voting_power, validate_voting_period, validate_voting_range},
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


// Event-Flow
// This module is meant to have a single use lifecycle. 

// The circle of event-flow life-cycle:
//
// a. Instantiation - this is when the proposal module is configured, including the time desired for the interaction phase
// b. Interaction - this starts when an admin starts the proposal module. Once the module has been started,  this is when proposals can be made & voted on. 
// c. Completion - interaction with the module is complete, no more state modifications can be made unless migration approval from dao.
// d. Iteration  - daos can choose to create another module , used to recognize chronological reuse for the same event, similar to subDAO.

// The main function we desire this contract to provide is:
// 1. Handle config for entry-judging module parameters
// 2. tally of voting points given to each proposal choice available
// 3. calculate results via points of each proposal & their choices
// 
//   The goal here is to rely on the points given to each entry, so that by the time the module interaction 
//   timeline ends, we can calculate the proposals with the most votes,
//    by judges to be what determines the results of the proposal module winner.
//
//

//   *************** Creating The Judging Module *********************
//  - set min & max voting range
//  - set if only admins create proposals
//  - set if proposal metadata is mutable 
//  - set a max number of created proposals permitted per DAO member
//  
//
//
//    *************** Using The Judging Module *********************
//  - vote
//  - query_votes_by_choice: query the votes of a 
//
//
//  *************** Notable Judging Module Checks *********************
//  - check if judge has uploaded more entries than permitted
//  - 





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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Propose(ProposeMsg {
            title,
            description,
            msgs,
            proposer,
        }) => execute_propose(deps, env, info.sender, title, description, msgs, proposer),
        ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale,
        } => execute_vote(deps, env, info, proposal_id, vote, rationale),
        ExecuteMsg::UpdateRationale {
            proposal_id,
            rationale,
        } => execute_update_rationale(deps, info, proposal_id, rationale),
        ExecuteMsg::UpdateProposal {
            proposal_id,
            msgs,
            proposer,
        } => execute_update_rationale(deps, info, proposal_id, msgs,proposer),
        ExecuteMsg::Execute { proposal_id } => execute_execute(deps, env, info, proposal_id),
        ExecuteMsg::Close { proposal_id } => execute_close(deps, env, info, proposal_id),
        ExecuteMsg::UpdateConfig {
            threshold,
            max_voting_period,
            min_voting_period,
            max_voting_range,
            min_voting_range,
            only_admins_propose,
            only_members_execute,
            allow_revoting,
            allow_updating_proposal,
            dao,
            close_proposal_on_execution_failure,
        } => execute_update_config(
            deps,
            info,
            threshold,
            max_voting_period,
            min_voting_period,
            max_voting_range,
            min_voting_range,
            only_admins_propose,
            only_members_execute,
            allow_revoting,
            allow_updating_proposal,
            dao,
            close_proposal_on_execution_failure,
        ),
        ExecuteMsg::UpdatePreProposeInfo { info: new_info } => {
            execute_update_proposal_creation_policy(deps, info, new_info)
        }
        ExecuteMsg::AddProposalHook { address } => {
            execute_add_proposal_hook(deps, env, info, address)
        }
        ExecuteMsg::RemoveProposalHook { address } => {
            execute_remove_proposal_hook(deps, env, info, address)
        }
        ExecuteMsg::AddVoteHook { address } => execute_add_vote_hook(deps, env, info, address),
        ExecuteMsg::RemoveVoteHook { address } => {
            execute_remove_vote_hook(deps, env, info, address)
        }
    }
}

    pub fn execute_propose(
        deps: DepsMut,
        env: Env,
        sender: Addr,
        title: String,
        description: String,
        msgs: Vec<CosmosMsg<Empty>>,
        proposer: Option<String>,
    ) -> Result<Response, ContractError> {
        let config = CONFIG.load(deps.storage)?;
        let proposal_creation_policy = CREATION_POLICY.load(deps.storage)?;

         // Check that the sender is permitted to create proposals.
    if !proposal_creation_policy.is_permitted(&sender) {
        return Err(ContractError::Unauthorized {});
    }

         // TODO: Check that the sender has not created more proposals than permitted.


  // Determine the appropriate proposer. If this is coming from our
    // pre-propose module, it must be specified. Otherwise, the
    // proposer should not be specified.
    let proposer = match (proposer, &proposal_creation_policy) {
        (None, ProposalCreationPolicy::Anyone {}) => sender.clone(),
        // `is_permitted` above checks that an allowed module is
        // actually sending the propose message.
        (Some(proposer), ProposalCreationPolicy::Module { .. }) => {
            deps.api.addr_validate(&proposer)?
        }
        _ => return Err(ContractError::InvalidProposer {}),
    };

    let voting_module: Addr = deps.querier.query_wasm_smart(
        config.dao.clone(),
        &dao_core::msg::QueryMsg::VotingModule {},
    )?;

    // Voting modules are not required to implement this
    // query. Lacking an implementation they are active by default.
    let active_resp: IsActiveResponse = deps
        .querier
        .query_wasm_smart(voting_module, &dao_interface::voting::Query::IsActive {})
        .unwrap_or(IsActiveResponse { active: true });

    if !active_resp.active {
        return Err(ContractError::InactiveDao {});
    }

    let expiration = config.max_voting_period.after(&env.block);

    let total_power = get_total_power(deps.as_ref(), &config.dao, Some(env.block.height))?;

    let proposal = {
        // Limit mutability to this block.
        let mut proposal = SingleChoiceProposal {
            title,
            description,
            proposer: proposer.clone(),
            start_height: env.block.height,
            min_voting_period: config.min_voting_period.map(|min| min.after(&env.block)),
            expiration,
            threshold: config.threshold,
            total_power,
            msgs,
            status: Status::Open,
            votes: Votes::zero(),
            allow_revoting: config.allow_revoting,
        };
        // Update the proposal's status. Addresses case where proposal
        // expires on the same block as it is created.
        proposal.update_status(&env.block);
        proposal
    };
    let id = advance_proposal_id(deps.storage)?;

    // Limit the size of proposals.
    //
    // The Juno mainnet has a larger limit for data that can be
    // uploaded as part of an execute message than it does for data
    // that can be queried as part of a query. This means that without
    // this check it is possible to create a proposal that can not be
    // queried.
    //
    // The size selected was determined by uploading versions of this
    // contract to the Juno mainnet until queries worked within a
    // reasonable margin of error.
    //
    // `to_vec` is the method used by cosmwasm to convert a struct
    // into it's byte representation in storage.
    let proposal_size = cosmwasm_std::to_vec(&proposal)?.len() as u64;
    if proposal_size > MAX_PROPOSAL_SIZE {
        return Err(ContractError::ProposalTooLarge {
            size: proposal_size,
            max: MAX_PROPOSAL_SIZE,
        });
    }

    PROPOSALS.save(deps.storage, id, &proposal)?;

    let hooks = new_proposal_hooks(PROPOSAL_HOOKS, deps.storage, id, proposer.as_str())?;

    Ok(Response::default()
        .add_submessages(hooks)
        .add_attribute("action", "propose")
        .add_attribute("sender", sender)
        .add_attribute("proposal_id", id.to_string())
        .add_attribute("status", proposal.status.to_string()))
}

pub fn execute_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    vote: MultipleChoiceVote,
    rationale: Option<String>,
) -> Result<Response<Empty>, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut prop = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(ContractError::NoSuchProposal { id: proposal_id })?;

    // TODO: Check that the vote is within the bounds of the set voting range of module

    // Check that this is a valid vote.
    if vote.option_id as usize >= prop.choices.len() {
        return Err(ContractError::InvalidVote {});
    }

 // Allow voting on proposals until they expire.
    // Voting on a non-open proposal will never change
    // their outcome as if an outcome has been determined,
    // it is because no possible sequence of votes may
    // cause a different one. This then serves to allow
    // for better tallies of opinions in the event that a
    // proposal passes or is rejected early.
    if prop.expiration.is_expired(&env.block) {
        return Err(ContractError::Expired { id: proposal_id });
    }


    let vote_power = get_voting_power(
        deps.as_ref(),
        info.sender.clone(),
        &config.dao,
        Some(prop.start_height),
    )?;
    if vote_power.is_zero() {
        return Err(ContractError::NotRegistered {});
    }
    
    BALLOTS.update(deps.storage, (proposal_id, &info.sender), |bal| match bal {
        Some(current_ballot) => {
            if prop.allow_revoting {
                if current_ballot.vote == vote {
                    // Don't allow casting the same vote more than
                    // once. This seems liable to be confusing
                    // behavior.
                    Err(ContractError::AlreadyCast {})
                } else {
                    // Remove the old vote if this is a re-vote.
                    prop.votes
                        .remove_vote(current_ballot.vote, current_ballot.power)?;
                    Ok(Ballot {
                        power: vote_power,
                        vote,
                        rationale,
                    })
                }
            } else {
                Err(ContractError::AlreadyVoted {})
            }
        }
        None => Ok(Ballot {
            vote,
            power: vote_power,
            rationale,
        }),
    })?;
    
    let old_status = prop.status;

    prop.votes.add_vote(vote, vote_power)?;
    prop.update_status(&env.block)?;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;
    let new_status = prop.status;
    let change_hooks = proposal_status_changed_hooks(
        PROPOSAL_HOOKS,
        deps.storage,
        proposal_id,
        old_status.to_string(),
        new_status.to_string(),
    )?;
    let vote_hooks = new_vote_hooks(
        VOTE_HOOKS,
        deps.storage,
        proposal_id,
        info.sender.to_string(),
        vote.to_string(),
    )?;
    Ok(Response::default()
        .add_submessages(change_hooks)
        .add_submessages(vote_hooks)
        .add_attribute("action", "vote")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("position", vote.to_string())
        .add_attribute("status", prop.status.to_string()))
}

pub fn execute_execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let mut prop = PROPOSALS
    .may_load(deps.storage, proposal_id)?
    .ok_or(ContractError::NoSuchProposal { id: proposal_id })?;

    let config = CONFIG.load(deps.storage)?;
    if config.only_members_execute {
        let power = get_voting_power(
            deps.as_ref(),
            info.sender.clone(),
            &config.dao,
            Some(prop.start_height),
        )?;
        if power.is_zero() {
            return Err(ContractError::Unauthorized {});
        }
    }
    
    // Check here that the proposal is passed. Allow it to be
    // executed even if it is expired so long as it passed during its
    // voting period.
    prop.update_status(&env.block)?;
    let old_status = prop.status;
    if prop.status != Status::Passed {
        return Err(ContractError::NotPassed {});
    }

    prop.status = Status::Executed;
    
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;


    // CALCULATING VOTING LOGIC 
    let vote_result = prop.calculate_vote_result()?;

    match vote_result {
        VoteResult::Tie => Err(ContractError::Tie {}), // We don't anticipate this case as the proposal would not be in passed state, checked above.
        VoteResult::SingleWinner(winning_choice) => {
            let response = if !winning_choice.msgs.is_empty() {
                let execute_message = WasmMsg::Execute {
                    contract_addr: config.dao.to_string(),
                    msg: to_binary(&dao_core::msg::ExecuteMsg::ExecuteProposalHook {
                        msgs: winning_choice.msgs,
                    })?,
                    funds: vec![],
                };
                match config.close_proposal_on_execution_failure {
                    true => {
                        let masked_proposal_id = mask_proposal_execution_proposal_id(proposal_id);
                        Response::default().add_submessage(SubMsg::reply_on_error(
                            execute_message,
                            masked_proposal_id,
                        ))
                    }
                    false => Response::default().add_message(execute_message),
                }
                } else {
                    Response::default()
                };

                let hooks = proposal_status_changed_hooks(
                    PROPOSAL_HOOKS,
                    deps.storage,
                    proposal_id,
                    old_status.to_string(),
                    prop.status.to_string(),
                )?;

                  // Add prepropose / deposit module hook which will handle deposit refunds.
                  let proposal_creation_policy = CREATION_POLICY.load(deps.storage)?;
                  let hooks = match proposal_creation_policy {
                    ProposalCreationPolicy::Anyone {} => hooks,
                    ProposalCreationPolicy::Module { addr } => {
                        let msg = to_binary(&PreProposeMsg::ProposalCompletedHook {
                            proposal_id,
                            new_status: prop.status,
                        })?;
                        let mut hooks = hooks;
                        hooks.push(SubMsg::reply_on_error(
                            WasmMsg::Execute {
                                contract_addr: addr.into_string(),
                                msg,
                                funds: vec![],
                            },
                            failed_pre_propose_module_hook_id(),
                        ));
                        hooks
                    }
                };


                // TODO: Set Max number of proposals for Entry

                Ok(response
                    .add_submessages(hooks)
                    .add_attribute("action", "execute")
                    .add_attribute("sender", info.sender)
                    .add_attribute("proposal_id", proposal_id.to_string())
                    .add_attribute("dao", config.dao))
                }
            }
        }

        pub fn execute_close(
            deps: DepsMut,
            env: Env,
            info: MessageInfo,
            proposal_id: u64,
        ) -> Result<Response<Empty>, ContractError> {
            let mut prop = PROPOSALS.load(deps.storage, proposal_id)?;
        
            prop.update_status(&env.block)?;
            if prop.status != Status::Rejected {
                return Err(ContractError::WrongCloseStatus {});
            }
        
            let old_status = prop.status;
        
            prop.status = Status::Closed;
        
            PROPOSALS.save(deps.storage, proposal_id, &prop)?;
        
            let hooks = proposal_status_changed_hooks(
                PROPOSAL_HOOKS,
                deps.storage,
                proposal_id,
                old_status.to_string(),
                prop.status.to_string(),
            )?;
        
            // Add prepropose / deposit module hook which will handle deposit refunds.
            let proposal_creation_policy = CREATION_POLICY.load(deps.storage)?;
            let hooks = match proposal_creation_policy {
                ProposalCreationPolicy::Anyone {} => hooks,
                ProposalCreationPolicy::Module { addr } => {
                    let msg = to_binary(&PreProposeMsg::ProposalCompletedHook {
                        proposal_id,
                        new_status: prop.status,
                    })?;
                    let mut hooks = hooks;
                    hooks.push(SubMsg::reply_on_error(
                        WasmMsg::Execute {
                            contract_addr: addr.into_string(),
                            msg,
                            funds: vec![],
                        },
                        failed_pre_propose_module_hook_id(),
                    ));
                    hooks
                }
            };
            Ok(Response::default()
                .add_submessages(hooks)
                .add_attribute("action", "close")
                .add_attribute("sender", info.sender)
                .add_attribute("proposal_id", proposal_id.to_string()))
        }

        #[allow(clippy::too_many_arguments)]
        pub fn execute_update_config(
            deps: DepsMut,
            info: MessageInfo,
            voting_strategy: VotingStrategy,
            min_voting_period: Option<Duration>,
            max_voting_period: Duration,
            min_voting_range: u64,
            max_voting_range: u64,
            only_members_execute: bool,
            only_admins_propose: bool,
            allow_revoting: bool,
            allow_updating_proposal: bool,
            dao: String,
            close_proposal_on_execution_failure: bool,
        ) -> Result<Response, ContractError> {
            let config = CONFIG.load(deps.storage)?;
            
            // Only the DAO may call this method. 
            if info.sender != config.dao {
                return Err(ContractError::Unauthorized {});
            }

            voting_strategy.validate()?;

            let dao = deps.api.addr_validate(&dao)?;

            let (min_voting_period, max_voting_period) =
        validate_voting_period(min_voting_period, max_voting_period)?;

        // min & maax choice voting range
        let (min_voting_range, max_voting_range) =
        validate_voting_range(min_voting_range, max_voting_range)?;

        CONFIG.save(
            deps.storage,
            &Config {
                voting_strategy,
                min_voting_period,
                max_voting_period,
                min_voting_range,
                max_voting_range,
                only_members_execute,
                only_admins_propose,
                allow_revoting,
                allow_updating_proposal,
                dao,
                close_proposal_on_execution_failure,
            },
        )?;

        Ok(Response::default()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
        }

        pub fn execute_update_proposal_creation_policy(
            deps: DepsMut,
            info: MessageInfo,
            new_info: PreProposeInfo,
        ) -> Result<Response, ContractError> {
            let config = CONFIG.load(deps.storage)?;
            if config.dao != info.sender {
                return Err(ContractError::Unauthorized {});
            }

            let (initial_policy, messages) = new_info.into_initial_policy_and_messages(config.dao)?;
            CREATION_POLICY.save(deps.storage, &initial_policy)?;

            Ok(Response::default()
            .add_submessages(messages)
            .add_attribute("action", "update_proposal_creation_policy")
            .add_attribute("sender", info.sender)
            .add_attribute("new_policy", format!("{initial_policy:?}")))
    }

    pub fn execute_update_rationale(
        deps: DepsMut,
        info: MessageInfo,
        proposal_id: u64,
        rationale: Option<String>,
    ) -> Result<Response, ContractError> {
        BALLOTS.update(
            deps.storage,
            // info.sender can't be forged so we implicitly access control
            // with the key.
            (proposal_id, &info.sender),
            |ballot| match ballot {
                Some(ballot) => Ok(Ballot {
                    rationale: rationale.clone(),
                    ..ballot
                }),
                None => Err(ContractError::NoSuchVote {
                    id: proposal_id,
                    voter: info.sender.to_string(),
                }),
            },
        )?;
    
        Ok(Response::default()
            .add_attribute("action", "update_rationale")
            .add_attribute("sender", info.sender)
            .add_attribute("proposal_id", proposal_id.to_string())
            .add_attribute("rationale", rationale.as_deref().unwrap_or("none")))
    }

    pub fn execute_add_proposal_hook(
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        address: String,
    ) -> Result<Response, ContractError> {
        let config = CONFIG.load(deps.storage)?;
        if config.dao != info.sender {
            // Only DAO can add hooks
            return Err(ContractError::Unauthorized {});
        }
    
        let validated_address = deps.api.addr_validate(&address)?;
    
        add_hook(PROPOSAL_HOOKS, deps.storage, validated_address)?;
    
        Ok(Response::default()
            .add_attribute("action", "add_proposal_hook")
            .add_attribute("address", address))
    }

    pub fn execute_remove_proposal_hook(
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        address: String,
    ) -> Result<Response, ContractError> {
        let config = CONFIG.load(deps.storage)?;
        if config.dao != info.sender {
            // Only DAO can remove hooks
            return Err(ContractError::Unauthorized {});
        }
    
        let validated_address = deps.api.addr_validate(&address)?;
    
        remove_hook(PROPOSAL_HOOKS, deps.storage, validated_address)?;
    
        Ok(Response::default()
            .add_attribute("action", "remove_proposal_hook")
            .add_attribute("address", address))
    }
    
    pub fn execute_add_vote_hook(
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        address: String,
    ) -> Result<Response, ContractError> {
        let config = CONFIG.load(deps.storage)?;
        if config.dao != info.sender {
            // Only DAO can add hooks
            return Err(ContractError::Unauthorized {});
        }
    
        let validated_address = deps.api.addr_validate(&address)?;
    
        add_hook(VOTE_HOOKS, deps.storage, validated_address)?;
    
        Ok(Response::default()
            .add_attribute("action", "add_vote_hook")
            .add_attribute("address", address))
    }

    pub fn execute_remove_vote_hook(
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        address: String,
    ) -> Result<Response, ContractError> {
        let config = CONFIG.load(deps.storage)?;
        if config.dao != info.sender {
            // Only DAO can remove hooks
            return Err(ContractError::Unauthorized {});
        }
    
        let validated_address = deps.api.addr_validate(&address)?;
    
        remove_hook(VOTE_HOOKS, deps.storage, validated_address)?;
    
        Ok(Response::default()
            .add_attribute("action", "remove_vote_hook")
            .add_attribute("address", address))
    }

    pub fn add_hook(
        hooks: Hooks,
        storage: &mut dyn Storage,
        validated_address: Addr,
    ) -> Result<(), ContractError> {
        hooks
            .add_hook(storage, validated_address)
            .map_err(ContractError::HookError)?;
        Ok(())
    }

    pub fn remove_hook(
        hooks: Hooks,
        storage: &mut dyn Storage,
        validate_address: Addr,
    ) -> Result<(), ContractError> {
        hooks
            .remove_hook(storage, validate_address)
            .map_err(ContractError::HookError)?;
        Ok(())
    }
    
    pub fn next_proposal_id(store: &dyn Storage) -> StdResult<u64> {
        Ok(PROPOSAL_COUNT.may_load(store)?.unwrap_or_default() + 1)
    }
    
    pub fn advance_proposal_id(store: &mut dyn Storage) -> StdResult<u64> {
        let id: u64 = next_proposal_id(store)?;
        PROPOSAL_COUNT.save(store, &id)?;
        Ok(id)
    }
    
    // QUERIES 
    #[cfg_attr(not(feature = "library"), entry_point)]
    pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
        match msg {
            QueryMsg::Config {} => query_config(deps),
            QueryMsg::Proposal { proposal_id } => query_proposal(deps, env, proposal_id),
            QueryMsg::ListProposals { start_after, limit } => {
                query_list_proposals(deps, env, start_after, limit)
            }
            QueryMsg::NextProposalId {} => query_next_proposal_id(deps),
            QueryMsg::ProposalCount {} => query_proposal_count(deps),
//TODO      QueryMsg::GetVoteByChoice { proposal_id, choice_id, voter } => query_vote_by_choice(deps, proposal_id, choice_id, voter),
            QueryMsg::GetVote { proposal_id, voter } => query_vote(deps, proposal_id, voter),
            QueryMsg::ListVotes {
                proposal_id,
                start_after,
                limit,
            } => query_list_votes(deps, proposal_id, start_after, limit),
            QueryMsg::Info {} => query_info(deps),
            QueryMsg::ReverseProposals {
                start_before,
                limit,
            } => query_reverse_proposals(deps, env, start_before, limit),
            QueryMsg::ProposalCreationPolicy {} => query_creation_policy(deps),
            QueryMsg::ProposalHooks {} => to_binary(&PROPOSAL_HOOKS.query_hooks(deps)?),
            QueryMsg::VoteHooks {} => to_binary(&VOTE_HOOKS.query_hooks(deps)?),
            QueryMsg::Dao {} => query_dao(deps),
        }
    }

    pub fn query_config(deps: Deps) -> StdResult<Binary> {
        let config = CONFIG.load(deps.storage)?;
        to_binary(&config)
    }

    pub fn query_dao(deps: Deps) -> StdResult<Binary> {
        let config = CONFIG.load(deps.storage)?;
        to_binary(&config.dao)
    }

    pub fn query_proposal(deps: Deps, env: Env, id: u64) -> StdResult<Binary> {
        let proposal = PROPOSALS.load(deps.storage, id)?;
        to_binary(&proposal.into_response(&env.block, id)?)
    }

    pub fn query_creation_policy(deps: Deps) -> StdResult<Binary> {
        let policy = CREATION_POLICY.load(deps.storage)?;
        to_binary(&policy)
    }

    pub fn query_list_proposals(
        deps: Deps,
        env: Env,
        start_after: Option<u64>,
        limit: Option<u64>,
    ) -> StdResult<Binary> {
        let min = start_after.map(Bound::exclusive);
        let limit = limit.unwrap_or(DEFAULT_LIMIT);
        let props: Vec<ProposalResponse> = PROPOSALS
        .range(deps.storage, min, None, cosmwasm_std::Order::Ascending)
        .take(limit as usize)
        .collect::<Result<Vec<(u64, MultipleChoiceProposal)>, _>>()?
        .into_iter()
        .map(|(id, proposal)| proposal.into_response(&env.block, id))
        .collect::<StdResult<Vec<ProposalResponse>>>()?;

        to_binary(&ProposalListResponse { proposals: props })
    }

    pub fn query_reverse_proposals(
        deps: Deps,
        env: Env,
        start_before: Option<u64>,
        limit: Option<u64>,
    ) -> StdResult<Binary> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT);
        let max = start_before.map(Bound::exclusive);
        let props: Vec<ProposalResponse> = PROPOSALS
            .range(deps.storage, None, max, cosmwasm_std::Order::Descending)
            .take(limit as usize)
            .collect::<Result<Vec<(u64, MultipleChoiceProposal)>, _>>()?
            .into_iter()
            .map(|(id, proposal)| proposal.into_response(&env.block, id))
            .collect::<StdResult<Vec<ProposalResponse>>>()?;
    
        to_binary(&ProposalListResponse { proposals: props })
    }

    pub fn query_next_proposal_id(deps: Deps) -> StdResult<Binary> {
        to_binary(&next_proposal_id(deps.storage)?)
    }

    pub fn query_proposal_count(deps: Deps) -> StdResult<Binary> {
        let proposal_count = PROPOSAL_COUNT.load(deps.storage)?;
        to_binary(&proposal_count)
    }

    pub fn query_vote(deps: Deps, proposal_id: u64, voter: String) -> StdResult<Binary> {
        let voter = deps.api.addr_validate(&voter)?;
        let ballot = BALLOTS.may_load(deps.storage, (proposal_id, &voter))?;
        let vote = ballot.map(|ballot| VoteInfo {
            voter,
            vote: ballot.vote,
            power: ballot.power,
            rationale: ballot.rationale,
        });
        to_binary(&VoteResponse { vote })
    }

// TODO:  query_vote_by_choice()

pub fn query_list_votes(
    deps: Deps,
    proposal_id: u64,
    start_after: Option<String>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let start_after = start_after
        .map(|addr| deps.api.addr_validate(&addr))
        .transpose()?;
    let min = start_after.as_ref().map(Bound::<&Addr>::exclusive);

    let votes = BALLOTS
        .prefix(proposal_id)
        .range(deps.storage, min, None, cosmwasm_std::Order::Ascending)
        .take(limit as usize)
        .map(|item| {
            let (voter, ballot) = item?;
            Ok(VoteInfo {
                voter,
                vote: ballot.vote,
                power: ballot.power,
                rationale: ballot.rationale,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    to_binary(&VoteListResponse { votes })
}

pub fn query_info(deps: Deps) -> StdResult<Binary> {
    let info = cw2::get_contract_version(deps.storage)?;
    to_binary(&dao_interface::voting::InfoResponse { info })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let repl = TaggedReplyId::new(msg.id)?;
    match repl {
        TaggedReplyId::FailedProposalExecution(proposal_id) => {
            PROPOSALS.update(deps.storage, proposal_id, |prop| match prop {
                Some(mut prop) => {
                    prop.status = Status::ExecutionFailed;
                    Ok(prop)
                }
                None => Err(ContractError::NoSuchProposal { id: proposal_id }),
            })?;
            Ok(Response::new().add_attribute("proposal execution failed", proposal_id.to_string()))
        }
        TaggedReplyId::FailedProposalHook(idx) => {
            let addr = PROPOSAL_HOOKS.remove_hook_by_index(deps.storage, idx)?;
            Ok(Response::new().add_attribute("removed_proposal_hook", format!("{addr}:{idx}")))
        }
        TaggedReplyId::FailedVoteHook(idx) => {
            let addr = VOTE_HOOKS.remove_hook_by_index(deps.storage, idx)?;
            Ok(Response::new().add_attribute("removed vote hook", format!("{addr}:{idx}")))
        }
        TaggedReplyId::PreProposeModuleInstantiation => {
            let res = parse_reply_instantiate_data(msg)?;
            let module = deps.api.addr_validate(&res.contract_address)?;
            CREATION_POLICY.save(
                deps.storage,
                &ProposalCreationPolicy::Module { addr: module },
            )?;

            match res.data {
                Some(data) => Ok(Response::new()
                    .add_attribute("update_pre_propose_module", res.contract_address)
                    .set_data(data)),
                None => Ok(Response::new()
                    .add_attribute("update_pre_propose_module", res.contract_address)),
            }
        }
        TaggedReplyId::FailedPreProposeModuleHook => {
            let addr = match CREATION_POLICY.load(deps.storage)? {
                ProposalCreationPolicy::Anyone {} => {
                    // Something is off if we're getting this
                    // reply and we don't have a pre-propose
                    // module installed. This should be
                    // unreachable.
                    return Err(ContractError::InvalidReplyID {
                        id: failed_pre_propose_module_hook_id(),
                    });
                }
                ProposalCreationPolicy::Module { addr } => {
                    // If we are here, our pre-propose module has
                    // errored while receiving a proposal
                    // hook. Rest in peace pre-propose module.
                    CREATION_POLICY.save(deps.storage, &ProposalCreationPolicy::Anyone {})?;
                    addr
                }
            };
            Ok(Response::new().add_attribute("failed_prepropose_hook", format!("{addr}")))
        }
    }
}
        
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}
