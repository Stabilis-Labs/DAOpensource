#![allow(dead_code)]

use dao::bootstrap::bootstrap_test::*;
use dao::dao::dao_test::*;
use dao::governance::governance_test::*;
use dao::incentives::incentives_test::*;
use dao::incentives::IncentivesId;
use dao::reentrancy::reentrancy_test::*;
use dao::staking::staking_test::*;
use dao::staking::Id;
use scrypto::prelude::ResourceSpecifier;
use scrypto_test::prelude::*;

pub struct Helper {
    pub env: TestEnvironment<InMemorySubstateDatabase>,
    pub package_address: PackageAddress,
    pub ilis: Bucket,
    pub admin: Bucket,
    pub xrd: Bucket,
    pub boot: Bucket,
    pub pool_token: ResourceAddress,
    pub staking_id_address: ResourceAddress,
    pub incentives_id_address: ResourceAddress,
    pub ilis_address: ResourceAddress,
    pub admin_address: ResourceAddress,
    pub xrd_address: ResourceAddress,
    pub dao: Dao,
    pub staking: Staking,
    pub governance: Governance,
    pub incentives: Incentives,
    pub reentrancy: ReentrancyProxy,
    pub bootstrap: LinearBootstrapPool,
}

#[derive(ScryptoSbor)]
pub struct Job {
    pub employee: Option<Reference>,
    pub last_payment: Instant,
    pub salary: Decimal,
    pub salary_token: ResourceAddress,
    pub duration: i64,
    pub recurring: bool,
    pub title: String,
    pub description: String,
}

impl Helper {
    pub fn new() -> Result<Self, RuntimeError> {
        let fake_dex_address = GlobalAddress::try_from_hex(
            "0df7665160fd68a27b3961ca504d0ecc12294d426c9ad56537a3f3e88d60",
        )
        .unwrap();
        let fake_locker_address = GlobalAddress::try_from_hex(
            "0d906318c6318c6fe2d9198c6318c6318cf7bd4f3bf55557c6318c6318c6",
        )
        .unwrap();
        let mut env = TestEnvironmentBuilder::new()
            .add_global_references(vec![fake_dex_address, fake_locker_address])
            .build();

        let package_address = PackageFactory::compile_and_publish(
            this_package!(),
            &mut env,
            CompileProfile::Standard,
        )?;

        let ilis = ResourceBuilder::new_fungible(OwnerRole::None)
            .divisibility(18)
            .mint_initial_supply(1000000, &mut env)?;
        let xrd = ResourceBuilder::new_fungible(OwnerRole::None)
            .divisibility(18)
            .mint_initial_supply(10000, &mut env)?;
        let admin = ResourceBuilder::new_fungible(OwnerRole::None)
            .divisibility(18)
            .mint_initial_supply(100, &mut env)?;

        let ilis_address = ilis.resource_address(&mut env)?;
        let admin_address = admin.resource_address(&mut env)?;
        let xrd_address = xrd.resource_address(&mut env)?;

        let dapp_definition: ComponentAddress = env
            .call_function_typed::<_, AccountCreateOutput>(
                ACCOUNT_PACKAGE,
                ACCOUNT_BLUEPRINT,
                ACCOUNT_CREATE_IDENT,
                &AccountCreateInput {},
            )?
            .0
             .0
            .into();

        let (
            dao,
            staking_ref,
            incentives_ref,
            governance_ref,
            reentrancy_ref,
            bootstrap_ref,
            founder_allocation,
            _non_bucket,
            boot,
            staking_id_address,
            incentives_id_address,
            pool_token,
        ) = Dao::instantiate_dao(
            ilis.take(dec!(500000), &mut env)?,
            dec!(0.01),
            dec!(0.1),
            dec!(0.1),
            dec!(0.19),
            admin.take(dec!(5), &mut env)?,
            "ILIS DAO".to_string(),
            "ILIS".to_string(),
            xrd.take(dec!(500), &mut env)?,
            dapp_definition,
            true,
            7,
            dec!(5000),
            7,
            UncheckedUrl::of("https://blabla.com").into(),
            UncheckedUrl::of("https://blabla.com").into(),
            UncheckedUrl::of("https://blabla.com").into(),
            UncheckedUrl::of("https://blabla.com").into(),
            UncheckedUrl::of("https://blabla.com").into(),
            UncheckedUrl::of("https://blabla.com").into(),
            package_address,
            &mut env,
        )?;

        assert_eq!(
            dec!(0.01) * dec!(500000),
            founder_allocation.amount(&mut env)?
        );
        assert_eq!(ilis_address, founder_allocation.resource_address(&mut env)?);
        assert_eq!(dao.get_token_amount(ilis_address, &mut env)?, dec!(300000));

        Ok(Self {
            env,
            package_address,
            ilis,
            xrd,
            admin,
            boot,
            ilis_address,
            admin_address,
            xrd_address,
            dao,
            staking: Staking(*staking_ref.as_node_id()),
            governance: Governance(*governance_ref.handle.as_node_id()),
            incentives: Incentives(*incentives_ref.as_node_id()),
            reentrancy: ReentrancyProxy(*reentrancy_ref.as_node_id()),
            bootstrap: LinearBootstrapPool(*bootstrap_ref.as_node_id()),
            pool_token,
            staking_id_address,
            incentives_id_address,
        })
    }

