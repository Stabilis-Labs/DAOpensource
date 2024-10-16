//! # DAO Governance Blueprint
//!
//! Can be used to build and submit proposals, vote on them, and execute them.
//!
//! Proposals work through ProposalSteps, which hold information about a step in a proposal and include a method call.
//! A proposer can build proposals by adding steps to them, and when finishing, submitting this proposal. Proposers can use their Proposal Receipts for this.
//! After submitting a proposal, user's can vote on it using their Staking IDs, locking their staked governance tokens for the duration of the vote.
//! After the voting period has passed, the proposal can be executed, which will execute all steps in the proposal one by one.
//! Calling methods on the Governance component itself needs to happen through the ReentrancyProxy component, as the Radix Engine does not support reentrancy.

use crate::reentrancy::reentrancy::*;
use crate::staking::staking::*;
use scrypto::prelude::*;

/// File structure, holding all information to lookup a file stored on the Radix Ledger.
#[derive(ScryptoSbor)]
pub struct File {
    pub kvs_address: String,
    pub component_address: ComponentAddress,
    pub file_hash: String,
}

/// Proposal structure, holding all information about a proposal in the governance component.
#[derive(ScryptoSbor)]
pub struct Proposal {
    pub title: String,
    pub description: String,
    pub files: Option<Vec<File>>,
    pub steps: Vec<ProposalStep>,
    pub votes_for: Decimal,
    pub votes_against: Decimal,
    pub votes: KeyValueStore<NonFungibleLocalId, Decimal>,
    pub deadline: Instant,
    pub has_failed_in_last_day: Option<bool>,
    pub next_index: i64,
    pub status: ProposalStatus,
    pub reentrancy: bool,
}

/// Proposal receipt structure, minted when a user wants to propose a new proposal, usable to update the proposal and submit it.
/// After the proposal is accepted, the receipt is redeemable for the fee paid.
#[derive(ScryptoSbor, NonFungibleData)]
pub struct ProposalReceipt {
    #[mutable]
    pub fee_paid: Decimal,
    #[mutable]
    pub proposal_id: u64,
    #[mutable]
    pub status: ProposalStatus,
}

/// Proposal step structure, holding information about a step in a proposal.
#[derive(ScryptoSbor)]
pub struct ProposalStep {
    pub component: ComponentAddress,
    pub badge: ResourceAddress,
    pub method: String,
    pub args: ScryptoValue,
    pub return_bucket: bool,
    pub reentrancy: bool,
}

/// ProposalStatus enum, holding all possible statuses of a proposal.
#[derive(ScryptoSbor, PartialEq, Clone, Copy)]
pub enum ProposalStatus {
    Building,
    Ongoing,
    VetoMode,
    Rejected,
    Accepted,
    Executed,
    Finished,
}

/// GovernanceParameters structure, holding all parameters of the governance component.
#[derive(ScryptoSbor)]
pub struct GovernanceParameters {
    pub fee: Decimal,
    pub proposal_duration: i64,
    pub quorum: Decimal,
    pub approval_threshold: Decimal,
    pub maximum_proposal_submit_delay: i64,
}

#[blueprint]
#[types(ResourceAddress, Vault, u64, Proposal, ProposalStatus, Decimal, Option<Vec<File>>)]
mod governance {
    enable_method_auth! {
        methods {
            put_tokens => PUBLIC;
            create_proposal => PUBLIC;
            add_proposal_step => PUBLIC;
            submit_proposal => PUBLIC;
            vote_on_proposal => PUBLIC;
            finish_voting => PUBLIC;
            execute_proposal_step => PUBLIC;
            retrieve_fee => PUBLIC;
            finish_reentrancy_step => restrict_to: [OWNER];
            send_tokens => restrict_to: [OWNER];
            set_parameters => restrict_to: [OWNER];
            set_staking_component => restrict_to: [OWNER];
            hurry_proposal => restrict_to: [OWNER];
        }
    }

