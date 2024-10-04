//! # Staking Blueprint

/*!
This blueprint enables advanced staking of a resource, with itself as a reward. It is a modification from the Incentives blueprint from the same package, and most functionality is completely equal.
To understand some design choices, it might be useful to check that one out first.
The modifications made to the incentives blueprint to get to this version are made to allow for the staking rewards are distributed continuously.
This is only possible when the staking rewards are distributed in the same token as the staked tokens, as the rewards are distributed to the staking pool, and the staking pool is the only one that can claim these rewards.
Still, we are using a Staking ID system, where users stake tokens to a staking ID, which is an NFT, to allow for locking of / voting with staked tokens and introduce unstaking delays.

The following description is a copy of the Incentives blueprint description with minimal changes to reflect the changes made in this blueprint:

The system works through a Staking ID. Users stake tokens to a staking ID, which is an NFT.
Staked tokens are put into a mother token pool, which is a OneResourcePool. Rewards are distributed to this pool.
Because the NFT records the user's stake in the entire mother token pool, and the mother token pool grows in value, the user's stake grows in value as well.

The component can easily lock these tokens because the user needs the NFT to claim unstake their tokens.
Unstaking is done by requesting an unstaking receipt, which can be redeemed through the component after a set delay, providing an unstaking delay.
Instead of unstaking, an transfer receipt can be minted, which can be redeemed by another user to transfer the staked tokens to their staking ID.

The 2 main advantages over simple OneResourcePool staking that are accomplished are:
- Staked tokens can be locked.
- An unstaking delay can be set (is technically also possible using the OneResourcePool).

To accomplish this, users now stake their tokens to a staking ID. The staked tokens are then held by the staking component:
- Rewards are claimed through the component, which can distribute any token as a reward.
- The component can easily lock these tokens. Two kind of locks exist
    - A lock for voting in proposals (voting lock).
        - Can be set by a governance component, and cannot be bought out.
    - A lock for unstaking (unstaking lock).
        - Can be bought out by paying a fee.
- Unstaking is done by requesting an unstaking or transfer receipt:
    - Unstaking receipts can be redeemed after a set delay.
    - Transfer receipts can be redeemed by another user to transfer the staked tokens to their staking ID. Tokens remain staked while in the transfer receipt form.

This NFT staking ID approach has some disadvantages over simple OneResourcePool staking:
- Wallet display of staked tokens is more difficult, as staked amounts are stored by an NFT (staking ID). Ideally, users need to use some kind of front-end to see their staked tokens, but the Staking ID shows their total pool token stake in the wallet as well.
- Staked tokens are not liquid, making it impossible to use them in traditional DEXes. Though they are transferable to other user's staking IDs via Transfer Receipts, so a DEX could be built on top of this system. This way, liquidity could be provided while still earning staking fees.
- It is more complex to set up and manage.
*/

use scrypto::prelude::*;

/// NFT receipt structure, minted when an unstake is requested, redeemable after a set delay.
#[derive(ScryptoSbor, NonFungibleData)]
pub struct UnstakeReceipt {
    #[mutable]
    pub amount: Decimal,
    #[mutable]
    pub redemption_time: Instant,
}

/// Staking ID structure, holding staked and locked amounts and date until which they are locked. Also stores the next period to claim rewards (updated after a user has claimed them).
#[derive(ScryptoSbor, NonFungibleData)]
pub struct Id {
    #[mutable]
    pub pool_amount_staked: Decimal,
    #[mutable]
    pub pool_amount_delegated_to_me: Decimal,
    #[mutable]
    pub delegating_voting_power_to: Option<NonFungibleLocalId>,
    #[mutable]
    pub locked_until: Option<Instant>,
    #[mutable]
    pub voting_until: Option<Instant>,
    #[mutable]
    pub undelegating_until: Option<Instant>,
}

/// Lock structure, holding the information about locking options of a token.
#[derive(ScryptoSbor)]
pub struct Lock {
    pub payment: Decimal,
    pub max_duration: i64,
    pub unlock_multiplier: Decimal,
}

/// Stakable unit structure, used by the component to data about a stakable token.
#[derive(ScryptoSbor)]
pub struct StakableUnit {
    pub unstake_delay: i64,
    pub pool_amount_staked: Decimal,
    pub vault: Vault,
    pub reward_amount: Decimal,
    pub lock: Lock,
}

/// Stake transfer receipt structure, minted when a user wants to transfer their staked tokens, redeemable by other users to add these tokens to their own staking ID.
#[derive(ScryptoSbor, NonFungibleData)]
pub struct StakeTransferReceipt {
    pub pool_amount: Decimal,
}

