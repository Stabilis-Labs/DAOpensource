mod helper;
use helper::Helper;

use scrypto::prelude::ResourceSpecifier;
use scrypto_test::prelude::*;

#[test]
fn test_dao_put_tokens() -> Result<(), RuntimeError> {
    // Initialize helper and create a bucket of XRD tokens
    let mut helper = Helper::new().unwrap();
    let bucket = helper.xrd.take(dec!(1000), &mut helper.env)?;

    // Put tokens into the DAO
    let _ = helper.dao_put_tokens(bucket)?;

    // Check if the DAO received the correct amount of tokens
    let amount = helper.dao_get_token_amount(helper.xrd_address)?;
    assert_eq!(amount, dec!(1000));

    Ok(())
}

#[test]
fn test_dao_send_tokens() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Prepare token sending parameters
    let specifier: ResourceSpecifier = ResourceSpecifier::Fungible(dec!(1000));
    let address: ResourceAddress = helper.ilis_address;
    let recipient: ComponentAddress = ComponentAddress::try_from(helper.staking.0.clone()).unwrap();

    // Send tokens from DAO to the staking component
    let _ = helper.dao_send_tokens(address, specifier, recipient)?;

    // Check remaining amount in DAO and new amount in staking component
    let remaining_amount = helper.dao_get_token_amount(address)?;
    let new_staking_amount = helper.get_remaining_staking_rewards()?;

    // Assert the correct amounts after token transfer
    assert_eq!(remaining_amount, dec!(299000));
    assert_eq!(new_staking_amount, dec!(51000));

    Ok(())
}

#[test]
fn test_dao_take_tokens() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Prepare token taking parameters
    let specifier: ResourceSpecifier = ResourceSpecifier::Fungible(dec!(1000));
    let address: ResourceAddress = helper.ilis_address;

    // Take tokens from the DAO
    let bucket = helper.dao_take_tokens(address, specifier)?;

    // Check remaining amount in DAO
    let remaining_amount = helper.dao_get_token_amount(address)?;

    // Assert the correct remaining amount and taken amount
    assert_eq!(remaining_amount, dec!(299000));
    let _ = helper.assert_bucket_eq(&bucket, helper.ilis_address, dec!(1000))?;

    Ok(())
}

#[test]
fn test_airdrop_membered_token() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();
    let mut map: IndexMap<Reference, Decimal> = IndexMap::new();

    // Create two accounts and assign airdrop amounts
    let account_1: Reference = helper.create_account()?;
    map.insert(account_1, dec!(1000));

    let account_2: Reference = helper.create_account()?;
    map.insert(account_2, dec!(2000));

    // Perform the airdrop
    let _ = helper.airdrop_membered_tokens(map, 0, 0)?;

    // Withdraw NFTs from accounts
    let _airdrop_1 = helper.withdraw_nft_from_account(
        account_1,
        helper.staking_id_address,
        NonFungibleLocalId::integer(1),
    )?;
    let _airdrop_2 = helper.withdraw_nft_from_account(
        account_2,
        helper.staking_id_address,
        NonFungibleLocalId::integer(2),
    )?;

    // Get member data for both accounts
    let id_data1 = helper.get_member_data(NonFungibleLocalId::integer(1))?;
    let id_data2 = helper.get_member_data(NonFungibleLocalId::integer(2))?;

    // Assert correct staked amounts
    assert_eq!(id_data1.pool_amount_staked, dec!(1000));
    assert_eq!(id_data2.pool_amount_staked, dec!(2000));

    Ok(())
}

#[test]
fn test_airdrop_locked_voting_membered_token() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();
    let mut map: IndexMap<Reference, Decimal> = IndexMap::new();

    // Create an account and assign airdrop amount
    let account_1: Reference = helper.create_account()?;
    map.insert(account_1, dec!(1000));

    // Perform the airdrop with locking and voting periods
    let _ = helper.airdrop_membered_tokens(map, 5, 4)?;

    // Withdraw NFT from account
    let _airdrop_1 = helper.withdraw_nft_from_account(
        account_1,
        helper.staking_id_address,
        NonFungibleLocalId::integer(1),
    )?;

    // Get member data
    let id_data1 = helper.get_member_data(NonFungibleLocalId::integer(1))?;

    // Calculate expected unlock and unvote times
    let time_when_unlockable = helper.env.get_current_time().add_days(5).unwrap();
    let time_when_unvotable = helper.env.get_current_time().add_days(4).unwrap();

    // Assert correct locking and voting periods
    assert_eq!(id_data1.voting_until.unwrap(), time_when_unvotable);
    assert_eq!(id_data1.locked_until.unwrap(), time_when_unlockable);

    Ok(())
}

