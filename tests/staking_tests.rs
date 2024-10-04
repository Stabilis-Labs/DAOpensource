mod helper;
use helper::Helper;

use scrypto_test::prelude::*;

#[test]
fn test_stake_without_and_with_id() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens without an ID
    let stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(stake_bucket)?;
    let stake_id_bucket = result.0.unwrap();
    let id_data_1 = helper.get_member_data(NonFungibleLocalId::integer(1))?;

    // Assert the staked amount and resource address
    assert_eq!(id_data_1.pool_amount_staked, dec!(10000));
    assert_eq!(
        helper.staking_id_address,
        stake_id_bucket.resource_address(&mut helper.env)?
    );

    // Stake another 10000 tokens with the existing ID
    let new_stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let _result_2 = helper.stake_with_id(new_stake_bucket, stake_id_bucket)?;
    let id_data_2 = helper.get_member_data(NonFungibleLocalId::integer(1))?;

    // Assert the total staked amount has increased
    assert_eq!(id_data_2.pool_amount_staked, dec!(20000));

    Ok(())
}

#[test]
fn test_stake_and_unstake_with_id() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens
    let stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(stake_bucket)?;

    // Unstake 5000 tokens
    let (unstake_receipt_1, stake_id_1) = helper.start_unstake(result.0.unwrap(), dec!(5000))?;
    let id_data_2 = helper.get_member_data(NonFungibleLocalId::integer(1))?;

    // Assert the remaining staked amount
    assert_eq!(id_data_2.pool_amount_staked, dec!(5000));

    // Unstake another 1000 tokens
    let (_unstake_receipt_2, stake_id_2) = helper.start_unstake(stake_id_1, dec!(1000))?;

    // Unstake the remaining 4000 tokens
    let (unstake_receipt_3, _) = helper.start_unstake(stake_id_2, dec!(6000))?;
    let id_data_3 = helper.get_member_data(NonFungibleLocalId::integer(1))?;

    // Assert that no tokens are left staked
    assert_eq!(id_data_3.pool_amount_staked, dec!(0));

    // Advance time by 7 days
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish unstaking and assert the returned amounts
    let unstaked_bucket_1 = helper.finish_unstake(unstake_receipt_1)?;
    let unstaked_bucket_2 = helper.finish_unstake(unstake_receipt_3)?;

    helper.assert_bucket_eq(&unstaked_bucket_1, helper.ilis_address, dec!(5000))?;
    helper.assert_bucket_eq(&unstaked_bucket_2, helper.ilis_address, dec!(4000))?;

    Ok(())
}

#[test]
fn test_unstake_before_time() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens
    let stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(stake_bucket)?;

    // Start unstaking 5000 tokens
    let (unstake_receipt_1, _stake_id_1) = helper.start_unstake(result.0.unwrap(), dec!(5000))?;

    // Attempt to finish unstaking immediately (should fail)
    let unstaked_bucket_fail = helper.finish_unstake(unstake_receipt_1);

    assert!(unstaked_bucket_fail.is_err());

    Ok(())
}

#[test]
fn test_transfer_stake() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens
    let stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(stake_bucket)?;

    // Transfer 4000 tokens to a new stake
    let (transfer_receipt, _stake_id) =
        helper.start_unstake_transfer(result.0.unwrap(), dec!(4000))?;

    let _result_2 = helper.stake_without_id(transfer_receipt)?;

    // Assert the amounts in both stakes
    let id_data_1 = helper.get_member_data(NonFungibleLocalId::integer(1))?;
    assert_eq!(id_data_1.pool_amount_staked, dec!(6000));

    let id_data_2 = helper.get_member_data(NonFungibleLocalId::integer(2))?;
    assert_eq!(id_data_2.pool_amount_staked, dec!(4000));

    Ok(())
}

#[test]
fn test_staking_rewards() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let _ = helper.stake_without_id(bucket_1)?;

    // Advance time by 1 day
    let new_time_1 = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(new_time_1);

    // Update rewards
    let _ = helper.rewarded_update()?;

    // Assert the reward amount
    let amount = helper.get_real_amount()?;
    assert_eq!(amount, dec!(2));

    // Stake another 10000 tokens
    let bucket_2 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let _ = helper.stake_without_id(bucket_2)?;

    // Assert the staked amount for the second stake
    let member_data = helper.get_member_data(NonFungibleLocalId::integer(2))?;
    assert_eq!(member_data.pool_amount_staked, dec!(5000));

    Ok(())
}

#[test]
fn test_locking() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(bucket_1)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let returned_stake_id = helper.lock_stake(stake_id, 10, true)?;

    // Assert the locked amount and duration
    let member_data = helper.get_member_data(NonFungibleLocalId::integer(1))?;
    assert!(member_data.pool_amount_staked > dec!(10100));
    assert!(member_data.pool_amount_staked < dec!(10101));
    assert_eq!(
        member_data.locked_until.unwrap(),
        helper.env.get_current_time().add_days(10).unwrap()
    );

    // Lock the stake for another 10 days
    let _ = helper.lock_stake(returned_stake_id, 10, true)?;

    // Assert the updated locked amount and duration
    let member_data = helper.get_member_data(NonFungibleLocalId::integer(1))?;
    assert!(member_data.pool_amount_staked > dec!(10201));
    assert!(member_data.pool_amount_staked < dec!(10202));
    assert_eq!(
        member_data.locked_until.unwrap(),
        helper.env.get_current_time().add_days(20).unwrap()
    );

    Ok(())
}