#[blueprint]
#[types(Decimal, Option<NonFungibleLocalId>, Option<Instant>, Instant)]
mod staking {
    enable_method_auth! {
        methods {
            create_id => PUBLIC;
            stake => PUBLIC;
            start_unstake => PUBLIC;
            finish_unstake => PUBLIC;
            update_period => PUBLIC;
            lock_stake => PUBLIC;
            unlock_stake => PUBLIC;
            get_remaining_rewards => PUBLIC;
            delegate_vote => PUBLIC;
            undelegate_vote => PUBLIC;
            put_tokens => PUBLIC;
            get_real_amount => PUBLIC;
            vote => restrict_to: [OWNER];
            remove_tokens => restrict_to: [OWNER];
            edit_stakable => restrict_to: [OWNER];
            set_unstake_delay => restrict_to: [OWNER];
        }
    }

    struct Staking {
        /// resource manager of the stake transfer receipts
        pub stake_transfer_receipt_manager: ResourceManager,
        /// counter for the stake transfer receipts
        pub stake_transfer_receipt_counter: u64,
        /// resource manager of the unstake receipts
        pub unstake_receipt_manager: ResourceManager,
        /// counter for the unstake receipts
        pub unstake_receipt_counter: u64,
        /// resource manager of the staking IDs
        pub id_manager: ResourceManager,
        /// counter for the staking IDs
        pub id_counter: u64,
        /// vault that stores staking rewards
        pub reward_vault: FungibleVault,
        // parameters for staking the token
        pub stakable_unit: StakableUnit,
        ///lsu pool for reward token
        pub mother_pool: Global<OneResourcePool>,
        ///Vault to put unstaked mother tokens in
        pub unstaked_mother_tokens: Vault,
        ///last update, to calculate continuous rewards
        pub last_update: Instant,
        ///address of mother token pool token
        pub pool_token_address: ResourceAddress,
        ///address of mother token
        pub mother_token_address: ResourceAddress,
    }

    impl Staking {
        /// this function instantiates the staking component
        ///
        /// ## INPUT
        /// - `controller`: the address of the controller badge, which will be the owner of the staking component
        /// - `rewards`: the initial rewards the staking component holds
        /// - `period_interval`: the interval in which rewards are distributed in days
        /// - `name`: the name of your project
        /// - `symbol`: the symbol of your project
        ///
        /// ## OUTPUT
        /// - the staking component
        ///
        /// ## LOGIC
        /// - all resource managers are created
        /// - the rewards are put into the reward vault and other values are set appropriately
        /// - the staking component is instantiated
        pub fn new(
            controller: ResourceAddress,
            rewards: Bucket,
            name: String,
            symbol: String,
            staking_id_name: String,
        ) -> (Global<Staking>, ResourceAddress, ResourceAddress) {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Staking::blueprint_id());

            let mother_token_address: ResourceAddress = rewards.resource_address();

            let mother_pool: Global<OneResourcePool> = Blueprint::<OneResourcePool>::instantiate(
                OwnerRole::Fixed(rule!(require(controller))),
                rule!(require(global_caller(component_address))),
                mother_token_address,
                None,
            );

            let pool_metadata: Result<Option<GlobalAddress>, MetadataConversionError> =
                mother_pool.get_metadata("pool_unit");
            let pool_token_address: ResourceAddress;

            if let Ok(Some(address)) = pool_metadata {
                pool_token_address = ResourceAddress::try_from(address).unwrap();
            } else {
                panic!("Mother token pool unit metadata not found.");
            }

            let id_manager = ResourceBuilder::new_integer_non_fungible::<Id>(OwnerRole::Fixed(
                rule!(require(controller)),
            ))
            .metadata(metadata!(
                init {
                    "name" => format!("{} {}", name, staking_id_name), updatable;
                    "symbol" => format!("id{}", symbol), updatable;
                    "description" => format!("A {} recording your stake in the {}.", staking_id_name, name), updatable;
                }
            ))
            .mint_roles(mint_roles!(
                minter => rule!(require(global_caller(component_address))
                || require_amount(
                    dec!("0.75"),
                    controller
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
                    controller
                ));
                non_fungible_data_updater_updater => rule!(deny_all);
            ))
            .create_with_no_initial_supply();

            let stake_transfer_receipt_manager = ResourceBuilder::new_integer_non_fungible::<StakeTransferReceipt>(
                OwnerRole::Fixed(rule!(require(controller))),
            )
            .metadata(metadata!(
                init {
                    "name" => format!("{} {} Transfer Receipt", name, staking_id_name), updatable;
                    "symbol" => format!("idtrans{}", symbol), updatable;
                    "description" => format!("A transfer receipt used for {}'s {}.", name, staking_id_name), updatable;
                }
            ))
            .mint_roles(mint_roles!(
                minter => rule!(require(global_caller(component_address)));
                minter_updater => rule!(deny_all);
            ))
            .burn_roles(burn_roles!(
                burner => rule!(require(global_caller(component_address)));
                burner_updater => rule!(deny_all);
            ))
            .create_with_no_initial_supply();

