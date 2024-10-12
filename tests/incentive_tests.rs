mod helper;
use helper::Helper;

use scrypto_test::prelude::*;

#[test]
fn test_incentives_stake_without_and_with_id() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens without an ID
    let stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(stake_bucket)?;
    let stake_id_bucket = result.0.unwrap();
    let id_data_1 = helper.get_incentive_data(NonFungibleLocalId::integer(1))?;

    // Assert that the staked amount is correct
    assert_eq!(
        id_data_1
            .resources
            .get(&helper.ilis_address)
            .unwrap()
            .amount_staked,
        dec!(10000)
    );

    // Assert that the stake ID resource address is correct
    assert_eq!(
        helper.incentives_id_address,
        stake_id_bucket.resource_address(&mut helper.env)?
    );

    // Stake an additional 10000 tokens with the existing ID
    let new_stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let _result_2 = helper.stake_incentives_with_id(new_stake_bucket, stake_id_bucket)?;
    let id_data_2 = helper.get_incentive_data(NonFungibleLocalId::integer(1))?;

    // Assert that the total staked amount is now 20000
    assert_eq!(
        id_data_2
            .resources
            .get(&helper.ilis_address)
            .unwrap()
            .amount_staked,
        dec!(20000)
    );

    Ok(())
}

#[test]
fn test_incentives_stake_and_unstake_with_id() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens
    let stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(stake_bucket)?;

    // Unstake 5000 tokens
    let (unstake_receipt_1, stake_id_1) =
        helper.start_incentives_unstake(helper.ilis_address, result.0.unwrap(), dec!(5000))?;
    let id_data_2 = helper.get_incentive_data(NonFungibleLocalId::integer(1))?;

    // Assert that 5000 tokens remain staked
    assert_eq!(
        id_data_2
            .resources
            .get(&helper.ilis_address)
            .unwrap()
            .amount_staked,
        dec!(5000)
    );

    // Unstake 1000 more tokens
    let (_unstake_receipt_2, stake_id_2) =
        helper.start_incentives_unstake(helper.ilis_address, stake_id_1, dec!(1000))?;

    // Attempt to unstake the remaining 4000 tokens (which should succeed)
    let (unstake_receipt_3, _) =
        helper.start_incentives_unstake(helper.ilis_address, stake_id_2, dec!(6000))?;
    let id_data_3 = helper.get_incentive_data(NonFungibleLocalId::integer(1))?;

    // Assert that no tokens remain staked
    assert_eq!(
        id_data_3
            .resources
            .get(&helper.ilis_address)
            .unwrap()
            .amount_staked,
        dec!(0)
    );

    // Advance time by 7 days
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);

    // Finish unstaking and verify the returned amounts
    let unstaked_bucket_1 = helper.finish_incentives_unstake(unstake_receipt_1)?;
    let unstaked_bucket_2 = helper.finish_incentives_unstake(unstake_receipt_3)?;

    helper.assert_bucket_eq(&unstaked_bucket_1, helper.ilis_address, dec!(5000))?;
    helper.assert_bucket_eq(&unstaked_bucket_2, helper.ilis_address, dec!(4000))?;

    Ok(())
}

#[test]
fn test_incentives_unstake_before_time() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens
    let stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(stake_bucket)?;

    // Start unstaking 5000 tokens
    let (unstake_receipt_1, _stake_id_1) =
        helper.start_incentives_unstake(helper.ilis_address, result.0.unwrap(), dec!(5000))?;

    // Attempt to finish unstaking immediately (should fail)
    let unstaked_bucket_fail = helper.finish_incentives_unstake(unstake_receipt_1);

    assert!(unstaked_bucket_fail.is_err());

    Ok(())
}

#[test]
fn test_transfer_incentives_stake() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens
    let stake_bucket = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(stake_bucket)?;

    // Transfer 4000 tokens to a new stake
    let (transfer_receipt, _stake_id) = helper.start_incentives_unstake_transfer(
        helper.ilis_address,
        result.0.unwrap(),
        dec!(4000),
    )?;

    // Stake the transferred tokens
    let _result_2 = helper.stake_incentives_without_id(transfer_receipt)?;

    // Verify the amounts in both stakes
    let id_data_1 = helper.get_incentive_data(NonFungibleLocalId::integer(1))?;
    assert_eq!(
        id_data_1
            .resources
            .get(&helper.ilis_address)
            .unwrap()
            .amount_staked,
        dec!(6000)
    );

    let id_data_2 = helper.get_incentive_data(NonFungibleLocalId::integer(2))?;
    assert_eq!(
        id_data_2
            .resources
            .get(&helper.ilis_address)
            .unwrap()
            .amount_staked,
        dec!(4000)
    );

    Ok(())
}