    /////////////////////////////////////////////////
    //////////////////// DAO ////////////////////////
    /////////////////////////////////////////////////

    pub fn dao_get_token_amount(
        &mut self,
        resource_address: ResourceAddress,
    ) -> Result<Decimal, RuntimeError> {
        let amount = self.dao.get_token_amount(resource_address, &mut self.env)?;

        Ok(amount)
    }

    pub fn dao_send_tokens(
        &mut self,
        address: ResourceAddress,
        specifier: ResourceSpecifier,
        recipient: ComponentAddress,
    ) -> Result<(), RuntimeError> {
        let _ = self.dao.send_tokens(
            address,
            specifier,
            recipient,
            "put_tokens".to_string(),
            &mut self.env,
        )?;

        Ok(())
    }

    pub fn dao_put_tokens(&mut self, bucket: Bucket) -> Result<(), RuntimeError> {
        self.dao.put_tokens(bucket, &mut self.env)?;

        Ok(())
    }

    pub fn dao_take_tokens(
        &mut self,
        address: ResourceAddress,
        specifier: ResourceSpecifier,
    ) -> Result<Bucket, RuntimeError> {
        let bucket = self.dao.take_tokens(address, specifier, &mut self.env)?;

        Ok(bucket)
    }

    pub fn airdrop_membered_tokens(
        &mut self,
        claimants: IndexMap<Reference, Decimal>,
        lock_duration: i64,
        vote_duration: i64,
    ) -> Result<(), RuntimeError> {
        self.dao
            .airdrop_membered_tokens(claimants, lock_duration, vote_duration, &mut self.env)?;

        Ok(())
    }

    pub fn airdrop_staked_tokens(
        &mut self,
        claimants: IndexMap<Reference, Decimal>,
        address: ResourceAddress,
        lock_duration: i64,
        vote_duration: i64,
    ) -> Result<(), RuntimeError> {
        self.dao.airdrop_staked_tokens(
            claimants,
            address,
            lock_duration,
            vote_duration,
            &mut self.env,
        )?;

        Ok(())
    }

    pub fn airdrop_tokens(
        &mut self,
        claimants: IndexMap<Reference, ResourceSpecifier>,
        address: ResourceAddress,
    ) -> Result<(), RuntimeError> {
        self.dao.airdrop_tokens(claimants, address, &mut self.env)?;

        Ok(())
    }

    pub fn create_job(
        &mut self,
        employee: Option<Reference>,
        salary: Decimal,
        salary_token: ResourceAddress,
        duration: i64,
        recurring: bool,
        title: String,
        description: String,
    ) -> Result<(), RuntimeError> {
        let _ = self.env.call_method_typed::<_, _, ()>(
            self.dao.0,
            "create_job",
            &(
                employee,
                salary,
                salary_token,
                duration,
                recurring,
                title,
                description,
            ),
        )?;

        Ok(())
    }

    pub fn employ(&mut self, job_id: u64, employee: Reference) -> Result<(), RuntimeError> {
        let _ =
            self.env
                .call_method_typed::<_, _, ()>(self.dao.0, "employ", &(job_id, employee))?;

        Ok(())
    }

    pub fn send_salary_to_employee(
        &mut self,
        employee: Reference,
        single_job: Option<u64>,
    ) -> Result<(), RuntimeError> {
        let _ = self.env.call_method_typed::<_, _, ()>(
            self.dao.0,
            "send_salary_to_employee",
            &(employee, single_job),
        )?;

        Ok(())
    }

