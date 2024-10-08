mod helper;
use helper::Helper;

use scrypto_test::prelude::*;

// Test to ensure proposal creation fails when insufficient tokens are provided
#[test]
fn test_failed_create_proposal() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Attempt to create a proposal with insufficient tokens (9999 instead of 10000)
    let failed_creation = helper.create_basic_proposal(dec!(9999));
    assert!(failed_creation.is_err());

    Ok(())
}

// Test successful proposal creation
#[test]
fn test_create_proposal() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Create a proposal with sufficient tokens (19999)
    let (bucket_1, _bucket_2) = helper.create_basic_proposal(dec!(19999))?;
    // Verify that the correct amount of tokens (9999) is returned
    let _ = helper.assert_bucket_eq(&bucket_1, helper.ilis_address, dec!(9999))?;

    Ok(())
}

// Test the full lifecycle of a proposal from creation to execution
#[test]
fn test_proposal_lifetime_to_excecution() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let proposal_bucket_return = helper.submit_proposal(proposal_bucket)?;
    // Vote on the proposal
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute the proposal
    helper.finish_voting(0)?;
    helper.execute_proposal_step(0, 1)?;

    // Verify rewarded update (which has been changed through accepting the proposal to 100 ILIS / day)
    let bucket_2 = helper.rewarded_update()?;
    helper.assert_bucket_eq(&bucket_2, helper.ilis_address, dec!(700))?;

    // Retrieve and verify the proposal fee
    let returned_payment = helper.retrieve_fee(proposal_bucket_return)?;
    helper.assert_bucket_eq(&returned_payment, helper.ilis_address, dec!(10000))?;

    Ok(())
}

// Test to ensure voting twice on the same proposal fails
#[test]
fn test_proposal_vote_twice() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;
    // Vote on the proposal
    let stake_id_return = helper.vote_on_proposal(true, stake_id, 0)?;
    // Attempt to vote again (should fail)
    let failure = helper.vote_on_proposal(true, stake_id_return, 0);

    assert!(failure.is_err());

    Ok(())
}

// Test to ensure unstaking too early after voting fails
#[test]
fn test_proposal_vote_and_unstake_too_early() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;
    // Vote on the proposal
    let stake_id_return = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Attempt to unstake (should fail as it's too early)
    let failure = helper.start_unstake(stake_id_return, dec!(5000));

    assert!(failure.is_err());

    Ok(())
}

// Test successful unstaking after voting
#[test]
fn test_proposal_vote_and_unstake() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;
    // Vote on the proposal
    let stake_id_return = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 8 days
    let new_time_1 = helper.env.get_current_time().add_days(8).unwrap();
    helper.env.set_current_time(new_time_1);

    // Unstake (should succeed as enough time has passed)
    let _ = helper.start_unstake(stake_id_return, dec!(5000))?;

    Ok(())
}

