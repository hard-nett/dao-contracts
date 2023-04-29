use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum VotingError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("min_voting_period and max_voting_period must have the same units (height or time)")]
    DurationUnitsConflict {},

    #[error("min_voting_range and max_voting_range must have the same units (height or time)")]
    RangeUnitsConflict {},

    #[error("Min voting period must be less than or equal to max voting period")]
    InvalidMinVotingPeriod {},

    #[error("Min voting range must be less than or equal to max voting range")]
    InvalidMinVotingRange {},
}