#[test]
fn test_airdrop_staked_token() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource
    helper.add_stakable(helper.ilis_address, dec!(100), dec!("1.01"), 365, dec!(3))?;

    let mut map: IndexMap<Reference, Decimal> = IndexMap::new();

    // Create two accounts and assign airdrop amounts
    let account_1: Reference = helper.create_account()?;
    map.insert(account_1, dec!(1000));

    let account_2: Reference = helper.create_account()?;
    map.insert(account_2, dec!(2000));

    // Perform the airdrop of staked tokens
    let _ = helper.airdrop_staked_tokens(map, helper.ilis_address, 0, 0)?;

    // Withdraw NFTs from accounts
    let _airdrop_1 = helper.withdraw_nft_from_account(
        account_1,
        helper.incentives_id_address,
        NonFungibleLocalId::integer(1),
    )?;
    let _airdrop_2 = helper.withdraw_nft_from_account(
        account_2,
        helper.incentives_id_address,
        NonFungibleLocalId::integer(2),
    )?;

    // Get incentive data for both accounts
    let id1 = helper.get_incentive_data(NonFungibleLocalId::integer(1))?;
    let id_data1 = id1.resources.get(&helper.ilis_address).unwrap();
    let id2 = helper.get_incentive_data(NonFungibleLocalId::integer(2))?;
    let id_data2 = id2.resources.get(&helper.ilis_address).unwrap();

    // Assert correct staked amounts
    assert_eq!(id_data1.amount_staked, dec!(1000));
    assert_eq!(id_data2.amount_staked, dec!(2000));

    Ok(())
}

#[test]
fn test_airdrop_locked_voting_staked_token() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a stakable resource
    helper.add_stakable(helper.ilis_address, dec!(100), dec!("1.01"), 365, dec!(3))?;

    let mut map: IndexMap<Reference, Decimal> = IndexMap::new();

    // Create an account and assign airdrop amount
    let account_1: Reference = helper.create_account()?;
    map.insert(account_1, dec!(1000));

    // Perform the airdrop of staked tokens with locking and voting periods
    let _ = helper.airdrop_staked_tokens(map, helper.ilis_address, 5, 4)?;

    // Withdraw NFT from account
    let _airdrop_1 = helper.withdraw_nft_from_account(
        account_1,
        helper.incentives_id_address,
        NonFungibleLocalId::integer(1),
    )?;

    // Get incentive data
    let id1 = helper.get_incentive_data(NonFungibleLocalId::integer(1))?;
    let id_data1 = id1.resources.get(&helper.ilis_address).unwrap();

    // Calculate expected unlock and unvote times
    let time_when_unlockable = helper.env.get_current_time().add_days(5).unwrap();
    let time_when_unvotable = helper.env.get_current_time().add_days(4).unwrap();

    // Assert correct locking and voting periods
    assert_eq!(id_data1.voting_until.unwrap(), time_when_unvotable);
    assert_eq!(id_data1.locked_until.unwrap(), time_when_unlockable);

    Ok(())
}

#[test]
fn test_airdrop_tokens() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    let mut map: IndexMap<Reference, ResourceSpecifier> = IndexMap::new();

    // Create two accounts and assign airdrop amounts
    let account_1: Reference = helper.create_account()?;
    let specifier_1: ResourceSpecifier = ResourceSpecifier::Fungible(dec!(3000));
    map.insert(account_1, specifier_1);

    let account_2: Reference = helper.create_account()?;
    let specifier_2: ResourceSpecifier = ResourceSpecifier::Fungible(dec!(4000));
    map.insert(account_2, specifier_2);

    // Perform the airdrop
    let _ = helper.airdrop_tokens(map, helper.ilis_address)?;

    // Withdraw airdropped tokens from accounts
    let airdrop_1 = helper.withdraw_from_account(account_1, helper.ilis_address, dec!(3000))?;
    let airdrop_2 = helper.withdraw_from_account(account_2, helper.ilis_address, dec!(4000))?;

    // Check remaining amount in DAO
    let leftover_amount = helper.dao_get_token_amount(helper.ilis_address)?;

    // Assert correct airdrop amounts and remaining DAO balance
    helper.assert_bucket_eq(&airdrop_1, helper.ilis_address, dec!(3000))?;
    helper.assert_bucket_eq(&airdrop_2, helper.ilis_address, dec!(4000))?;
    assert_eq!(leftover_amount, dec!(293000));

    Ok(())
}