// Test proposal failure due to veto during the last day of voting
#[test]
fn test_proposal_enter_veto_mode_during_last_day_fail_by_veto() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for multiple voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();
    let bucket_2 = helper.ilis.take(dec!(5000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();
    let bucket_3 = helper.ilis.take(dec!(20000), &mut helper.env)?;
    let stake_id_3 = helper.stake_without_id(bucket_3)?.0.unwrap();
    let bucket_4 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id_4 = helper.stake_without_id(bucket_4)?.0.unwrap();
    let bucket_5 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id_5 = helper.stake_without_id(bucket_5)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // First vote
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 6 days
    let new_time_1 = helper.env.get_current_time().add_days(6).unwrap();
    helper.env.set_current_time(new_time_1);

    // More votes
    let _ = helper.vote_on_proposal(true, stake_id_2, 0)?;
    let _ = helper.vote_on_proposal(false, stake_id_3, 0)?;
    let _ = helper.vote_on_proposal(true, stake_id_5, 0)?;

    // Advance time by 1 day (entering last day)
    let new_time_2 = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(new_time_2);

    // Veto vote during last day
    let _ = helper.vote_on_proposal(false, stake_id_4, 0)?;

    // Advance time by 1 more day
    let new_time_3 = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(new_time_3);

    // Finish voting and attempt to execute (should fail due to veto)
    let _ = helper.finish_voting(0)?;
    let failure = helper.execute_proposal_step(0, 1);

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_proposal_enter_veto_mode_during_last_day_but_succeed() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for multiple voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();
    let bucket_2 = helper.ilis.take(dec!(5000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();
    let bucket_3 = helper.ilis.take(dec!(20000), &mut helper.env)?;
    let stake_id_3 = helper.stake_without_id(bucket_3)?.0.unwrap();
    let bucket_5 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id_5 = helper.stake_without_id(bucket_5)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // First vote
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 6 days
    let new_time_1 = helper.env.get_current_time().add_days(6).unwrap();
    helper.env.set_current_time(new_time_1);

    // More votes
    let _ = helper.vote_on_proposal(true, stake_id_2, 0)?;
    let _ = helper.vote_on_proposal(false, stake_id_3, 0)?;
    let _ = helper.vote_on_proposal(true, stake_id_5, 0)?;

    // Advance time by 2 more days (past voting period)
    let new_time_2 = helper.env.get_current_time().add_days(2).unwrap();
    helper.env.set_current_time(new_time_2);

    // Finish voting and execute (should succeed)
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 1)?;

    Ok(())
}

#[test]
fn test_proposal_enter_veto_mode_but_vote_for() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for multiple voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();
    let bucket_3 = helper.ilis.take(dec!(20000), &mut helper.env)?;
    let stake_id_3 = helper.stake_without_id(bucket_3)?.0.unwrap();
    let bucket_4 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id_4 = helper.stake_without_id(bucket_4)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // First vote
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 6 days
    let new_time_1 = helper.env.get_current_time().add_days(6).unwrap();
    helper.env.set_current_time(new_time_1);

    // Vote against, entering veto mode
    let _ = helper.vote_on_proposal(false, stake_id_3, 0)?;

    // Advance time by 1 day (entering last day)
    let new_time_2 = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(new_time_2);

    // Attempt to vote for during veto mode (should fail)
    let failure = helper.vote_on_proposal(true, stake_id_4, 0);
    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_proposal_enter_last_day_failing_then_succeed_fail_in_veto_mode() -> Result<(), RuntimeError>
{
    let mut helper = Helper::new().unwrap();

    // Stake tokens for multiple voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();
    let bucket_2 = helper.ilis.take(dec!(5000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();
    let bucket_3 = helper.ilis.take(dec!(20000), &mut helper.env)?;
    let stake_id_3 = helper.stake_without_id(bucket_3)?.0.unwrap();
    let bucket_5 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id_5 = helper.stake_without_id(bucket_5)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // First vote (against)
    let _ = helper.vote_on_proposal(false, stake_id, 0)?;

    // Advance time by 6 days
    let new_time_1 = helper.env.get_current_time().add_days(6).unwrap();
    helper.env.set_current_time(new_time_1);

    // More votes
    let _ = helper.vote_on_proposal(false, stake_id_2, 0)?;
    let _ = helper.vote_on_proposal(true, stake_id_3, 0)?;

    // Advance time by 1 day (entering last day)
    let new_time_2 = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(new_time_2);

    // Vote against during last day (entering veto mode)
    let _ = helper.vote_on_proposal(false, stake_id_5, 0)?;

    // Advance time by 1 more day
    let new_time_3 = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(new_time_3);

    // Finish voting and attempt to execute (should fail due to veto)
    let _ = helper.finish_voting(0)?;
    let failure = helper.execute_proposal_step(0, 1);
    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_proposal_enter_last_day_failing_then_succeed() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for two voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();
    let bucket_2 = helper.ilis.take(dec!(5000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // First vote (against)
    let _ = helper.vote_on_proposal(false, stake_id, 0)?;

    // Advance time by 6 days
    let new_time_1 = helper.env.get_current_time().add_days(6).unwrap();
    helper.env.set_current_time(new_time_1);

    // Second vote (against)
    let _ = helper.vote_on_proposal(false, stake_id_2, 0)?;

    // Advance time by 2 more days
    let new_time_2 = helper.env.get_current_time().add_days(2).unwrap();
    helper.env.set_current_time(new_time_2);

    // Finish voting and attempt to execute (should fail due to all votes against)
    let _ = helper.finish_voting(0)?;
    let failure = helper.execute_proposal_step(0, 1);
    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_proposal_enter_last_day_failing_and_keep_failing() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for two voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();
    let bucket_2 = helper.ilis.take(dec!(5000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // First vote (against)
    let _ = helper.vote_on_proposal(false, stake_id, 0)?;

    // Advance time by 6 days
    let new_time_1 = helper.env.get_current_time().add_days(6).unwrap();
    helper.env.set_current_time(new_time_1);

    // Second vote (against)
    let _ = helper.vote_on_proposal(false, stake_id_2, 0)?;

    // Advance time by 2 more days
    let new_time_2 = helper.env.get_current_time().add_days(2).unwrap();
    helper.env.set_current_time(new_time_2);

    // Finish voting and attempt to execute (should fail due to all votes against)
    let _ = helper.finish_voting(0)?;
    let failure = helper.execute_proposal_step(0, 1);
    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_proposal_fail_below_quorum() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(5000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // Vote on the proposal
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days (end of voting period)
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and attempt to execute (should fail due to not meeting quorum)
    let _ = helper.finish_voting(0)?;
    let failure = helper.execute_proposal_step(0, 1);

    assert!(failure.is_err());

    Ok(())
}

#[test]
pub fn test_proposal_with_multiple_steps_fail_to_retrieve_fee() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create a proposal with multiple steps
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let proposal_bucket_return = helper.add_normal_proposal_step(proposal_bucket)?;

    // Submit the proposal and vote
    let proposal_bucket_return_2 = helper.submit_proposal(proposal_bucket_return)?;
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days (end of voting period)
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute only the first step
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 1)?;

    // Attempt to retrieve fee (should fail as not all steps are executed)
    let failure = helper.retrieve_fee(proposal_bucket_return_2);

    assert!(failure.is_err());

    Ok(())
}

#[test]
pub fn test_proposal_with_multiple_steps_succeed_in_one_call() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create a proposal with multiple steps
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let proposal_bucket_return = helper.add_normal_proposal_step(proposal_bucket)?;

    // Submit the proposal and vote
    let proposal_bucket_return_2 = helper.submit_proposal(proposal_bucket_return)?;
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days (end of voting period)
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute all steps in one call
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 2)?;

    // Successfully retrieve fee
    let _ = helper.retrieve_fee(proposal_bucket_return_2)?;

    Ok(())
}

#[test]
pub fn test_hurried_proposal() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create a proposal with multiple steps
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;

    // Submit the proposal and vote
    let proposal_bucket_return_2 = helper.submit_proposal(proposal_bucket)?;
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;
    let _ = helper.hurry_proposal(0, 1)?;

    // Advance time by 1 day (end of voting period due to hurry)
    let new_time_1 = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute all steps in one call
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 1)?;

    // Successfully retrieve fee
    let _ = helper.retrieve_fee(proposal_bucket_return_2)?;

    Ok(())
}

#[test]
pub fn test_proposal_with_multiple_steps_succeed_in_one_call_overshoot() -> Result<(), RuntimeError>
{
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create a proposal with multiple steps
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let proposal_bucket_return = helper.add_normal_proposal_step(proposal_bucket)?;

    // Submit the proposal and vote
    let proposal_bucket_return_2 = helper.submit_proposal(proposal_bucket_return)?;
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days (end of voting period)
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute all steps with overshoot
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 3)?; // Overshoot: only 2 steps exist

    // Successfully retrieve fee
    let _ = helper.retrieve_fee(proposal_bucket_return_2)?;

    Ok(())
}

#[test]
pub fn test_proposal_with_multiple_steps_succeed_in_individual_calls() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create a proposal with multiple steps
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let proposal_bucket_return = helper.add_normal_proposal_step(proposal_bucket)?;

    // Submit the proposal and vote
    let proposal_bucket_return_2 = helper.submit_proposal(proposal_bucket_return)?;
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days (end of voting period)
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute steps individually
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 1)?;
    let _ = helper.execute_proposal_step(0, 1)?;

    // Successfully retrieve fee
    let _ = helper.retrieve_fee(proposal_bucket_return_2)?;

    Ok(())
}