#[test]
fn test_incentives_staking_rewards() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let stake_id = helper.stake_incentives_without_id(bucket_1)?.0.unwrap();

    // Advance time by 7 days and update rewards
    let new_time_1 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_1);
    let _ = helper.rewarded_update()?;

    // Check rewards for the first stake
    let (stake_id_returned, rewards) = helper.update_incentives_id(stake_id)?;
    helper.assert_bucket_eq(&rewards, helper.ilis_address, dec!(10000))?;

    // Stake 40000 more tokens
    let bucket_2 = helper.ilis.take(dec!(40000), &mut helper.env)?;
    let stake_id_2 = helper.stake_incentives_without_id(bucket_2)?.0.unwrap();

    // Advance time by 7 days and update rewards
    let new_time_2 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_2);
    let _ = helper.rewarded_update()?;

    // Check rewards for the second stake
    let (stake_id_2_returned, rewards) = helper.update_incentives_id(stake_id_2)?;
    helper.assert_bucket_eq(&rewards, helper.ilis_address, dec!(8000))?;

    // Advance time by 7 days and update rewards
    let new_time_3 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_3);
    let _ = helper.rewarded_update()?;

    // Do nothing (simulating no claim)

    // Advance time by 7 days and update rewards
    let new_time_4 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_4);
    let _ = helper.rewarded_update()?;

    // Check rewards for the second stake (should be double due to unclaimed previous period)
    let (stake_id_2_returned, rewards) = helper.update_incentives_id(stake_id_2_returned)?;
    helper.assert_bucket_eq(&rewards, helper.ilis_address, dec!(16000))?;

    // Advance time by 7 days and update rewards
    let new_time_5 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_5);
    let _ = helper.rewarded_update()?;

    // Check rewards for the second stake
    let (_stake_id_2_returned, rewards) = helper.update_incentives_id(stake_id_2_returned)?;
    helper.assert_bucket_eq(&rewards, helper.ilis_address, dec!(8000))?;

    // Advance time by 7 days and update rewards
    let new_time_6 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_6);
    let _ = helper.rewarded_update()?;

    // Do nothing (simulating no claim)

    // Advance time by 7 days and update rewards
    let new_time_7 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_7);
    let _ = helper.rewarded_update()?;

    // Check rewards for the first stake (should be a max of 5 periods, even though 7 periods have passed without claim, due to the max claim delay of 5 periods)
    let (_stake_id_returned, rewards) = helper.update_incentives_id(stake_id_returned)?;
    helper.assert_bucket_eq(&rewards, helper.ilis_address, dec!(10000))?;

    Ok(())
}

#[test]
fn test_incentives_locking() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(bucket_1)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let (returned_stake_id, rewards) =
        helper.lock_incentives_stake(helper.ilis_address, stake_id, 10)?;

    // Check the locked status and rewards
    let member_data = helper.get_incentive_data(NonFungibleLocalId::integer(1))?;
    assert!(rewards.amount(&mut helper.env)? > dec!(100));
    assert!(rewards.amount(&mut helper.env)? < dec!(101));
    assert_eq!(
        member_data
            .resources
            .get(&helper.ilis_address)
            .unwrap()
            .locked_until
            .unwrap(),
        helper.env.get_current_time().add_days(10).unwrap()
    );

    // Lock the stake for another 10 days
    let _ = helper.lock_incentives_stake(helper.ilis_address, returned_stake_id, 10)?;

    // Check the updated locked status and rewards
    let member_data = helper.get_incentive_data(NonFungibleLocalId::integer(1))?;
    assert!(rewards.amount(&mut helper.env)? > dec!(100));
    assert!(rewards.amount(&mut helper.env)? < dec!(101));
    assert_eq!(
        member_data
            .resources
            .get(&helper.ilis_address)
            .unwrap()
            .locked_until
            .unwrap(),
        helper.env.get_current_time().add_days(20).unwrap()
    );

    Ok(())
}