            let id_address: ResourceAddress = id_manager.address();

            let unstake_receipt_manager =
                ResourceBuilder::new_integer_non_fungible::<UnstakeReceipt>(OwnerRole::Fixed(
                    rule!(require(controller)),
                ))
                .metadata(metadata!(
                    init {
                        "name" => format!("{} {} Unstake Receipt", name, staking_id_name), updatable;
                        "symbol" => format!("unst{}", symbol), updatable;
                        "description" => format!("A receipt for removing stake from {}'s {}.", name, staking_id_name), updatable;
                    }
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(require(global_caller(component_address)));
                    minter_updater => rule!(deny_all);
                ))
                .burn_roles(burn_roles!(
                    burner => rule!(require(global_caller(component_address)));
                    burner_updater => rule!(deny_all);
                ))
                .non_fungible_data_update_roles(non_fungible_data_update_roles!(
                    non_fungible_data_updater => rule!(require(global_caller(component_address)));
                    non_fungible_data_updater_updater => rule!(deny_all);
                ))
                .create_with_no_initial_supply();

            let mother_lock: Lock = Lock {
                payment: dec!("1.001"),
                max_duration: 365i64,
                unlock_multiplier: dec!(2),
            };

            let stakable_unit = StakableUnit {
                unstake_delay: 7,
                pool_amount_staked: dec!(0),
                vault: Vault::new(pool_token_address),
                reward_amount: dec!(10000),
                lock: mother_lock,
            };