#[test]
fn test_proposal_deadline_set_at_submission() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;

    // Advance time by 3 days before submitting the proposal
    let new_time_1 = helper.env.get_current_time().add_days(3).unwrap();
    helper.env.set_current_time(new_time_1);

    // Submit the proposal and vote
    let _ = helper.submit_proposal(proposal_bucket)?;
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 6 more days (9 days total from original time)
    let new_time_2 = helper.env.get_current_time().add_days(6).unwrap();
    helper.env.set_current_time(new_time_2);

    // Attempt to finish voting (should fail as deadline is set from submission time)
    let failure = helper.finish_voting(0);

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_pool_to_real_for_voting() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(5000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Advance time by 1 day and update rewards
    let new_time_1 = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(new_time_1);
    let _ = helper.rewarded_update()?;

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // Vote on the proposal
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days (end of voting period)
    let new_time_2 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_2);

    // Finish voting and execute the proposal
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 1)?;

    Ok(())
}

#[test]
fn test_reentrancy_step_in_middle_of_proposal_fail_to_end() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create a proposal with multiple steps including a reentrancy step
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let proposal_bucket_return = helper.add_reentrancy_proposal_step(proposal_bucket)?;
    let proposal_bucket_return_2 = helper.add_normal_proposal_step(proposal_bucket_return)?;
    let _proposal_bucket_return_3 = helper.submit_proposal(proposal_bucket_return_2)?;

    // Vote on the proposal
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days (end of voting period)
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute all steps
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 3)?;

    // Attempt to finish voting again (should fail due to reentrancy)
    let failure = helper.finish_voting(0);

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_reentrancy_step_in_middle_of_proposal_fail_execute_while_reentrancy_is_true(
) -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create a proposal with multiple steps including a reentrancy step
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let proposal_bucket_return = helper.add_reentrancy_proposal_step(proposal_bucket)?;
    let proposal_bucket_return_2 = helper.add_normal_proposal_step(proposal_bucket_return)?;
    let _proposal_bucket_return_3 = helper.submit_proposal(proposal_bucket_return_2)?;

    // Vote on the proposal
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days (end of voting period)
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute all steps
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 3)?;

    // Attempt to execute a step while reentrancy is true (should fail)
    let failure = helper.execute_proposal_step(0, 1);

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_reentrancy_step_in_middle_of_proposal_succeed_execute() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake tokens for a single voter
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    // Create a proposal with multiple steps including a reentrancy step
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let proposal_bucket_return = helper.add_reentrancy_proposal_step(proposal_bucket)?;
    let proposal_bucket_return_2 = helper.add_normal_proposal_step(proposal_bucket_return)?;
    let proposal_bucket_return_3 = helper.submit_proposal(proposal_bucket_return_2)?;

    // Vote on the proposal
    let _ = helper.vote_on_proposal(true, stake_id, 0)?;

    // Advance time by 7 days (end of voting period)
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute steps
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 2)?;
    let _ = helper.execute_reentrancy(0)?;

    // Verify that new proposals can be created cheaper (for 5000 instead of 10000) after reentrancy is executed
    let _ = helper.create_basic_proposal(dec!(5000))?;

    // Execute remaining step and retrieve fee
    let _ = helper.execute_proposal_step(0, 1)?;
    let _ = helper.retrieve_fee(proposal_bucket_return_3)?;

    Ok(())
}