    struct Governance {
        /// The staking component, used to get voting information from
        staking: Global<Staking>,
        /// The reentrancy component, used to execute ProposalSteps that require reentrancy
        reentrancy: Global<ReentrancyProxy>,
        /// The address of the mother token (the governance token)
        mother_token_address: ResourceAddress,
        /// The address of the mother pool token, used to represent staked mother tokens
        mother_pool_token_address: ResourceAddress,
        /// The vault holding the fee paid for proposals
        proposal_fee_vault: Vault,
        /// Resource manager for proposal receipts
        proposal_receipt_manager: ResourceManager,
        /// KVS holding all vaults, indexed by their address (these vaults should contain badges used for authorizing proposal steps)
        vaults: KeyValueStore<ResourceAddress, Vault>,
        /// KVS holding all proposals, indexed by their ID
        proposals: KeyValueStore<u64, Proposal>,
        /// Counter for the proposal IDs
        proposal_counter: u64,
        /// Governance parameters
        parameters: GovernanceParameters,
        /// The address of Staking IDs, which are used to vote on proposals
        voting_id_address: ResourceAddress,
        /// The address of the controller badge, used to authorize owner methods
        controller_badge_address: ResourceAddress,
        /// The address of the component
        component_address: ComponentAddress,
    }

    impl Governance {
        /// Instantiates a new Governance component.
        ///
        /// # Input
        /// - `controller_badge`: Badge to use for the controller badge vault, allowing access to owner methods
        /// - `protocol_name`: Name of the protocol
        /// - `protocol_token_symbol`: Symbol of the protocol token
        /// - `proposal_receipt_icon_url`: URL of the icon for the proposal receipt
        /// - `staking`: Staking component to use for voting
        /// - `mother_token_address`: Address of the mother (governance) token
        /// - `mother_pool_token_address`: Address of the mother pool token
        /// - `voting_id_address`: Address of the Staking IDs
        ///
        /// # Output
        /// - `Global<Governance>`: The newly instantiated Governance component
        ///
        /// # Logic
        /// - Instantiates a reentrancy component,
        /// - Instantiates a new Governance component with the given parameters
        pub fn instantiate_governance(
            mut controller_badge: Bucket,
            protocol_name: String,
            protocol_token_symbol: String,
            proposal_receipt_icon_url: Url,
            staking: Global<Staking>,
            mother_token_address: ResourceAddress,
            mother_pool_token_address: ResourceAddress,
            voting_id_address: ResourceAddress,
            dapp_def_address: GlobalAddress,
            info_url: Url,
        ) -> (Global<Governance>, Global<ReentrancyProxy>) {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Governance::blueprint_id());

            let reentrancy: Global<ReentrancyProxy> =
                ReentrancyProxy::new(controller_badge.take(1), dapp_def_address, info_url.clone());

            let controller_badge_address: ResourceAddress = controller_badge.resource_address();