#[test]
fn test_airdrop_nfts() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();

    // Create and put NFTs into DAO
    let nft_bucket_1 = helper.create_staking_id()?;
    let nft_bucket_2 = helper.create_staking_id()?;
    let _ = helper.dao_put_tokens(nft_bucket_1)?;
    let _ = helper.dao_put_tokens(nft_bucket_2)?;

    helper.env.disable_auth_module();

    let mut map: IndexMap<Reference, ResourceSpecifier> = IndexMap::new();

    // Create two accounts and assign NFTs for airdrop
    let account_1: Reference = helper.create_account()?;
    let mut set_1: IndexSet<NonFungibleLocalId> = IndexSet::new();
    set_1.insert(NonFungibleLocalId::integer(1));
    let specifier_1: ResourceSpecifier = ResourceSpecifier::NonFungible(set_1);
    map.insert(account_1, specifier_1);

    let account_2: Reference = helper.create_account()?;
    let mut set_2: IndexSet<NonFungibleLocalId> = IndexSet::new();
    set_2.insert(NonFungibleLocalId::integer(2));
    let specifier_2: ResourceSpecifier = ResourceSpecifier::NonFungible(set_2);
    map.insert(account_2, specifier_2);

    // Perform the NFT airdrop
    let _ = helper.airdrop_tokens(map, helper.staking_id_address)?;

    // Withdraw airdropped NFTs from accounts
    let _airdrop_1 = helper.withdraw_nft_from_account(
        account_1,
        helper.staking_id_address,
        NonFungibleLocalId::integer(1),
    )?;
    let _airdrop_2 = helper.withdraw_nft_from_account(
        account_2,
        helper.staking_id_address,
        NonFungibleLocalId::integer(2),
    )?;

    Ok(())
}

#[test]
fn test_job_lifetime() -> Result<(), RuntimeError> {
    // Initialize the helper and disable authentication
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Create a test account
    let account = helper.create_account()?;

    // Create three jobs: two without an employee and one with the test account as employee
    let _ = helper.create_job(
        None,
        dec!(1000),
        helper.ilis_address,
        7,
        true,
        "test job".to_string(),
        "test descr".to_string(),
    )?;
    let _ = helper.create_job(
        Some(account),
        dec!(1000),
        helper.ilis_address,
        7,
        true,
        "test job".to_string(),
        "test descr".to_string(),
    )?;
    let _ = helper.create_job(
        None,
        dec!(1000),
        helper.ilis_address,
        7,
        true,
        "test job".to_string(),
        "test descr".to_string(),
    )?;

    // Attempt to send salary (should not change balance as no job is assigned yet)
    let _ = helper.send_salary_to_employee(account, None)?;
    let amount_1 = helper.dao_get_token_amount(helper.ilis_address)?;

    // Employ the account for the first job
    let _ = helper.employ(0, account)?;

    // Advance time by 10 days
    let new_time_1 = helper.env.get_current_time().add_days(10).unwrap();
    helper.env.set_current_time(new_time_1);

    // Send salary (should decrease balance)
    helper.send_salary_to_employee(account, None)?;
    let amount_2 = helper.dao_get_token_amount(helper.ilis_address)?;

    // Advance time by 15 more days
    let new_time_2 = helper.env.get_current_time().add_days(15).unwrap();
    helper.env.set_current_time(new_time_2);

    // Send salary for specific job and then for all jobs
    let _ = helper.send_salary_to_employee(account, Some(0))?;
    let amount_3 = helper.dao_get_token_amount(helper.ilis_address)?;
    let _ = helper.send_salary_to_employee(account, None)?;
    let amount_4 = helper.dao_get_token_amount(helper.ilis_address)?;

    // Employ the account for the third job
    let _ = helper.employ(2, account)?;

    // Advance time and send salaries
    let new_time_3 = helper.env.get_current_time().add_days(5).unwrap();
    helper.env.set_current_time(new_time_3);
    let _ = helper.send_salary_to_employee(account, None)?;
    let amount_5 = helper.dao_get_token_amount(helper.ilis_address)?;

    let new_time_4 = helper.env.get_current_time().add_days(3).unwrap();
    helper.env.set_current_time(new_time_4);
    let _ = helper.send_salary_to_employee(account, None)?;
    let amount_6 = helper.dao_get_token_amount(helper.ilis_address)?;

    // Fire the account from the first job
    let new_time_5 = helper.env.get_current_time().add_days(3).unwrap();
    helper.env.set_current_time(new_time_5);
    let _ = helper.fire(account, 0, None)?;
    let amount_7 = helper.dao_get_token_amount(helper.ilis_address)?;

    // Advance time and send salary (should only pay for remaining job)
    let new_time_6 = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(new_time_6);
    let _ = helper.send_salary_to_employee(account, None)?;
    let amount_8 = helper.dao_get_token_amount(helper.ilis_address)?;

    // Re-employ and immediately fire from first job with no compensation
    let _ = helper.employ(0, account)?;
    let _ = helper.fire(account, 0, Some(dec!(0)))?;
    let amount_9 = helper.dao_get_token_amount(helper.ilis_address)?;

    // Withdraw accumulated salary
    let salary = helper.withdraw_from_account(account, helper.ilis_address, dec!(14000))?;

    // Assert all balance changes
    assert_eq!(amount_1, dec!(300000));
    assert_eq!(amount_2, dec!(298000));
    assert_eq!(amount_3, dec!(296000));
    assert_eq!(amount_4, dec!(294000));
    assert_eq!(amount_5, dec!(292000));
    assert_eq!(amount_6, dec!(291000));
    assert_eq!(amount_7, dec!(289000));
    assert_eq!(amount_8, dec!(286000));
    assert_eq!(amount_9, dec!(286000));
    helper.assert_bucket_eq(&salary, helper.ilis_address, dec!(14000))?;

    Ok(())
}