#[test]
fn test_delegate_and_vote_and_unstake_immediately_fail() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // Stake tokens for two voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    let bucket_2 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();

    // Delegate vote, vote, and then undelegate
    let stake_id_returned = helper.delegate_vote(stake_id, NonFungibleLocalId::integer(2))?;
    let _ = helper.vote_on_proposal(true, stake_id_2, 0)?;
    let stake_id_returned_2 = helper.undelegate_vote(stake_id_returned)?;

    // Attempt to unstake immediately (should fail)
    let failure = helper.start_unstake(stake_id_returned_2, dec!(5000));

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_delegate_and_vote_and_unstake_succeed() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // Stake tokens for two voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    let bucket_2 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();

    // Delegate vote, vote, and then undelegate
    let stake_id_returned = helper.delegate_vote(stake_id, NonFungibleLocalId::integer(2))?;
    let _ = helper.vote_on_proposal(true, stake_id_2, 0)?;
    let stake_id_returned_2 = helper.undelegate_vote(stake_id_returned)?;

    // Advance time by 8 days
    let new_time_1 = helper.env.get_current_time().add_days(8).unwrap();
    helper.env.set_current_time(new_time_1);

    // Attempt to unstake (should succeed)
    let _ = helper.start_unstake(stake_id_returned_2, dec!(5000))?;

    Ok(())
}