            let component = Self {
                id_manager,
                stake_transfer_receipt_manager,
                stake_transfer_receipt_counter: 0,
                unstake_receipt_manager,
                unstake_receipt_counter: 0,
                id_counter: 0,
                reward_vault: FungibleVault::with_bucket(rewards.as_fungible()),
                stakable_unit,
                mother_pool,
                unstaked_mother_tokens: Vault::new(mother_token_address),
                last_update: Clock::current_time_rounded_to_seconds(),
                pool_token_address,
                mother_token_address,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(controller))))
            .with_address(address_reservation)
            .globalize();

            (component, id_address, pool_token_address)
        }

        /// This method updates the component's period and saves the rewards accompanying the period, enabling stakers to claim them.
        ///
        /// ## INPUT
        /// - none
        ///
        /// ## OUTPUT
        /// - none
        ///
        /// ## LOGIC
        /// - the mother token staking rewards are distributed every time the method is called, depending on how many minutes have passed since the last update
        /// - a new value for the last update is set
        pub fn update_period(&mut self) {
            if Clock::current_time_is_strictly_after(self.last_update, TimePrecision::Second) {
                let seconds_since_last_update: i64 = Clock::current_time_rounded_to_seconds()
                    .seconds_since_unix_epoch
                    - self.last_update.seconds_since_unix_epoch;
                let seconds_per_period: i64 = 86400; //one day of seconds
                let reward_fraction: Decimal = self.stakable_unit.reward_amount
                    * Decimal::from(seconds_since_last_update)
                    / Decimal::from(seconds_per_period);

                if self.reward_vault.amount() > reward_fraction {
                    self.mother_pool
                        .protected_deposit(self.reward_vault.take(reward_fraction).into());
                }
                self.last_update = Clock::current_time_rounded_to_seconds();
            }
        }

        /// This method requests an unstake of staked tokens
        ///
        /// ## INPUT
        /// - `id_proof`: the proof of the staking ID
        /// - `amount`: the amount of tokens to unstake
        /// - `stake_transfer`: whether to transfer the staked tokens to another user
        ///
        /// ## OUTPUT
        /// - the unstake receipt / transfer receipt
        ///
        /// ## LOGIC
        /// - the method checks the staking ID
        /// - the method checks the staked amount
        /// - the method checks if the staked tokens are locked or voting (then unstaking is not possible)
        /// - if not, tokens are removed from staking ID stake
        /// - if the user wants to transfer the tokens, a transfer receipt is minted
        /// - if the user wants to unstake the tokens, an unstake receipt is minted and pool tokens are converted to normal mother tokens again.
        pub fn start_unstake(
            &mut self,
            id_proof: NonFungibleProof,
            amount: Decimal,
            stake_transfer: bool,
        ) -> Bucket {
            let id_proof =
                id_proof.check_with_message(self.id_manager.address(), "Invalid Id supplied!");

            let id = id_proof.non_fungible::<Id>().local_id().clone();
            let mut id_data: Id = self.id_manager.get_non_fungible_data(&id);
            let mut unstake_amount: Decimal = amount;

            assert!(
                id_data.pool_amount_staked > dec!(0),
                "No stake available to unstake."
            );

            if let Some(locked_until) = id_data.locked_until {
                assert!(
                    Clock::current_time_is_at_or_after(locked_until, TimePrecision::Second),
                    "You cannot unstake tokens currently locked."
                );
            }

            if let Some(voting_until) = id_data.voting_until {
                assert!(
                    Clock::current_time_is_at_or_after(voting_until, TimePrecision::Second),
                    "You cannot unstake tokens currently voting in a proposal."
                );
            }

            if let Some(undelegating_until) = id_data.undelegating_until {
                assert!(
                    Clock::current_time_is_at_or_after(undelegating_until, TimePrecision::Second),
                    "You cannot unstake tokens currently undelegating.."
                );
            }

            assert!(
                id_data.delegating_voting_power_to.is_none(),
                "Undelegate voting power before unstaking"
            );

            if amount >= id_data.pool_amount_staked {
                unstake_amount = id_data.pool_amount_staked;
                id_data.pool_amount_staked = dec!(0);
            } else {
                id_data.pool_amount_staked -= amount;
            }

            self.stakable_unit.pool_amount_staked -= unstake_amount;

            self.id_manager.update_non_fungible_data(
                &id,
                "pool_amount_staked",
                id_data.pool_amount_staked,
            );

            if stake_transfer {
                let stake_transfer_receipt = StakeTransferReceipt {
                    pool_amount: unstake_amount,
                };
                self.stake_transfer_receipt_counter += 1;
                self.stake_transfer_receipt_manager.mint_non_fungible(
                    &NonFungibleLocalId::integer(self.stake_transfer_receipt_counter),
                    stake_transfer_receipt,
                )
            } else {
                unstake_amount = self.unmake_mother_lsu(unstake_amount);
                let unstake_receipt = UnstakeReceipt {
                    amount: unstake_amount,
                    redemption_time: Clock::current_time_rounded_to_seconds()
                        .add_days(self.stakable_unit.unstake_delay)
                        .unwrap(),
                };
                self.unstake_receipt_counter += 1;
                self.unstake_receipt_manager.mint_non_fungible(
                    &NonFungibleLocalId::integer(self.unstake_receipt_counter),
                    unstake_receipt,
                )
            }
        }

        /// This method finishes an unstake, redeeming the unstaked tokens
        ///
        /// ## INPUT
        /// - `receipt`: the unstake receipt
        ///
        /// ## OUTPUT
        /// - the unstaked tokens
        ///
        /// ## LOGIC
        /// - the method checks the receipt
        /// - the method checks the redemption time
        /// - the method burns the receipt
        /// - the method returns the unstaked tokens
        pub fn finish_unstake(&mut self, receipt: Bucket) -> Bucket {
            assert!(receipt.resource_address() == self.unstake_receipt_manager.address());

            let receipt_data = receipt
                .as_non_fungible()
                .non_fungible::<UnstakeReceipt>()
                .data();

            assert!(
                Clock::current_time_is_at_or_after(
                    receipt_data.redemption_time,
                    TimePrecision::Second
                ),
                "You cannot unstake tokens before the redemption time."
            );

            receipt.burn();
            self.unstaked_mother_tokens.take(receipt_data.amount)
        }

        /// This method creates a new staking ID
        ///
        /// ## INPUT
        /// - none
        ///
        /// ## OUTPUT
        /// - the staking ID
        ///
        /// ## LOGIC
        /// - the method increments the ID counter
        /// - the method creates a new ID
        /// - the method returns the ID
        pub fn create_id(&mut self) -> Bucket {
            self.id_counter += 1;

            let id_data = Id {
                pool_amount_staked: dec!(0),
                pool_amount_delegated_to_me: dec!(0),
                delegating_voting_power_to: None,
                locked_until: None,
                voting_until: None,
                undelegating_until: None,
            };

            let id: Bucket = self
                .id_manager
                .mint_non_fungible(&NonFungibleLocalId::integer(self.id_counter), id_data);

            id
        }

        /// This method stakes tokens to a staking ID
        ///
        /// ## INPUT
        /// - `stake_bucket`: bucket containing either the tokens to stake or a stake transfer receipt
        /// - `id_proof`: the proof of the staking ID
        ///
        /// ## OUTPUT
        /// - an optional staking ID (if none was provided)
        ///
        /// ## LOGIC
        /// - the method checks whether a staking ID is supplied, if not, it creates one
        /// - the method passes the id and stake_bucket to the stake_advanced method
        /// - if the stake_advanced method returns a lock_rewards bucket, the method passes this bucket and the id to the stake_advanced method again, this time with the with_lock_rewards parameter set to false
        pub fn stake(
            &mut self,
            stake_bucket: Bucket,
            id_proof: Option<Proof>,
        ) -> (Option<Bucket>, Option<Bucket>) {
            let id: NonFungibleLocalId;
            let mut id_bucket: Option<Bucket> = None;

            if let Some(id_proof) = id_proof {
                let id_proof =
                    id_proof.check_with_message(self.id_manager.address(), "Invalid Id supplied!");
                id = id_proof
                    .as_non_fungible()
                    .non_fungible::<Id>()
                    .local_id()
                    .clone();
            } else {
                let new_id: Bucket = self.create_id();
                id = new_id
                    .as_non_fungible()
                    .non_fungible::<Id>()
                    .local_id()
                    .clone();
                id_bucket = Some(new_id);
            }

            let lock_rewards: Option<Bucket> = self.stake_advanced(stake_bucket, &id, true);
            if let Some(lock_rewards) = lock_rewards {
                let lock_rewards_empty: Option<Bucket> =
                    self.stake_advanced(lock_rewards, &id, false);
                (id_bucket, lock_rewards_empty)
            } else {
                (id_bucket, None)
            }
        }

        /// This method delegates voting power to another staking ID, making the other ID able to vote with your stake, without getting staking rewards
        ///
        /// ## INPUT
        /// - `id_proof`: the proof of the staking ID
        /// - `delegate_id`: the ID to delegate to
        ///
        /// ## OUTPUT
        /// - none
        ///
        /// ## LOGIC
        /// - the method checks the staking ID
        /// - the method retrieves info on the staking ID and the ID to delegate to
        /// - the method checks whether the staking ID has a stake available to delegate
        /// - the method checks whether the staking ID is currently voting
        /// - the method checks whether the staking ID is currently undelegating
        /// - the method updates the staking ID so that it delegates voting power to the other ID, and is now unable to vote or unstake
        ///     - to stop delegating the undelegate_vote method can be used
        /// - the method updates the other ID so that it receives the delegated voting power
        pub fn delegate_vote(
            &mut self,
            id_proof: NonFungibleProof,
            delegate_id: NonFungibleLocalId,
        ) {
            let id_proof =
                id_proof.check_with_message(self.id_manager.address(), "Invalid Id supplied!");
            let id = id_proof.non_fungible::<Id>().local_id().clone();

            let mut id_data: Id = self.id_manager.get_non_fungible_data(&id);
            let mut delegate_id_data: Id = self.id_manager.get_non_fungible_data(&delegate_id);

            assert!(
                id_data.pool_amount_staked > dec!(0),
                "No stake available to delegate."
            );
            if let Some(voting_until) = id_data.voting_until {
                assert!(
                    Clock::current_time_is_at_or_after(voting_until, TimePrecision::Second),
                    "You cannot delegate tokens currently voting."
                );
            }
            if let Some(undelegating_until) = id_data.undelegating_until {
                assert!(
                    Clock::current_time_is_at_or_after(undelegating_until, TimePrecision::Second),
                    "You cannot delegate tokens currently undelegating."
                );
            }

            id_data.delegating_voting_power_to = Some(delegate_id.clone());
            delegate_id_data.pool_amount_delegated_to_me += id_data.pool_amount_staked;

            self.id_manager.update_non_fungible_data(
                &id,
                "delegating_voting_power_to",
                id_data.delegating_voting_power_to,
            );
            self.id_manager.update_non_fungible_data(
                &delegate_id,
                "pool_amount_delegated_to_me",
                delegate_id_data.pool_amount_delegated_to_me,
            );
        }

        /// This method undelegates voting power from another staking ID
        ///
        /// ## INPUT
        /// - `id_proof`: the proof of the staking ID
        ///
        /// ## OUTPUT
        /// - none
        ///
        /// ## LOGIC
        /// - the method checks the staking ID
        /// - the method retrieves info on the staking ID
        /// - the method checks whether the staking ID is currently delegating
        /// - the method updates the staking ID so that it no longer delegates voting power to the other ID
        ///     - this includes setting the undelegating_until to the other ID's locked_until, so that the staking ID cannot vote or unstake until the other ID's lock is over
        /// - the method updates the other ID so that it no longer receives the delegated voting power
        pub fn undelegate_vote(&mut self, id_proof: NonFungibleProof) {
            let id_proof =
                id_proof.check_with_message(self.id_manager.address(), "Invalid Id supplied!");
            let id = id_proof.non_fungible::<Id>().local_id().clone();
            let mut id_data: Id = self.id_manager.get_non_fungible_data(&id);

            if let Some(delegate_id) = id_data.delegating_voting_power_to {
                let mut delegate_id_data: Id = self.id_manager.get_non_fungible_data(&delegate_id);

                delegate_id_data.pool_amount_delegated_to_me -= id_data.pool_amount_staked;
                id_data.delegating_voting_power_to = None;
                id_data.undelegating_until = delegate_id_data.voting_until;

                self.id_manager.update_non_fungible_data(
                    &delegate_id,
                    "pool_amount_delegated_to_me",
                    delegate_id_data.pool_amount_delegated_to_me,
                );
                self.id_manager.update_non_fungible_data(
                    &id,
                    "delegating_voting_power_to",
                    id_data.delegating_voting_power_to,
                );
                self.id_manager.update_non_fungible_data(
                    &id,
                    "undelegating_until",
                    id_data.undelegating_until,
                );
            } else {
                panic!("No delegation to undelegate.");
            }
        }

        /// This method locks staked tokens for a certain duration and gives rewards for locking them
        ///
        /// ## INPUT
        /// - `id_proof`: the proof of the staking ID
        /// - `days_to_lock`: the duration for which the tokens are locked in days
        ///
        /// ## OUTPUT
        /// - rewards for locking the tokens
        ///
        /// ## LOGIC
        /// - the method checks the staking ID
        /// - the method checks whether the staking ID tokens are already locked
        /// - the method locks the tokens by updating the staking ID
        /// - the method calculates and returns the rewards for locking the tokens
        pub fn lock_stake(
            &mut self,
            id_proof: NonFungibleProof,
            days_to_lock: i64,
            for_reward: bool,
        ) {
            let id_proof =
                id_proof.check_with_message(self.id_manager.address(), "Invalid Id supplied!");
            let id = id_proof.non_fungible::<Id>().local_id().clone();
            let mut id_data: Id = self.id_manager.get_non_fungible_data(&id);

            let real_amount_staked = self.get_real_amount(id_data.pool_amount_staked);
            let new_lock: Instant;
            let stakable = &self.stakable_unit;
            let max_lock: Instant = Clock::current_time_rounded_to_seconds()
                .add_days(stakable.lock.max_duration)
                .unwrap();

            if let Some(locked_until) = id_data.locked_until {
                if locked_until.compare(
                    Clock::current_time_rounded_to_seconds(),
                    TimeComparisonOperator::Gt,
                ) {
                    new_lock = locked_until.add_days(days_to_lock).unwrap();
                } else {
                    new_lock = Clock::current_time_rounded_to_seconds()
                        .add_days(days_to_lock)
                        .unwrap();
                }
            } else {
                new_lock = Clock::current_time_rounded_to_seconds()
                    .add_days(days_to_lock)
                    .unwrap();
            }

            assert!(
                new_lock.compare(max_lock, TimeComparisonOperator::Lte),
                "New lock duration exceeds maximum lock duration."
            );

            id_data.locked_until = Some(new_lock);

            self.id_manager
                .update_non_fungible_data(&id, "locked_until", id_data.locked_until);

            if for_reward {
                let lock_reward: Bucket = self
                    .reward_vault
                    .take(
                        (stakable.lock.payment.checked_powi(days_to_lock).unwrap()
                            * real_amount_staked)
                            - real_amount_staked,
                    )
                    .into();
                self.stake_advanced(lock_reward, &id, false);
            }
        }

        /// This method unlocks locked (and, naturally, staked) tokens for a certain duration against payment that's (probably) worth more than the locking reward
        ///
        /// ## INPUT
        /// - `id_proof`: the proof of the staking ID
        /// - `payment`: the payment for unlocking the tokens
        /// - `days_to_unlock`: the duration that the lock is shortened by in days
        ///
        /// ## OUTPUT
        /// - leftover payment
        ///
        /// ## LOGIC
        /// - the method checks the staking ID
        /// - the method calculates the unlock fee
        /// - the method checks whether the payment is enough, takes it, and stores it in the reward vault
        /// - the method updates the locking time of the tokens
        /// - the method returns leftover unlock fee

        pub fn unlock_stake(
            &mut self,
            id_proof: NonFungibleProof,
            mut payment: Bucket,
            days_to_unlock: i64,
        ) -> Bucket {
            let id_proof =
                id_proof.check_with_message(self.id_manager.address(), "Invalid Id supplied!");
            let id = id_proof.non_fungible::<Id>().local_id().clone();
            let stakable = &self.stakable_unit;
            let mut id_data: Id = self.id_manager.get_non_fungible_data(&id);

            let real_amount_staked = self.get_real_amount(id_data.pool_amount_staked);
            let necessary_payment = stakable.lock.unlock_multiplier
                * ((stakable.lock.payment.checked_powi(days_to_unlock).unwrap()
                    * real_amount_staked)
                    - real_amount_staked);
            assert!(
                payment.amount() >= necessary_payment,
                "Payment is not enough to unlock the tokens."
            );
            let to_use_tokens: Bucket = payment.take(necessary_payment);
            self.mother_pool.protected_deposit(to_use_tokens);

            let new_lock: Instant;
            let min_lock: Instant = Clock::current_time_rounded_to_seconds()
                .add_days(-1)
                .unwrap();

            if let Some(locked_until) = id_data.locked_until {
                new_lock = locked_until.add_days(-days_to_unlock).unwrap();
            } else {
                panic!("Tokens not locked.");
            }

            assert!(
                new_lock.compare(min_lock, TimeComparisonOperator::Gte),
                "Unlocking too many days in the past. You're wasting your payment!"
            );

            id_data.locked_until = Some(new_lock);

            self.id_manager
                .update_non_fungible_data(&id, "locked_until", id_data.locked_until);

            payment
        }

        //===================================================================
        //                          ADMIN METHODS
        //===================================================================

        /// Method puts tokens into the reward vault
        pub fn put_tokens(&mut self, bucket: Bucket) {
            self.reward_vault.put(bucket.as_fungible());
        }

        /// Method removes tokens from the reward vault
        pub fn remove_tokens(&mut self, amount: Decimal) -> Bucket {
            self.reward_vault.take(amount).into()
        }

        /// Method sets the unstake delay, the amount of days a user has to wait before claiming unstaked tokens
        pub fn set_unstake_delay(&mut self, new_delay: i64) {
            assert!(new_delay > 0, "Unstake delay must be positive.");
            assert!(
                new_delay <= self.stakable_unit.unstake_delay * 2 + 1,
                "Unstake delay cannot be more than twice + 1 the current delay."
            );
            self.stakable_unit.unstake_delay = new_delay;
        }

        /// Method edits a stakable resource
        pub fn edit_stakable(
            &mut self,
            reward_amount: Decimal,
            payment: Decimal,
            max_duration: i64,
            unlock_multiplier: Decimal,
        ) {
            let lock: Lock = Lock {
                payment,
                max_duration,
                unlock_multiplier,
            };

            self.stakable_unit.reward_amount = reward_amount;
            self.stakable_unit.lock = lock;
        }

        /// This method locks staked tokens for voting
        ///
        /// ## INPUT
        /// - `address`: the address of the stakable token
        /// - `lock_until`: the date until which the tokens are locked
        /// - `id`: the staking ID
        ///
        /// ## OUTPUT
        /// - none
        ///
        /// ## LOGIC
        /// - the method checks the staking ID
        /// - the method checks whether the staking ID tokens are vote-locked by (un)delegating
        /// - the method updates the voting_until field of the staking ID appropriately

        pub fn vote(&mut self, voting_until: Instant, id: NonFungibleLocalId) -> Decimal {
            let id_data: Id = self.id_manager.get_non_fungible_data(&id);

            assert!(
                id_data.delegating_voting_power_to.is_none(),
                "Cannot vote when your voting power is delegated to another ID."
            );
            if let Some(undelegating_until) = id_data.undelegating_until {
                assert!(
                    Clock::current_time_is_at_or_after(undelegating_until, TimePrecision::Second),
                    "You cannot vote with tokens that are being undelegated."
                );
            }

            let vote_power: Decimal =
                id_data.pool_amount_staked + id_data.pool_amount_delegated_to_me;

            if id_data.voting_until.map_or(true, |voting_until_id| {
                voting_until_id.compare(voting_until, TimeComparisonOperator::Lt)
            }) {
                self.id_manager
                    .update_non_fungible_data(&id, "voting_until", Some(voting_until));
            }

            vote_power
        }

        /// This method gets the amount of tokens still able to be rewarded
        ///
        /// ## INPUT
        /// - none
        ///
        /// ## OUTPUT
        /// - amount of tokens still able to be rewarded
        ///
        /// ## LOGIC
        /// - the method checks the amount of tokens in the reward_vault

        pub fn get_remaining_rewards(&self) -> Decimal {
            self.reward_vault.amount()
        }

        //===================================================================
        //                          HELPER METHODS
        //===================================================================

        /// This method counts the staked tokens and puts them away in the staking component's vault.
        ///
        /// ## INPUT
        /// - `stake_bucket`: the bucket of staked tokens
        ///
        /// ## OUTPUT
        /// - the amount of staked tokens
        /// - the address of the stakable token
        ///
        /// ## LOGIC
        /// - the method checks whether the staked token is a stakable token
        /// - the method puts the staked tokens in the staking component's vault
        /// - the method returns the amount of staked tokens and the address of the stakable token

        fn stake_tokens(&mut self, stake_bucket: Bucket) -> Decimal {
            let address: ResourceAddress = stake_bucket.resource_address();
            assert!(
                address == self.pool_token_address,
                "Token supplied does not match requested stakable token."
            );
            let stake_amount: Decimal = stake_bucket.amount();
            self.stakable_unit.vault.put(stake_bucket);

            stake_amount
        }

        /// This method counts the staked tokens from a transfer receipt and burns it.
        ///
        /// ## INPUT
        /// - `receipt`: the transfer receipt
        ///
        /// ## OUTPUT
        /// - the amount of staked tokens
        /// - the address of the stakable token
        ///
        /// ## LOGIC
        /// - the method extracts the data from the receipt
        /// - the method burns the receipt
        /// - the method returns the amount of staked tokens and the address of the stakable token

        fn stake_transfer_receipt(&mut self, receipt: NonFungibleBucket) -> Decimal {
            let receipt_data = receipt.non_fungible::<StakeTransferReceipt>().data();
            let stake_amount: Decimal = receipt_data.pool_amount;
            receipt.burn();

            stake_amount
        }

        /// This method stakes tokens to a staking ID
        ///
        /// ## INPUT
        /// - `stake_bucket`: bucket containing either the tokens to stake or a stake transfer receipt
        /// - `id_proof`: the proof of the staking ID
        /// - `with_lock_rewards`: whether to calculate lock rewards or not (lock rewards are also staked, when received through staking, and no more lock rewards should be given for that)
        ///
        /// ## OUTPUT
        /// - the id of the staking ID
        ///
        /// ## LOGIC
        /// - the method checks the staking ID
        /// - the method checks if latest rewards have been claimed, if not, the method fails
        /// - the method checks whether it received tokens or a transfer receipt
        /// - the received mother tokens are converted to mother pool tokens
        /// - the method adds the tokens to the internal vault, or burns the transfer receipt
        /// - if the staked tokens are already locked, the method calculates the lock reward and returns it (if with_lock_rewards is true)
        /// - the method updates the staking ID
        fn stake_advanced(
            &mut self,
            mut stake_bucket: Bucket,
            id: &NonFungibleLocalId,
            with_lock_rewards: bool,
        ) -> Option<Bucket> {
            let mut lock_reward_bucket: Option<Bucket> = None;

            let mut id_data: Id = self.id_manager.get_non_fungible_data(id);

            if stake_bucket.resource_address() == self.reward_vault.resource_address() {
                stake_bucket = self.make_mother_lsu(stake_bucket);
            }

            let stake_amount: Decimal;

            if stake_bucket.resource_address() == self.stake_transfer_receipt_manager.address() {
                stake_amount = self.stake_transfer_receipt(stake_bucket.as_non_fungible());
            } else {
                stake_amount = self.stake_tokens(stake_bucket);
            }

            id_data.pool_amount_staked += stake_amount;

            if let Some(locked_until) = id_data.locked_until {
                if with_lock_rewards {
                    let seconds_to_unlock = locked_until.seconds_since_unix_epoch
                        - Clock::current_time_rounded_to_seconds().seconds_since_unix_epoch;
                    let seconds_to_unlock_dec = Decimal::from(seconds_to_unlock);
                    let full_days_to_unlock = (seconds_to_unlock_dec / dec!(86400))
                        .checked_floor()
                        .unwrap();
                    let whole_days_to_unlock: i64 =
                        i64::try_from(full_days_to_unlock.0 / Decimal::ONE.0).unwrap();
                    let real_stake_amount = self.get_real_amount(stake_amount);
                    lock_reward_bucket = Some(
                        self.reward_vault
                            .take(
                                (self
                                    .stakable_unit
                                    .lock
                                    .payment
                                    .checked_powi(whole_days_to_unlock)
                                    .unwrap()
                                    * real_stake_amount)
                                    - real_stake_amount,
                            )
                            .into(),
                    );
                }
            }

            if let Some(delegate_id) = id_data.delegating_voting_power_to {
                let mut delegate_id_data: Id = self.id_manager.get_non_fungible_data(&delegate_id);
                delegate_id_data.pool_amount_delegated_to_me += stake_amount;
                self.id_manager.update_non_fungible_data(
                    &delegate_id,
                    "pool_amount_delegated_to_me",
                    delegate_id_data.pool_amount_delegated_to_me,
                );
            }

            self.id_manager.update_non_fungible_data(
                id,
                "pool_amount_staked",
                id_data.pool_amount_staked,
            );

            self.stakable_unit.pool_amount_staked += stake_amount;

            lock_reward_bucket
        }

        /// This method converts the reward token to an LSU so you don't have to claim rewards manually
        fn make_mother_lsu(&mut self, stake_bucket: Bucket) -> Bucket {
            self.mother_pool.contribute(stake_bucket)
        }

        /// This method converts the LSU back into a fungible token so you can claim rewards manually
        fn unmake_mother_lsu(&mut self, amount: Decimal) -> Decimal {
            let unstake_bucket: Bucket = self.stakable_unit.vault.take(amount);
            let unstaked_mother_token: Bucket = self.mother_pool.redeem(unstake_bucket);
            let amount = unstaked_mother_token.amount();
            self.unstaked_mother_tokens.put(unstaked_mother_token);
            amount
        }

        pub fn get_real_amount(&self, amount: Decimal) -> Decimal {
            self.mother_pool.get_redemption_value(amount)
        }
    }
}