#[test]
fn test_post_remove_announcement() -> Result<(), RuntimeError> {
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Post an announcement
    let _announcement = helper.post_announcement("Test Announcement".to_string())?;

    // Remove the posted announcement
    let _ = helper.remove_announcement(0)?;

    Ok(())
}

#[test]
fn test_rewarded_calls() -> Result<(), RuntimeError> {
    // Initialize the helper and disable authentication
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Test initial rewarded update (should be 0)
    let bucket = helper.rewarded_update()?;
    helper.assert_bucket_eq(&bucket, helper.ilis_address, dec!(0))?;

    // Advance time by one day
    let time_in_a_day = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(time_in_a_day);

    // Test rewarded update after one day (should be 5000)
    let bucket_2 = helper.rewarded_update()?;
    helper.assert_bucket_eq(&bucket_2, helper.ilis_address, dec!(5000))?;

    // Change the update reward to 10000
    let _ = helper.set_update_reward(dec!(10000))?;

    // Advance time by another day
    let time_in_a_day_2 = helper.env.get_current_time().add_days(1).unwrap();
    helper.env.set_current_time(time_in_a_day_2);

    // Test rewarded update with new reward amount (should be 10000)
    let bucket_3 = helper.rewarded_update()?;
    helper.assert_bucket_eq(&bucket_3, helper.ilis_address, dec!(10000))?;

    Ok(())
}

#[test]
fn test_rewarded_call_addition() -> Result<(), RuntimeError> {
    // Initialize the helper and disable authentication
    let mut helper = Helper::new().unwrap();
    helper.env.disable_auth_module();

    // Add a rewarded call for the 'finish_bootstrap' method of the bootstrap component
    let _ = helper.add_rewarded_call(
        ComponentAddress::try_from(helper.bootstrap.0).unwrap(),
        vec!["finish_bootstrap".to_string()],
    )?;

    // Start the bootstrap process
    let _ = helper.start_bootstrap()?;

    // Advance time by one week
    let time_in_a_week = helper.env.get_current_time().add_days(7).unwrap();
    helper.env.set_current_time(time_in_a_week);

    // Perform a rewarded update
    let _bucket = helper.rewarded_update()?;

    // Attempt to swap XRD in the bootstrap (this should fail)
    let xrd_bucket = helper.xrd.take(dec!(1), &mut helper.env)?;
    let failure = helper.bootstrap_swap(xrd_bucket);

    // Assert that the swap failed (bootstrap should be finished)
    assert!(failure.is_err());

    Ok(())
}