#[test]
fn test_delegate_and_vote_and_unstake_succeed_after_voting_period() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // Stake tokens for two voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    let bucket_2 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();

    // Delegate vote and vote
    let stake_id_returned = helper.delegate_vote(stake_id, NonFungibleLocalId::integer(2))?;
    let _ = helper.vote_on_proposal(true, stake_id_2, 0)?;

    // Advance time by 8 days (past voting period)
    let new_time_1 = helper.env.get_current_time().add_days(8).unwrap();
    helper.env.set_current_time(new_time_1);

    // Undelegate and unstake (should succeed)
    let stake_id_returned_2 = helper.undelegate_vote(stake_id_returned)?;
    let _ = helper.start_unstake(stake_id_returned_2, dec!(5000))?;

    Ok(())
}

#[test]
fn test_delegate_and_vote_not_allowed() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // Stake tokens for two voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    let bucket_2 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let _stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();

    // Delegate vote
    let stake_id_returned = helper.delegate_vote(stake_id, NonFungibleLocalId::integer(2))?;

    // Attempt to vote with delegated stake (should fail)
    let failure = helper.vote_on_proposal(true, stake_id_returned, 0);

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_delegate_and_win_vote_through_delegation() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // Stake tokens for three voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    let bucket_2 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();

    let bucket_3 = helper.ilis.take(dec!(15000), &mut helper.env)?;
    let stake_id_3 = helper.stake_without_id(bucket_3)?.0.unwrap();

    // Delegate vote and vote
    let _stake_id_returned = helper.delegate_vote(stake_id, NonFungibleLocalId::integer(2))?;
    let _ = helper.vote_on_proposal(true, stake_id_2, 0)?;
    let _ = helper.vote_on_proposal(false, stake_id_3, 0)?;

    // Advance time by 8 days
    let new_time_1 = helper.env.get_current_time().add_days(8).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute proposal (should succeed due to delegation)
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 1)?;

    Ok(())
}

#[test]
fn test_delegate_and_stake_extra_win_vote_through_delegation() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Create and submit a proposal
    let (_bucket_return_payment, proposal_bucket) = helper.create_basic_proposal(dec!(10000))?;
    let _ = helper.submit_proposal(proposal_bucket)?;

    // Stake tokens for three voters
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_without_id(bucket_1)?.0.unwrap();

    let bucket_2 = helper.ilis.take(dec!(3000), &mut helper.env)?;
    let stake_id_2 = helper.stake_without_id(bucket_2)?.0.unwrap();

    let bucket_3 = helper.ilis.take(dec!(15000), &mut helper.env)?;
    let stake_id_3 = helper.stake_without_id(bucket_3)?.0.unwrap();

    // Delegate vote and stake extra tokens
    let bucket_4 = helper.ilis.take(dec!(4000), &mut helper.env)?;
    let stake_id_returned = helper.delegate_vote(stake_id, NonFungibleLocalId::integer(2))?;
    let _stake_id_returned_2 = helper.stake_with_id(bucket_4, stake_id_returned)?;

    // Vote
    let _ = helper.vote_on_proposal(true, stake_id_2, 0)?;
    let _ = helper.vote_on_proposal(false, stake_id_3, 0)?;

    // Advance time by 8 days
    let new_time_1 = helper.env.get_current_time().add_days(8).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish voting and execute proposal (should succeed due to delegation and extra stake)
    let _ = helper.finish_voting(0)?;
    let _ = helper.execute_proposal_step(0, 1)?;

    Ok(())
}