#[test]
fn test_incentives_lock_too_long() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(bucket_1)?;

    let stake_id = result.0.unwrap();

    // Attempt to lock the stake for longer than the maximum allowed period (should fail)
    let failure = helper.lock_incentives_stake(helper.ilis_address, stake_id, 366);

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_incentives_lock_and_unstake() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(bucket_1)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let (returned_stake_id, _rewards) =
        helper.lock_incentives_stake(helper.ilis_address, stake_id, 10)?;

    // Advance time by 10 days
    let new_time_1 = helper.env.get_current_time().add_days(10).unwrap();
    helper.env.set_current_time(new_time_1);

    // Attempt to unstake 5000 tokens (should succeed)
    let _result =
        helper.start_incentives_unstake(helper.ilis_address, returned_stake_id, dec!(5000))?;

    Ok(())
}

#[test]
fn test_lock_and_unstake_too_early_incentives() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(bucket_1)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let (returned_stake_id, _rewards) =
        helper.lock_incentives_stake(helper.ilis_address, stake_id, 10)?;

    // Attempt to unstake 5000 tokens immediately (should fail)
    let failure =
        helper.start_incentives_unstake(helper.ilis_address, returned_stake_id, dec!(5000));

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_lock_and_unlock_too_far_incentives() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens and prepare 1000 tokens for payment
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(bucket_1)?;
    let payment_bucket = helper.ilis.take(dec!(1000), &mut helper.env)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let (returned_stake_id, _rewards) =
        helper.lock_incentives_stake(helper.ilis_address, stake_id, 10)?;

    // Attempt to unlock the stake for 12 days (should fail as it's longer than the lock period)
    let failure =
        helper.unlock_incentives_stake(helper.ilis_address, returned_stake_id, payment_bucket, 12);

    assert!(failure.is_err());

    Ok(())
}

#[test]
fn test_incentives_unlock_too_early() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens and prepare 1000 tokens for payment
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(bucket_1)?;
    let payment_bucket = helper.ilis.take(dec!(1000), &mut helper.env)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let (returned_stake_id, _rewards) =
        helper.lock_incentives_stake(helper.ilis_address, stake_id, 10)?;

    // Unlock the stake for 5 days
    let (returned_stake_id_2, _leftover_payment) = helper.unlock_incentives_stake(
        helper.ilis_address,
        returned_stake_id,
        payment_bucket,
        5,
    )?;

    // Attempt to unstake 5000 tokens immediately (should fail)
    let failed_unstake =
        helper.start_incentives_unstake(helper.ilis_address, returned_stake_id_2, dec!(5000));

    assert!(failed_unstake.is_err());

    Ok(())
}

#[test]
fn test_incentives_unlock_to_unstake_partial_pay_off() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource with specific parameters
    let _ = helper.add_stakable(helper.ilis_address, dec!(10000), dec!(1.001), 365, dec!(1.002))?;
    helper.env.enable_auth_module();

    // Stake 10000 tokens and prepare 1000 tokens for payment
    let bucket_1 = helper.ilis.take(dec!(10000), &mut helper.env)?;
    let result = helper.stake_incentives_without_id(bucket_1)?;
    let payment_bucket = helper.ilis.take(dec!(1000), &mut helper.env)?;

    let stake_id = result.0.unwrap();

    // Lock the stake for 10 days
    let (returned_stake_id, _rewards) =
        helper.lock_incentives_stake(helper.ilis_address, stake_id, 10)?;

    // Unlock the stake for 5 days
    let (returned_stake_id_2, _leftover_payment) = helper.unlock_incentives_stake(
        helper.ilis_address,
        returned_stake_id,
        payment_bucket,
        5,
    )?;

    // Advance time by 5 days
    let new_time_1 = helper.env.get_current_time().add_days(5).unwrap();
    helper.env.set_current_time(new_time_1);

    // Attempt to unstake 5000 tokens (should succeed)
    let _ =
        helper.start_incentives_unstake(helper.ilis_address, returned_stake_id_2, dec!(5000))?;

    Ok(())
}