    pub fn fire(
        &mut self,
        employee: Reference,
        job_id: u64,
        salary_modifier: Option<Decimal>,
    ) -> Result<(), RuntimeError> {
        let _ = self.env.call_method_typed::<_, _, ()>(
            self.dao.0,
            "fire",
            &(employee, job_id, salary_modifier),
        )?;

        Ok(())
    }

    pub fn post_announcement(&mut self, announcement: String) -> Result<(), RuntimeError> {
        self.dao
            .post_announcement(announcement, None, &mut self.env)?;

        Ok(())
    }

    pub fn remove_announcement(&mut self, announcement_id: u64) -> Result<(), RuntimeError> {
        self.dao
            .remove_announcement(announcement_id, &mut self.env)?;

        Ok(())
    }

    pub fn rewarded_update(&mut self) -> Result<Bucket, RuntimeError> {
        let bucket = self.dao.rewarded_update(&mut self.env)?;

        Ok(bucket)
    }

    pub fn add_rewarded_call(
        &mut self,
        component: ComponentAddress,
        methods: Vec<String>,
    ) -> Result<(), RuntimeError> {
        self.dao
            .add_rewarded_call(component, methods, &mut self.env)?;

        Ok(())
    }

    pub fn set_update_reward(&mut self, reward: Decimal) -> Result<(), RuntimeError> {
        self.dao.set_update_reward(reward, &mut self.env)?;

        Ok(())
    }

    //////////////////////////////////////////////////
    //////////////////// BOOTSTRAP ///////////////////
    //////////////////////////////////////////////////

    pub fn bootstrap_swap(&mut self, payment: Bucket) -> Result<Bucket, RuntimeError> {
        let return_bucket = self.bootstrap.swap(payment, &mut self.env)?;

        Ok(return_bucket)
    }

    pub fn start_bootstrap(&mut self) -> Result<(), RuntimeError> {
        self.env.disable_auth_module();
        let _ = self.bootstrap.start_bootstrap(&mut self.env)?;
        self.env.enable_auth_module();
        Ok(())
    }

    pub fn finish_bootstrap(&mut self) -> Result<(), RuntimeError> {
        let _ = self.bootstrap.finish_bootstrap(&mut self.env)?;

        Ok(())
    }

    pub fn reclaim_bootstrap_initial(
        &mut self,
        boot_badge: Bucket,
    ) -> Result<Bucket, RuntimeError> {
        let return_bucket = self.bootstrap.reclaim_initial(boot_badge, &mut self.env)?;

        Ok(return_bucket)
    }

    //////////////////////////////////////////////////
    //////////////////// STAKING /////////////////////
    //////////////////////////////////////////////////

    pub fn create_staking_id(&mut self) -> Result<Bucket, RuntimeError> {
        let bucket1 = self.staking.create_id(&mut self.env)?;

        Ok(bucket1)
    }

    pub fn stake_without_id(
        &mut self,
        stake_bucket: Bucket,
    ) -> Result<(Option<Bucket>, Option<Bucket>), RuntimeError> {
        let (bucket1, bucket2) = self.staking.stake(stake_bucket, None, &mut self.env)?;

        Ok((bucket1, bucket2))
    }

    pub fn stake_with_id(
        &mut self,
        stake_bucket: Bucket,
        stake_id: Bucket,
    ) -> Result<(Option<Bucket>, Option<Bucket>, Bucket), RuntimeError> {
        let stake_id_proof = stake_id.create_proof_of_all(&mut self.env)?;
        let (bucket1, bucket2) =
            self.staking
                .stake(stake_bucket, Some(stake_id_proof), &mut self.env)?;

        Ok((bucket1, bucket2, stake_id))
    }

    pub fn start_unstake(
        &mut self,
        stake_id: Bucket,
        amount: Decimal,
    ) -> Result<(Bucket, Bucket), RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let bucket1 = self
            .staking
            .start_unstake(stake_id_proof, amount, false, &mut self.env)?;

