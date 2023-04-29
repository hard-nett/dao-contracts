use cosmwasm_schema::cw_serde;
use cosmwasm_std::{CosmosMsg, Empty, StdError, StdResult, Uint128};


use crate::threshold::{validate_quorum, PercentageThreshold, ThresholdError};


/// Maximum number of choices for entry judges votes. Chosen
/// in order to impose a bound on state / queries.

pub const MAX_NUM_CHOICES: u32 = 20;
const NONE_OPTION_DESCRIPTION: &str = "None of the above";


/// Determines how many choices may be selected.
#[cw_serde]
pub enum VotingStrategy {
    SingleChoice { quorum: PercentageThreshold },
}

impl VotingStrategy {
    pub fn validate(&self) -> Result<(), ThresholdError> {
        match self {
            VotingStrategy::SingleChoice { quorum } => validate_quorum(quorum),
        }
    }

    pub fn get_quorum(&self) -> PercentageThreshold {
        match self {
            VotingStrategy::SingleChoice { quorum } => *quorum,
        }
    }
}

/// A multiple choice vote, picking the desired option
#[cw_serde]
#[derive(Copy)]
pub struct EntryJudgingVote {
    // A vote indicates which option the user has selected, and the number of points allocated to that option
    pub option_id: u32,
    pub vote: Uint128,
}

impl std::fmt::Display for EntryJudgingVote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.option_id)
    }
}

// Holds the vote weights for each option
#[cw_serde]

pub struct EntryJudgingVotes {
    // Vote counts is a vector of integers indicating the vote weight for each option
    // (the index corresponds to the option).
    pub vote_weights: Vec<Uint128>,
}

impl EntryJudgingVotes {
     /// Sum of all vote weights
     pub fn total(&self) -> Uint128 {
        self.vote_weights.iter().sum()
    }

     // Add a vote to the tally
     pub fn add_vote(&mut self, vote: EntryJudgingVote, weight: Uint128) -> StdResult<()> {
        self.vote_weights[vote.option_id as usize] = self.vote_weights[vote.option_id as usize]
            .checked_add(weight)
            .map_err(StdError::overflow)?;
        Ok(())
    }

    
    // Remove a vote from the tally
    pub fn remove_vote(&mut self, vote: EntryJudgingVote, weight: Uint128) -> StdResult<()> {
        self.vote_weights[vote.option_id as usize] = self.vote_weights[vote.option_id as usize]
            .checked_sub(weight)
            .map_err(StdError::overflow)?;
        Ok(())
    }

    // Default tally of zero for all multiple choice options
    pub fn zero(num_choices: usize) -> Self {
        Self {
            vote_weights: vec![Uint128::zero(); num_choices],
        }
    }
}

/// Represents the type of Multiple choice option. "None of the above" has a special
/// type for example.
#[cw_serde]
pub enum MultipleChoiceOptionType {
    /// Choice that represents selecting none of the options; still counts toward quorum
    /// and allows proposals with all bad options to be voted against.
    Standard,
}

/// Represents unchecked multiple choice options
#[cw_serde]
pub struct MultipleChoiceOptions {
    pub options: Vec<MultipleChoiceOption>,
}

/// Unchecked multiple choice option
#[cw_serde]
pub struct MultipleChoiceOption {
    pub title: String,
    pub description: String,
    pub msgs: Vec<CosmosMsg<Empty>>,
}

/// Multiple choice options that have been verified for correctness, and have all fields
/// necessary for voting.
#[cw_serde]
pub struct CheckedMultipleChoiceOptions {
    pub options: Vec<CheckedMultipleChoiceOption>,
}

/// A verified option that has all fields needed for voting.
#[cw_serde]
pub struct CheckedMultipleChoiceOption {
    // This is the index of the option in both the vote_weights and proposal.choices vectors.
    // Workaround due to not being able to use HashMaps in Cosmwasm.
    pub index: u32,
    pub option_type: MultipleChoiceOptionType,
    pub title: String,
    pub description: String,
    pub msgs: Vec<CosmosMsg<Empty>>,
    pub vote_count: Uint128,
}

impl MultipleChoiceOptions {
    pub fn into_checked(self) -> StdResult<CheckedMultipleChoiceOptions> {
        if self.options.len() < 2 || self.options.len() > MAX_NUM_CHOICES as usize {
            return Err(StdError::GenericErr {
                msg: "Wrong number of choices".to_string(),
            });
        }

        let mut checked_options: Vec<CheckedMultipleChoiceOption> =
        Vec::with_capacity(self.options.len() + 1);

    // Iterate through choices and save the index and option type for each
    self.options
        .into_iter()
        .enumerate()
        .for_each(|(idx, choice)| {
            let checked_option = CheckedMultipleChoiceOption {
                index: idx as u32,
                option_type: MultipleChoiceOptionType::Standard,
                description: choice.description,
                msgs: choice.msgs,
                vote_count: Uint128::zero(),
                title: choice.title,
            };
            checked_options.push(checked_option)
        });
        
  // Add a "None of the above" option, required for every multiple choice proposal.
  let none_option = CheckedMultipleChoiceOption {
    index: (checked_options.capacity() - 1) as u32,
    option_type: MultipleChoiceOptionType::None,
    description: NONE_OPTION_DESCRIPTION.to_string(),
    msgs: vec![],
    vote_count: Uint128::zero(),
    title: NONE_OPTION_DESCRIPTION.to_string(),
};

checked_options.push(none_option);

let options = CheckedMultipleChoiceOptions {
    options: checked_options,
};
Ok(options)
}
}