#[test]
fn test_lock_too_long() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(bucket_1)?;

    let stake_id = result.0.unwrap();

    // Attempt to lock the stake for more than 365 days (should fail)
    let failure = helper.lock_stake(stake_id, 366, true);

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_lock_and_unstake() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(bucket_1)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let returned_stake_id = helper.lock_stake(stake_id, 10, true)?;

    // Advance time by 10 days
    let new_time_1 = helper.env.get_current_time().add_days(10).unwrap();
    helper.env.set_current_time(new_time_1);

    // Unstake 5000 tokens (should succeed)
    let _result = helper.start_unstake(returned_stake_id, dec!(5000))?;

    Ok(())
}

#[test]
fn test_lock_and_unstake_too_early() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(bucket_1)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let returned_stake_id = helper.lock_stake(stake_id, 10, true)?;

    // Attempt to unstake immediately (should fail)
    let failure = helper.start_unstake(returned_stake_id, dec!(5000));

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_lock_and_unlock_too_far() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens and prepare 1000 tokens for payment
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(bucket_1)?;
    let payment_bucket = helper.ilis.take(dec!(1000), &mut helper.env)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let returned_stake_id = helper.lock_stake(stake_id, 10, true)?;

    // Attempt to unlock for 12 days (should fail)
    let failure = helper.unlock_stake(returned_stake_id, payment_bucket, 12);

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_unlock_too_early() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens and prepare 1000 tokens for payment
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(bucket_1)?;
    let payment_bucket = helper.ilis.take(dec!(1000), &mut helper.env)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let returned_stake_id = helper.lock_stake(stake_id, 10, true)?;

    // Unlock the stake for 5 days
    let (returned_stake_id_2, _leftover_payment) =
        helper.unlock_stake(returned_stake_id, payment_bucket, 5)?;

    // Attempt to unstake immediately (should fail)
    let failed_unstake = helper.start_unstake(returned_stake_id_2, dec!(5000));

    assert!(failed_unstake.is_err());

    Ok(())
}

#[test]
fn test_unlock_to_unstake_partial_pay_off() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens and prepare 1000 tokens for payment
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_without_id(bucket_1)?;
    let payment_bucket = helper.ilis.take(dec!(1000), &mut helper.env)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let returned_stake_id = helper.lock_stake(stake_id, 10, true)?;

    // Unlock the stake for 5 days
    let (returned_stake_id_2, _leftover_payment) =
        helper.unlock_stake(returned_stake_id, payment_bucket, 5)?;

    // Advance time by 5 days
    let new_time_1 = helper.env.get_current_time().add_days(5).unwrap();
    helper.env.set_current_time(new_time_1);

    // Unstake 5000 tokens (should succeed)
    let _ = helper.start_unstake(returned_stake_id_2, dec!(5000))?;

    Ok(())
}

#[test]
fn test_delegate_and_undelegate() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens for two different stakes
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result_1 = helper.stake_without_id(bucket_1)?;

    let stake_id_1 = result_1.0.unwrap();

    let bucket_2 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let _result_2 = helper.stake_without_id(bucket_2)?;

    // Delegate voting power from stake 1 to stake 2
    let returned_stake_id = helper.delegate_vote(stake_id_1, NonFungibleLocalId::integer(2))?;

    // Assert the delegation
    let member_data_1 = helper.get_member_data(NonFungibleLocalId::integer(1))?;
    let member_data_2 = helper.get_member_data(NonFungibleLocalId::integer(2))?;

    assert_eq!(member_data_1.pool_amount_delegated_to_me, dec!(0));
    assert_eq!(
        member_data_1.delegating_voting_power_to,
        Some(NonFungibleLocalId::integer(2))
    );
    assert_eq!(member_data_2.pool_amount_delegated_to_me, dec!(10000));

    // Undelegate voting power
    let _ = helper.undelegate_vote(returned_stake_id)?;

    // Assert the undelegation
    let member_data_3 = helper.get_member_data(NonFungibleLocalId::integer(1))?;
    let member_data_4 = helper.get_member_data(NonFungibleLocalId::integer(2))?;

    assert_eq!(member_data_3.delegating_voting_power_to, None);
    assert_eq!(member_data_4.pool_amount_delegated_to_me, dec!(0));

    Ok(())
}

#[test]
fn test_delegate_and_fail_unstake() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Stake 10000 tokens for two different stakes
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result_1 = helper.stake_without_id(bucket_1)?;

    let stake_id_1 = result_1.0.unwrap();

    let bucket_2 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let _result_2 = helper.stake_without_id(bucket_2)?;

    // Delegate voting power from stake 1 to stake 2
    let returned_stake_id = helper.delegate_vote(stake_id_1, NonFungibleLocalId::integer(2))?;

    // Attempt to unstake from the delegated stake (should fail)
    let failed_unstake = helper.start_unstake(returned_stake_id, dec!(5000));

    assert!(failed_unstake.is_err());

    Ok(())
}
