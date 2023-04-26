## dao-entry-judging

A proposal module for a DAO DAO DAO which allows the users to allocate
points on a number of elements, from an array of `EntryJudgingOption`.


Votes can be cast for as long as the proposal is not expired.

## Desired behaviors
- Admins set number of proposal elements
- Admins set a minimum proposal element vote. minimum 0.0
- Admins set a maximum proposal element vote.
- Admins set if dao members can upload proposals.
- If allowed, admin set limit on number of proposals available per dao member, defaults 1.
- Proposals are able to be uploaded via the desired configuration of the admins. 
- Proposals are able to be updated by the creators, and event admins if needed but only while entries are active
- Proposals elements vote weights are defined by the voter, within the proposal element vote range set by admins.
- Queries for the current leader of proposals within the event-judging module, based of sum of all proposal element points voted on by a dao members
- Queries for the current leader of proposals within the event-judging module, by proposal element, based on the sun of a specific proposal element points voted on by a dao members.


## Undesired behavior

The undesired behavior of this contract is tested under `testing/adversarial_tests.rs`.

In general, it should cover:
- Executing unpassed proposals
- Executing proposals more than once
- Social engineering proposals for financial benefit
- Convincing proposal modules to spend someone else's allowance
- Ensuring more proposals cannot be created by dao member than configured 
- Ensuring proposals cannot be modified after voting period is over
- Ensuring self-voting choice implements desired choice
- Ensuring voting is not accepted outside the bounds of the voting params

## Proposal deposits

Proposal deposits for this module are handled by the
[`dao-pre-entry-judging`](../../pre-propose/dao-pre-entry-judging)
contract.

## Hooks

This module supports hooks for voting and proposal status changes. One
may register a contract to receive these hooks with the `AddVoteHook`
and `AddProposalHook` methods. Upon registration the contract will
receive messages whenever a vote is cast and a proposal's status
changes (for example, when the proposal passes).

The format for these hook messages can be located in the
`proposal-hooks` and `vote-hooks` packages located in
`packages/proposal-hooks` and `packages/vote-hooks` respectively.

To stop an invalid hook receiver from locking the proposal module
receivers will be removed from the hook list if they error when
handling a hook.

## Revoting

The proposals may be configured to allow revoting.
In such cases, users are able to change their vote as long as the proposal is still open.
Revoting for the currently cast option will return an error.

## Self-Voting

The proposals may be configured to allow a proposal creator to .
In such cases, users are able to change their vote as long as the proposal is still open.
Revoting for the currently cast option will return an error.