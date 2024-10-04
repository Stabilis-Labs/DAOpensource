mod helper;
use helper::Helper;

use scrypto_test::prelude::*;

#[test]
fn test_bootstrap_start() -> Result<(), RuntimeError> {
    // Initialize a new helper instance
    let mut helper = Helper::new().unwrap();

    // Attempt to start the bootstrap process
    let _ = helper.start_bootstrap()?;

    Ok(())
}

#[test]
fn test_bootstrap_lifetime() -> Result<(), RuntimeError> {
    // Initialize a new helper instance
    let mut helper = Helper::new().unwrap();

    // Create buckets for testing
    let xrd_bucket = helper.xrd.take(dec!(1), &mut helper.env)?;
    let xrd_bucket_2 = helper.xrd.take(dec!(1), &mut helper.env)?;
    let xrd_bucket_3 = helper.xrd.take(dec!(1), &mut helper.env)?;
    let boot_bucket = helper.boot.take(dec!(1), &mut helper.env)?;

    // Start the bootstrap process
    let _ = helper.start_bootstrap()?;

    // Perform initial swap
    let bucket = helper.bootstrap_swap(xrd_bucket)?;

    // Advance time by 5 days
    let new_time = helper.env.get_current_time().add_days(5).unwrap();
    helper.env.set_current_time(new_time);

    // Perform second swap
    let bucket_2 = helper.bootstrap_swap(xrd_bucket_2)?;

    // Assert that the second swap yielded more tokens than the first
    assert!(bucket.amount(&mut helper.env)? < bucket_2.amount(&mut helper.env)?);

    // Advance time by another 5 days
    let new_time2 = helper.env.get_current_time().add_days(5).unwrap();
    helper.env.set_current_time(new_time2);

    // Finish the bootstrap process
    let _ = helper.finish_bootstrap()?;

    // Reclaim initial bootstrap funds
    let retrieved_initial = helper.reclaim_bootstrap_initial(boot_bucket)?;

    // Assert that the retrieved amount is correct
    let _ = helper.assert_bucket_eq(&retrieved_initial, helper.xrd_address, dec!(500))?;

    // Attempt to swap after bootstrap has finished (should fail)
    let bucket3_result = helper.bootstrap_swap(xrd_bucket_3);
    assert!(bucket3_result.is_err());

    Ok(())
}