        Ok((bucket1, stake_id))
    }

    pub fn start_unstake_transfer(
        &mut self,
        stake_id: Bucket,
        amount: Decimal,
    ) -> Result<(Bucket, Bucket), RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let bucket1 = self
            .staking
            .start_unstake(stake_id_proof, amount, true, &mut self.env)?;

        Ok((bucket1, stake_id))
    }

    pub fn finish_unstake(&mut self, receipt: Bucket) -> Result<Bucket, RuntimeError> {
        let unstake_bucket = self.staking.finish_unstake(receipt, &mut self.env)?;

        Ok(unstake_bucket)
    }

    pub fn delegate_vote(
        &mut self,
        stake_id: Bucket,
        delagatee: NonFungibleLocalId,
    ) -> Result<Bucket, RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let _ = self
            .staking
            .delegate_vote(stake_id_proof, delagatee, &mut self.env)?;

        Ok(stake_id)
    }

    pub fn undelegate_vote(&mut self, stake_id: Bucket) -> Result<Bucket, RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let _ = self
            .staking
            .undelegate_vote(stake_id_proof, &mut self.env)?;

        Ok(stake_id)
    }

    pub fn get_remaining_staking_rewards(&mut self) -> Result<Decimal, RuntimeError> {
        let rewards = self.staking.get_remaining_rewards(&mut self.env)?;

        Ok(rewards)
    }

    pub fn lock_stake(
        &mut self,
        stake_id: Bucket,
        duration: i64,
        for_reward: bool,
    ) -> Result<Bucket, RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let _ = self
            .staking
            .lock_stake(stake_id_proof, duration, for_reward, &mut self.env)?;

        Ok(stake_id)
    }

    pub fn unlock_stake(
        &mut self,
        stake_id: Bucket,
        payment: Bucket,
        duration: i64,
    ) -> Result<(Bucket, Bucket), RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let leftover_payment =
            self.staking
                .unlock_stake(stake_id_proof, payment, duration, &mut self.env)?;

        Ok((stake_id, leftover_payment))
    }

    pub fn get_real_amount(&mut self) -> Result<Decimal, RuntimeError> {
        let amount = self.staking.get_real_amount(dec!(1), &mut self.env)?;

        Ok(amount)
    }

    //////////////////////////////////////////////////
    //////////////////// INCENTIVES //////////////////
    //////////////////////////////////////////////////

    pub fn add_stakable(
        &mut self,
        address: ResourceAddress,
        reward_amount: Decimal,
        payment: Decimal,
        max_duration: i64,
        unlock_multiplier: Decimal,
    ) -> Result<(), RuntimeError> {
        let _ = self.incentives.add_stakable(
            address,
            reward_amount,
            payment,
            max_duration,
            unlock_multiplier,
            dec!(1),
            &mut self.env,
        )?;

        Ok(())
    }

    pub fn stake_incentives_without_id(
        &mut self,
        stake_bucket: Bucket,
    ) -> Result<(Option<Bucket>, Option<Bucket>), RuntimeError> {
        let (bucket1, bucket2) = self.incentives.stake(stake_bucket, None, &mut self.env)?;

        Ok((bucket1, bucket2))
    }

    pub fn stake_incentives_with_id(
        &mut self,
        stake_bucket: Bucket,
        stake_id: Bucket,
    ) -> Result<(Option<Bucket>, Option<Bucket>, Bucket), RuntimeError> {
        let stake_id_proof = stake_id.create_proof_of_all(&mut self.env)?;
        let (bucket1, bucket2) =
            self.incentives
                .stake(stake_bucket, Some(stake_id_proof), &mut self.env)?;

        Ok((bucket1, bucket2, stake_id))
    }

    pub fn start_incentives_unstake(
        &mut self,
        address: ResourceAddress,
        stake_id: Bucket,
        amount: Decimal,
    ) -> Result<(Bucket, Bucket), RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let bucket1 =
            self.incentives
                .start_unstake(stake_id_proof, address, amount, false, &mut self.env)?;

        Ok((bucket1, stake_id))
    }

    pub fn start_incentives_unstake_transfer(
        &mut self,
        address: ResourceAddress,
        stake_id: Bucket,
        amount: Decimal,
    ) -> Result<(Bucket, Bucket), RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let bucket1 =
            self.incentives
                .start_unstake(stake_id_proof, address, amount, true, &mut self.env)?;

        Ok((bucket1, stake_id))
    }

    pub fn finish_incentives_unstake(&mut self, receipt: Bucket) -> Result<Bucket, RuntimeError> {
        let unstake_bucket = self.incentives.finish_unstake(receipt, &mut self.env)?;

        Ok(unstake_bucket)
    }

    pub fn lock_incentives_stake(
        &mut self,
        address: ResourceAddress,
        stake_id: Bucket,
        duration: i64,
    ) -> Result<(Bucket, Bucket), RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let bucket =
            self.incentives
                .lock_stake(address, stake_id_proof, duration, &mut self.env)?;

        Ok((stake_id, bucket.0))
    }

    pub fn unlock_incentives_stake(
        &mut self,
        address: ResourceAddress,
        stake_id: Bucket,
        payment: Bucket,
        duration: i64,
    ) -> Result<(Bucket, Bucket), RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let leftover_payment = self.incentives.unlock_stake(
            address,
            stake_id_proof,
            payment,
            duration,
            &mut self.env,
        )?;

        Ok((stake_id, leftover_payment))
    }

    pub fn update_incentives_id(
        &mut self,
        stake_id: Bucket,
    ) -> Result<(Bucket, Bucket), RuntimeError> {
        let stake_id_proof = NonFungibleProof(stake_id.create_proof_of_all(&mut self.env)?);
        let rewards = self.incentives.update_id(stake_id_proof, &mut self.env)?;

        Ok((stake_id, rewards.0))
    }

    //////////////////////////////////////////////////
    /////////////////// GOVERNANCE ///////////////////
    //////////////////////////////////////////////////

    pub fn create_basic_proposal(
        &mut self,
        payment_amount: Decimal,
    ) -> Result<(Bucket, Bucket), RuntimeError> {
        let value: ScryptoValue = scrypto_decode(&scrypto_encode(&(dec!(100),)).unwrap()).unwrap();
        let result = self.governance.create_proposal(
            "Test Proposal".to_string(),
            "This is a test proposal".to_string(),
            None,
            ComponentAddress::try_from(self.dao.0.clone()).unwrap(),
            self.admin_address,
            "set_update_reward".to_string(),
            value,
            false,
            false,
            self.ilis.take(payment_amount, &mut self.env)?,
            &mut self.env,
        )?;

        Ok(result)
    }

    pub fn add_normal_proposal_step(
        &mut self,
        proposal_receipt: Bucket,
    ) -> Result<Bucket, RuntimeError> {
        let proposal_receipt_proof =
            NonFungibleProof(proposal_receipt.create_proof_of_all(&mut self.env)?);
        let _ = self.governance.add_proposal_step(
            proposal_receipt_proof,
            ComponentAddress::try_from(self.dao.0.clone()).unwrap(),
            self.admin_address,
            "set_update_reward".to_string(),
            scrypto_decode(&scrypto_encode(&(dec!(2000),)).unwrap()).unwrap(),
            false,
            false,
            &mut self.env,
        )?;

        Ok(proposal_receipt)
    }

    pub fn add_reentrancy_proposal_step(
        &mut self,
        proposal_receipt: Bucket,
    ) -> Result<Bucket, RuntimeError> {
        let proposal_receipt_proof =
            NonFungibleProof(proposal_receipt.create_proof_of_all(&mut self.env)?);
        let _ = self.governance.add_proposal_step(
            proposal_receipt_proof,
            ComponentAddress::try_from(self.governance.0.clone()).unwrap(),
            self.admin_address,
            "set_parameters".to_string(),
            scrypto_decode(
                &scrypto_encode(&(dec!(5000), 7i64, dec!(10000), dec!(0.5), 7i64)).unwrap(),
            )
            .unwrap(),
            false,
            true,
            &mut self.env,
        )?;

        Ok(proposal_receipt)
    }

    pub fn submit_proposal(&mut self, proposal_receipt: Bucket) -> Result<Bucket, RuntimeError> {
        let proposal_receipt_proof =
            NonFungibleProof(proposal_receipt.create_proof_of_all(&mut self.env)?);
        let _ = self
            .governance
            .submit_proposal(proposal_receipt_proof, &mut self.env)?;

        Ok(proposal_receipt)
    }

    pub fn vote_on_proposal(
        &mut self,
        for_against: bool,
        vote_id: Bucket,
        proposal_id: u64,
    ) -> Result<Bucket, RuntimeError> {
        let vote_id_proof = NonFungibleProof(vote_id.create_proof_of_all(&mut self.env)?);
        let _ = self.governance.vote_on_proposal(
            proposal_id,
            for_against,
            vote_id_proof,
            &mut self.env,
        )?;

        Ok(vote_id)
    }

    pub fn finish_voting(&mut self, proposal_id: u64) -> Result<(), RuntimeError> {
        let _ = self.governance.finish_voting(proposal_id, &mut self.env)?;

        Ok(())
    }

    pub fn execute_proposal_step(
        &mut self,
        proposal_id: u64,
        steps: i64,
    ) -> Result<(), RuntimeError> {
        let _ = self
            .governance
            .execute_proposal_step(proposal_id, steps, &mut self.env)?;

        Ok(())
    }

    pub fn execute_reentrancy(&mut self, proposal_id: u64) -> Result<(), RuntimeError> {
        let _ = self.reentrancy.call(proposal_id, &mut self.env)?;

        Ok(())
    }

    pub fn retrieve_fee(&mut self, proposal_receipt: Bucket) -> Result<Bucket, RuntimeError> {
        let proposal_receipt_proof =
            NonFungibleProof(proposal_receipt.create_proof_of_all(&mut self.env)?);
        let fee = self
            .governance
            .retrieve_fee(proposal_receipt_proof, &mut self.env)?;

        Ok(fee)
    }

    pub fn hurry_proposal(
        &mut self,
        proposal_id: u64,
        new_duration: i64,
    ) -> Result<(), RuntimeError> {
        let _ = self
            .governance
            .hurry_proposal(proposal_id, new_duration, &mut self.env)?;

        Ok(())
    }

    /////////////////////////////////////////////////
    //////////////////// TEST HELPERS ///////////////
    /////////////////////////////////////////////////

    pub fn create_account(&mut self) -> Result<Reference, RuntimeError> {
        let account = self
            .env
            .call_function_typed::<_, AccountCreateOutput>(
                ACCOUNT_PACKAGE,
                ACCOUNT_BLUEPRINT,
                ACCOUNT_CREATE_IDENT,
                &AccountCreateInput {},
            )?
            .0;
        Ok(account.0.into())
    }

    pub fn withdraw_from_account(
        &mut self,
        account: Reference,
        resource_address: ResourceAddress,
        amount: Decimal,
    ) -> Result<Bucket, RuntimeError> {
        let bucket = self.env.call_method_typed::<_, _, AccountWithdrawOutput>(
            account.as_node_id().clone(),
            ACCOUNT_WITHDRAW_IDENT,
            &AccountWithdrawInput {
                resource_address,
                amount,
            },
        )?;

        Ok(bucket)
    }

    pub fn withdraw_nft_from_account(
        &mut self,
        account: Reference,
        resource_address: ResourceAddress,
        id: NonFungibleLocalId,
    ) -> Result<Bucket, RuntimeError> {
        let mut ids: IndexSet<NonFungibleLocalId> = IndexSet::new();
        ids.insert(id);
        let bucket = self
            .env
            .call_method_typed::<_, _, AccountWithdrawNonFungiblesOutput>(
                account.as_node_id().clone(),
                ACCOUNT_WITHDRAW_NON_FUNGIBLES_IDENT,
                &AccountWithdrawNonFungiblesInput {
                    resource_address,
                    ids,
                },
            )?;

        Ok(bucket)
    }

    pub fn get_member_data(&mut self, id: NonFungibleLocalId) -> Result<Id, RuntimeError> {
        let resource_manager = ResourceManager(self.staking_id_address);
        let nft_data = resource_manager.get_non_fungible_data::<_, _, Id>(id, &mut self.env)?;

        Ok(nft_data)
    }

    pub fn get_incentive_data(
        &mut self,
        id: NonFungibleLocalId,
    ) -> Result<IncentivesId, RuntimeError> {
        let resource_manager = ResourceManager(self.incentives_id_address);
        let nft_data =
            resource_manager.get_non_fungible_data::<_, _, IncentivesId>(id, &mut self.env)?;

        Ok(nft_data)
    }

    pub fn assert_bucket_eq(
        &mut self,
        bucket: &Bucket,
        address: ResourceAddress,
        amount: Decimal,
    ) -> Result<(), RuntimeError> {
        assert_eq!(bucket.resource_address(&mut self.env)?, address);
        assert_eq!(bucket.amount(&mut self.env)?, amount);

        Ok(())
    }
}