            let proposal_receipt_manager = ResourceBuilder::new_integer_non_fungible::<
                ProposalReceipt,
            >(OwnerRole::Fixed(rule!(require(
                controller_badge.resource_address()
            ))))
            .metadata(metadata!(
                init {
                    "name" => format!("{} proposal receipt", protocol_name), updatable;
                    "symbol" => format!("prop{}", protocol_token_symbol), updatable;
                    "description" => format!("Proposal receipt for {}", protocol_name), updatable;
                    "icon_url" => proposal_receipt_icon_url, updatable;
                }
            ))
            .mint_roles(mint_roles!(
                minter => rule!(require(global_caller(component_address))
                || require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
                ));
                minter_updater => rule!(deny_all);
            ))
            .burn_roles(burn_roles!(
                burner => rule!(deny_all);
                burner_updater => rule!(deny_all);
            ))
            .non_fungible_data_update_roles(non_fungible_data_update_roles!(
                non_fungible_data_updater => rule!(require(global_caller(component_address))
                || require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()));
                non_fungible_data_updater_updater => rule!(deny_all);
            ))
            .create_with_no_initial_supply();

            let parameters = GovernanceParameters {
                fee: dec!(10000),
                proposal_duration: 3,
                quorum: dec!(10000),
                approval_threshold: dec!("0.5"),
                maximum_proposal_submit_delay: 7,
            };

            let vaults: KeyValueStore<ResourceAddress, Vault> =
                GovernanceKeyValueStore::new_with_registered_type();

            vaults.insert(
                controller_badge.resource_address(),
                Vault::with_bucket(controller_badge),
            );

            let governance = Self {
                staking,
                mother_token_address,
                mother_pool_token_address,
                proposal_fee_vault: Vault::new(mother_token_address),
                vaults,
                proposal_receipt_manager,
                proposals: GovernanceKeyValueStore::new_with_registered_type(),
                proposal_counter: 0,
                parameters,
                voting_id_address,
                controller_badge_address,
                component_address,
                reentrancy,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(controller_badge_address))))
            .with_address(address_reservation)
            .metadata(metadata! {
                init {
                    "name" => format!("{} Governance", protocol_name), updatable;
                    "description" => format!("Governance for {}", protocol_name), updatable;
                    "info_url" => info_url, updatable;
                    "dapp_definition" => dapp_def_address, updatable;
                }
            })
            .globalize();

            (governance, reentrancy)
        }

        /// Puts tokens into the Governance component (most often badges needed for authenticating proposalsteps)
        pub fn put_tokens(&mut self, tokens: Bucket) {
            if self.vaults.get_mut(&tokens.resource_address()).is_some() {
                self.vaults
                    .get_mut(&tokens.resource_address())
                    .unwrap()
                    .put(tokens);
            } else {
                self.vaults
                    .insert(tokens.resource_address(), Vault::with_bucket(tokens));
            };
        }

        /// Sends tokens to a component by calling the put_tokens method on the component.
        pub fn send_tokens(
            &mut self,
            address: ResourceAddress,
            tokens: ResourceSpecifier,
            receiver_address: ComponentAddress,
            put_method: String,
        ) {
            let payment: Bucket = match tokens {
                ResourceSpecifier::Fungible(amount) => self
                    .vaults
                    .get_mut(&address)
                    .unwrap()
                    .as_fungible()
                    .take_advanced(
                        amount,
                        WithdrawStrategy::Rounded(RoundingMode::ToNegativeInfinity),
                    )
                    .into(),
                ResourceSpecifier::NonFungible(ids) => self
                    .vaults
                    .get_mut(&address)
                    .unwrap()
                    .as_non_fungible()
                    .take_non_fungibles(&ids)
                    .into(),
            };
            let receiver: Global<AnyComponent> = Global::from(receiver_address);
            receiver.call_raw::<()>(&put_method, scrypto_args!(payment));
        }

        /// Creates a new proposal.
        ///
        /// # Input
        /// - `title`: Title of the proposal
        /// - `description`: Description of the proposal
        /// - `component`: Address of the component to call (in the first step)
        /// - `badge`: Badge to use for authorization (in the first step)
        /// - `method`: Method to call on the component (in the first step)
        /// - `args`: Arguments to pass to the method (in the first step)
        /// - `return_bucket`: Whether the method returns a bucket
        /// - `payment`: Payment for the proposal
        ///
        /// # Output
        /// - A bucket with the leftover payment
        /// - A bucket with the incomplete proposal receipt
        ///
        /// # Logic
        /// - Checks if the payment is correct and more than the fee
        /// - Puts the fee into the proposal fee vault
        /// - Creates a new ProposalStep with the given parameters
        /// - Creates a new Proposal with this ProposalStep
        /// - Mints a new ProposalReceipt for this proposal
        /// - Inserts the proposal into the proposals KVS
        /// - Increments the proposal counter
        pub fn create_proposal(
            &mut self,
            title: String,
            description: String,
            files: Option<Vec<File>>,
            component: ComponentAddress,
            badge: ResourceAddress,
            method: String,
            args: ScryptoValue,
            return_bucket: bool,
            reentrancy: bool,
            mut payment: Bucket,
        ) -> (Bucket, Bucket) {
            assert!(
                payment.resource_address() == self.mother_token_address
                    && payment.amount() >= self.parameters.fee,
                "Invalid payment, must be more than the fee and correct token."
            );

            self.proposal_fee_vault
                .put(payment.take(self.parameters.fee));

            let first_step = ProposalStep {
                component,
                badge,
                method,
                args,
                return_bucket,
                reentrancy,
            };

            let proposal = Proposal {
                title,
                description,
                files,
                steps: vec![first_step],
                votes_for: dec!(0),
                votes_against: dec!(0),
                votes: KeyValueStore::new(),
                deadline: Clock::current_time_rounded_to_seconds()
                    .add_minutes(self.parameters.maximum_proposal_submit_delay)
                    .unwrap(),
                next_index: 0,
                has_failed_in_last_day: None,
                status: ProposalStatus::Building,
                reentrancy: false,
            };

            let proposal_receipt = ProposalReceipt {
                fee_paid: self.parameters.fee,
                proposal_id: self.proposal_counter,
                status: ProposalStatus::Building,
            };

            let incomplete_proposal_receipt: Bucket =
                self.proposal_receipt_manager.mint_non_fungible(
                    &NonFungibleLocalId::integer(self.proposal_counter),
                    proposal_receipt,
                );

            self.proposals.insert(self.proposal_counter, proposal);
            self.proposal_counter += 1;

            (payment, incomplete_proposal_receipt)
        }

        /// Adds a step to a proposal.
        ///
        /// # Input
        /// - `proposal_receipt_proof`: Proof of the proposal receipt you want to add a step to
        /// - `component`: Address of the component to call for this step
        /// - `badge`: Badge to use for authorization for this step
        /// - `method`: Method to call on the component for this step
        /// - `args`: Arguments to pass to the method for this step
        /// - `return_bucket`: Whether the method returns a bucket
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Checks if the proposal receipt is valid
        /// - Checks whether the proposal is in the building phase
        /// - Adds a new ProposalStep to the proposal
        pub fn add_proposal_step(
            &mut self,
            proposal_receipt_proof: NonFungibleProof,
            component: ComponentAddress,
            badge: ResourceAddress,
            method: String,
            args: ScryptoValue,
            return_bucket: bool,
            reentrancy: bool,
        ) {
            let receipt_proof = proposal_receipt_proof.check_with_message(
                self.proposal_receipt_manager.address(),
                "Invalid proposal receipt supplied!",
            );

            let receipt = receipt_proof.non_fungible::<ProposalReceipt>().data();
            assert!(
                receipt.status == ProposalStatus::Building,
                "Proposal is not being built!"
            );

            let proposal_id: u64 = receipt.proposal_id;
            let mut proposal = self.proposals.get_mut(&proposal_id).unwrap();

            let step = ProposalStep {
                component,
                badge,
                method,
                args,
                return_bucket,
                reentrancy,
            };

            proposal.steps.push(step);
        }

        /// Submits a proposal.
        ///
        /// # Input
        /// - `proposal_receipt_proof`: Proof of the proposal receipt you want to submit
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Checks if the proposal receipt is valid
        /// - Checks whether the proposal is in the building phase
        /// - Updates the proposal status to ongoing
        /// - Updates the proposal deadline
        /// - Updates the proposal receipt status to ongoing
        pub fn submit_proposal(&mut self, proposal_receipt_proof: NonFungibleProof) {
            let receipt_proof = proposal_receipt_proof.check_with_message(
                self.proposal_receipt_manager.address(),
                "Invalid proposal receipt supplied!",
            );

            let receipt = receipt_proof.non_fungible::<ProposalReceipt>().data();
            assert!(
                receipt.status == ProposalStatus::Building,
                "Proposal is not being built!"
            );

            let proposal_id: u64 = receipt.proposal_id;
            let proposal_deadline = self.proposals.get(&proposal_id).unwrap().deadline;
            let too_late: bool = Clock::current_time_rounded_to_seconds()
                .compare(proposal_deadline, TimeComparisonOperator::Gt);

            if too_late {
                let fee_paid: Decimal = self
                    .proposal_receipt_manager
                    .get_non_fungible_data::<ProposalReceipt>(&NonFungibleLocalId::integer(
                        proposal_id,
                    ))
                    .fee_paid;
                let fee_tokens: Bucket = self.proposal_fee_vault.take(fee_paid);
                self.put_tokens(fee_tokens);
                self.proposals.get_mut(&proposal_id).unwrap().status = ProposalStatus::Rejected;
                self.proposal_receipt_manager.update_non_fungible_data(
                    &NonFungibleLocalId::integer(proposal_id),
                    "status",
                    ProposalStatus::Rejected,
                );
            } else {
                let mut proposal = self.proposals.get_mut(&proposal_id).unwrap();

                proposal.status = ProposalStatus::Ongoing;
                proposal.deadline = Clock::current_time_rounded_to_seconds()
                    .add_minutes(self.parameters.proposal_duration)
                    .unwrap();

                self.proposal_receipt_manager.update_non_fungible_data(
                    &NonFungibleLocalId::integer(proposal_id),
                    "status",
                    proposal.status,
                );
            }
        }

        /// Votes on a proposal.
        ///
        /// # Input
        /// - `proposal_id`: ID of the proposal to vote on
        /// - `for_against`: Whether to vote for or against the proposal
        /// - `voting_id_proof`: Proof of the voting ID to use for voting
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Checks whether the proposal is ongoing or in veto mode, so whether it's even votable
        /// - If voted for, checks whether the proposal is not in veto mode (and whether < 1 day is left), if both are the case, the proposal can't be voted for on!
        /// - If the proposal hasn't entered the last day yet, checks whether it is now in the last day, if so, checks whether the proposal has failed, and if so, enters veto mode
        /// - Gets ID from the voting ID proof
        /// - Checks if the voting period has passed
        /// - Checks if the user has already voted on this proposal
        ///    - if so, checks if the user is changing their vote, which isn't allowed
        /// - Checks if the proposal is ongoing
        /// - Calculates vote power
        /// - Adds the vote to the proposal
        /// - If in last day, checks if the proposal has failed, and if so, enters veto mode

        pub fn vote_on_proposal(
            &mut self,
            proposal_id: u64,
            for_against: bool,
            voting_id_proof: NonFungibleProof,
        ) {
            let mut proposal = self.proposals.get_mut(&proposal_id).unwrap();

            assert!(
                proposal.status == ProposalStatus::Ongoing
                    || proposal.status == ProposalStatus::VetoMode,
                "Proposal not ongoing!"
            );

            if proposal.status == ProposalStatus::VetoMode
                && Clock::current_time_is_at_or_after(
                    proposal.deadline.add_minutes(-1).unwrap(),
                    TimePrecision::Second,
                )
            {
                assert!(
                    !for_against,
                    "Proposal in veto mode, impossible to vote for."
                );
            }

            if Clock::current_time_is_at_or_after(
                proposal.deadline.add_minutes(-1).unwrap(),
                TimePrecision::Second,
            ) && proposal.has_failed_in_last_day.is_none()
                && proposal.status == ProposalStatus::Ongoing
            {
                if proposal.votes_for
                    > self.parameters.approval_threshold
                        * (proposal.votes_for + proposal.votes_against)
                {
                    proposal.has_failed_in_last_day = Some(false);
                } else {
                    proposal.has_failed_in_last_day = Some(true);
                    proposal.status = ProposalStatus::VetoMode;
                    proposal.deadline = proposal.deadline.add_minutes(1).unwrap();
                }
            }

            let id_proof = voting_id_proof
                .check_with_message(self.voting_id_address, "Invalid staking ID supplied!");
            let id: NonFungibleLocalId = id_proof.as_non_fungible().non_fungible_local_id();

            if let Some(vote) = proposal.votes.get(&id) {
                if *vote >= dec!(0) {
                    panic!("You have already voted for this proposal!");
                } else {
                    panic!("You have already voted against this proposal!");
                }
            }

            assert!(
                !Clock::current_time_is_at_or_after(proposal.deadline, TimePrecision::Second),
                "Voting period has passed!"
            );

            let vote_power: Decimal = self
                .vaults
                .get_mut(&self.controller_badge_address)
                .unwrap()
                .as_fungible()
                .authorize_with_amount(dec!("0.75"), || {
                    self.staking
                        .vote(proposal.deadline.add_minutes(1).unwrap(), id.clone())
                });

            if for_against {
                proposal.votes.insert(id.clone(), vote_power);
                proposal.votes_for += vote_power;
            } else {
                proposal.votes.insert(id.clone(), dec!("-1") * vote_power);
                proposal.votes_against += vote_power;
            }

            let proposal_failing: bool = proposal.votes_for
                <= self.parameters.approval_threshold
                    * (proposal.votes_for + proposal.votes_against);

            if proposal.has_failed_in_last_day.is_some()
                && proposal.status == ProposalStatus::Ongoing
                && proposal_failing
            {
                proposal.has_failed_in_last_day = Some(true);
                proposal.deadline = proposal.deadline.add_minutes(1).unwrap();
                proposal.status = ProposalStatus::VetoMode;
            }
        }

        /// Finishes voting on a proposal.
        ///
        /// # Input
        /// - `proposal_id`: ID of the proposal to finish voting on
        /// - `forced_finish`: Whether to force the finish of the voting period (only for testing and will be removed)
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Checks if the proposal is ongoing
        /// - Checks if the voting period has passed
        /// - Checks if the proposal has enough votes to be accepted
        /// - Updates the proposal status (to either Accepted or Rejected)
        pub fn finish_voting(&mut self, proposal_id: u64) {
            let mut accepted: bool = true;
            {
                let mut proposal = self.proposals.get_mut(&proposal_id).unwrap();

                assert!(
                    Clock::current_time_is_at_or_after(proposal.deadline, TimePrecision::Second),
                    "Voting period has not passed yet!"
                );

                assert!(
                    proposal.status == ProposalStatus::Ongoing
                        || proposal.status == ProposalStatus::VetoMode,
                    "Proposal not ongoing!"
                );

                let pool_unit_multiplier = self.staking.get_real_amount(dec!(1));
                let votes_for: Decimal = proposal.votes_for * pool_unit_multiplier;
                let votes_against: Decimal = proposal.votes_against * pool_unit_multiplier;
                let total_votes = votes_against + votes_for;

                if (votes_for > self.parameters.approval_threshold * total_votes)
                    && (total_votes >= self.parameters.quorum)
                {
                    proposal.status = ProposalStatus::Accepted;
                } else {
                    proposal.status = ProposalStatus::Rejected;
                    accepted = false;
                }

                self.proposal_receipt_manager.update_non_fungible_data(
                    &NonFungibleLocalId::integer(proposal_id),
                    "status",
                    proposal.status,
                );
            }
            if accepted == false {
                let fee_paid: Decimal = self
                    .proposal_receipt_manager
                    .get_non_fungible_data::<ProposalReceipt>(&NonFungibleLocalId::integer(
                        proposal_id,
                    ))
                    .fee_paid;
                let fee_tokens: Bucket = self.proposal_fee_vault.take(fee_paid);
                self.put_tokens(fee_tokens);
            }
        }

        /// Executes a step in a proposal.
        ///
        /// # Input
        /// - `proposal_id`: ID of the proposal to execute the step for
        /// - `steps_to_execute`: Number of steps to execute
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Checks if the proposal is accepted
        /// - Checks if the previous step required reentrancy (and whether this has been completed yet)
        /// - Executes the steps
        /// - Updates the proposal status to executed if all steps have been executed
        /// - Handles potentially returned buckets
        pub fn execute_proposal_step(&mut self, proposal_id: u64, steps_to_execute: i64) {
            let mut buckets: Vec<Bucket> = Vec::new();
            let mut reentrancy_happened = false;
            {
                let mut proposal = self.proposals.get_mut(&proposal_id).unwrap();
                assert!(
                    proposal.status == ProposalStatus::Accepted,
                    "Proposal not accepted!"
                );

                assert!(
                    proposal.reentrancy == false,
                    "The previous step required reentrancy! Complete this first by calling the ReentrancyProxy component."
                );

                for _ in 0..steps_to_execute {
                    let step: &ProposalStep = &proposal.steps[proposal.next_index as usize];
                    let component: Global<AnyComponent> = Global::from(step.component);
                    if step.component == self.component_address || step.reentrancy {
                        reentrancy_happened = true;
                        self.vaults
                            .get_mut(&self.controller_badge_address)
                            .unwrap()
                            .as_fungible()
                            .authorize_with_amount(dec!("0.75"), || {
                                self.reentrancy.send_step(
                                    proposal_id,
                                    step.component,
                                    step.method.clone(),
                                    step.args.clone(),
                                );
                            });
                        break;
                    } else {
                        if step.return_bucket {
                            let bucket: Bucket = self
                                .vaults
                                .get_mut(&step.badge)
                                .unwrap()
                                .as_fungible()
                                .authorize_with_amount(dec!("0.75"), || {
                                    component.call::<ScryptoValue, Bucket>(&step.method, &step.args)
                                });
                            buckets.push(bucket);
                        } else {
                            self.vaults
                                .get_mut(&step.badge)
                                .unwrap()
                                .as_fungible()
                                .authorize_with_amount(dec!("0.75"), || {
                                    component.call::<ScryptoValue, ()>(&step.method, &step.args)
                                });
                        }
                    }

                    proposal.next_index += 1;

                    if proposal.next_index as usize == proposal.steps.len() {
                        break;
                    }
                }
                if reentrancy_happened == true {
                    proposal.reentrancy = true;
                } else if proposal.next_index as usize == proposal.steps.len() {
                    proposal.status = ProposalStatus::Executed;
                    self.proposal_receipt_manager.update_non_fungible_data(
                        &NonFungibleLocalId::integer(proposal_id),
                        "status",
                        proposal.status,
                    );
                }
            }

            for bucket in buckets {
                self.put_tokens(bucket);
            }
        }

        /// Finishes a reentrancy step in a proposal.
        ///
        /// This method is only really called by the ReentrancyProxy after it has executed a step, to update within this component that the reentrancy step has been completed.
        ///
        /// # Input
        /// - `proposal_id`: ID of the proposal to finish the reentrancy step for
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Increments the next index of the proposal
        /// - Updates the proposal status to executed if all steps have been executed
        /// - Updates the proposal receipt status to executed if all steps have been executed
        pub fn finish_reentrancy_step(&mut self, proposal_id: u64) {
            let mut proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.reentrancy = false;
            proposal.next_index += 1;

            if proposal.next_index as usize == proposal.steps.len() {
                proposal.status = ProposalStatus::Executed;
                self.proposal_receipt_manager.update_non_fungible_data(
                    &NonFungibleLocalId::integer(proposal_id),
                    "status",
                    proposal.status,
                );
            }
        }

        /// Retrieves the fee paid for a proposal.
        ///
        /// # Input
        /// - `proposal_receipt_proof`: Proof of the proposal receipt to retrieve the fee for
        ///
        /// # Output
        /// - The bucket with the fee paid
        ///
        /// # Logic
        /// - Checks if the proposal receipt is valid
        /// - Checks if the proposal is executed
        /// - Updates the proposal receipt status to finished
        /// - Returns the fee paid
        pub fn retrieve_fee(&mut self, proposal_receipt_proof: NonFungibleProof) -> Bucket {
            let receipt_proof = proposal_receipt_proof.check_with_message(
                self.proposal_receipt_manager.address(),
                "Invalid proposal receipt supplied!",
            );
            let receipt = receipt_proof.non_fungible::<ProposalReceipt>().data();

            assert!(
                receipt.status == ProposalStatus::Executed,
                "Only executed proposals can have their fees refunded!"
            );

            self.proposal_receipt_manager.update_non_fungible_data(
                receipt_proof.non_fungible::<ProposalReceipt>().local_id(),
                "status",
                ProposalStatus::Finished,
            );

            self.proposal_fee_vault.take(receipt.fee_paid)
        }

        pub fn hurry_proposal(&mut self, proposal_id: u64, new_duration: i64) {
            let new_deadline = Clock::current_time_rounded_to_seconds()
                .add_minutes(new_duration)
                .unwrap();
            let mut proposal = self.proposals.get_mut(&proposal_id).unwrap();
            assert!(
                proposal.status == ProposalStatus::Ongoing,
                "Proposal not ongoing!"
            );
            assert!(
                new_deadline.compare(proposal.deadline, TimeComparisonOperator::Lte),
                "New deadline is after old deadline!"
            );
            assert!(new_duration > 0, "New duration is not positive!");
            proposal.deadline = new_deadline;
        }

        ///Sets the new staking component and voting id address
        pub fn set_staking_component(
            &mut self,
            proxy_component: ComponentAddress,
            new_voting_id_address: ResourceAddress,
        ) {
            self.staking = proxy_component.into();
            self.voting_id_address = new_voting_id_address;
        }

        /// Sets new parameters for the governance component.
        pub fn set_parameters(
            &mut self,
            fee: Decimal,
            proposal_duration: i64,
            quorum: Decimal,
            approval_threshold: Decimal,
            maximum_proposal_submit_delay: i64,
        ) {
            assert!(
                maximum_proposal_submit_delay > 0,
                "Maximum proposal submit delay must be positive!"
            );
            assert!(proposal_duration > 0, "Proposal duration must be positive!");
            assert!(quorum > dec!(0), "Quorum must be positive!");
            assert!(
                approval_threshold > dec!(0) && approval_threshold <= dec!(1),
                "Approval threshold must be between 0 and 1!"
            );
            assert!(fee > dec!(0), "Fee must be positive!");
            self.parameters.fee = fee;
            self.parameters.proposal_duration = proposal_duration;
            self.parameters.quorum = quorum;
            self.parameters.approval_threshold = approval_threshold;
            self.parameters.maximum_proposal_submit_delay = maximum_proposal_submit_delay;
        }
    }
}
