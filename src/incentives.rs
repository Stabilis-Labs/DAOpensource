//! # Incentives Blueprint

/*!
This blueprint enables advanced staking of resources. Staking rewards are distributed periodically.

The system works through a Staking ID. Users stake tokens to a staking ID, which is an NFT.
The staked tokens are then held by the staking component. Rewards are claimed through the component, which can distribute any token as a reward.
The component can easily lock these tokens.
Unstaking is done by requesting an unstaking receipt, which can be redeemed through the component after a set delay, providing an unstaking delay.
Instead of unstaking, an transfer receipt can be minted, which can be redeemed by another user to transfer the staked tokens to their staking ID.

The 3 main advantages over simple OneResourcePool staking that are accomplished are:
- Staking reward can be a token different from the staked token.
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
- Wallet display of staked tokens is more difficult, as staked amounts are stored by an NFT (staking ID). Ideally, users need to use some kind of front-end to see their staked tokens.
- Staking rewards are distributed periodically, not continuously.
- User needs to claim rewards manually. Though this could be automated in some way.
- Staked tokens are not liquid, making it impossible to use them in traditional DEXes. Though they are transferable to other user's staking IDs via Transfer Receipts, so a DEX could be built on top of this system. This way, liquidity could be provided while still earning staking fees.
- It is more complex to set up and manage.
*/

use scrypto::prelude::*;

/// NFT receipt structure, minted when an unstake is requested, redeemable after a set delay.
#[derive(ScryptoSbor, NonFungibleData)]
pub struct UnstakeReceipt {
    #[mutable]
    pub address: ResourceAddress,
    #[mutable]
    pub amount: Decimal,
    #[mutable]
    pub redemption_time: Instant,
}

/// Staking ID structure, holding staked and locked amounts and date until which they are locked. Also stores the next period to claim rewards (updated after a user has claimed them).
#[derive(ScryptoSbor, NonFungibleData)]
pub struct IncentivesId {
    #[mutable]
    pub resources: HashMap<ResourceAddress, Resource>,
    #[mutable]
    pub next_period: i64,
}

/// Lock structure, holding the information about locking options of a token.
#[derive(ScryptoSbor)]
pub struct Lock {
    pub payment: Decimal,
    pub max_duration: i64,
    pub unlock_payment: Decimal,
}

/// Resource structure, holding information about a staked token within a staking ID.
#[derive(ScryptoSbor, Clone)]
pub struct Resource {
    pub amount_staked: Decimal,
    pub locked_until: Option<Instant>,
    pub voting_until: Option<Instant>,
}

/// Stakable unit structure, used by the component to data about a stakable token.
#[derive(ScryptoSbor)]
pub struct StakableUnit {
    pub address: ResourceAddress,
    pub amount_staked: Decimal,
    pub vault: Vault,
    pub reward_amount: Decimal,
    pub lock: Lock,
    pub rewards: KeyValueStore<i64, Decimal>,
}

/// Stake transfer receipt structure, minted when a user wants to transfer their staked tokens, redeemable by other users to add these tokens to their own staking ID.
#[derive(ScryptoSbor, NonFungibleData)]
pub struct StakeTransferReceipt {
    pub address: ResourceAddress,
    pub amount: Decimal,
}

#[blueprint]
#[types(i64, Decimal, HashMap<ResourceAddress, Resource>, ResourceAddress, Instant)]
mod incentives {
    enable_method_auth! {
        methods {
            create_id => PUBLIC;
            stake => PUBLIC;
            start_unstake => PUBLIC;
            finish_unstake => PUBLIC;
            update_id => PUBLIC;
            update_period => PUBLIC;
            lock_stake => PUBLIC;
            unlock_stake => PUBLIC;
            get_remaining_rewards => PUBLIC;
            put_tokens => PUBLIC;
            vote => restrict_to: [OWNER];
            set_period_interval => restrict_to: [OWNER];
            set_max_claim_delay => restrict_to: [OWNER];
            remove_tokens => restrict_to: [OWNER];
            add_stakable => restrict_to: [OWNER];
            edit_stakable => restrict_to: [OWNER];
            set_next_period_to_now => restrict_to: [OWNER];
            set_unstake_delay => restrict_to: [OWNER];
        }
    }

    struct Incentives {
        /// interval in which rewards are distributed in days
        pub period_interval: i64,
        /// time the next interval starts
        pub next_period: Instant,
        /// current period, starting at 0, incremented after each period_interval
        pub current_period: i64,
        /// maximum amount of weeks rewards are stored for a user, after which they become unclaimable
        pub max_claim_delay: i64,
        /// resource manager of the stake transfer receipts
        pub stake_transfer_receipt_manager: ResourceManager,
        /// counter for the stake transfer receipts
        pub stake_transfer_receipt_counter: u64,
        /// resource manager of the unstake receipts
        pub unstake_receipt_manager: ResourceManager,
        /// counter for the unstake receipts
        pub unstake_receipt_counter: u64,
        /// delay after which unstaked tokens can be redeemed in days
        pub unstake_delay: i64,
        /// resource manager of the staking IDs
        pub id_manager: ResourceManager,
        /// counter for the staking IDs
        pub id_counter: u64,
        /// vault that stores staking rewards
        pub reward_vault: FungibleVault,
        // keyvaluestore, holding stakable units and their data
        pub stakes: HashMap<ResourceAddress, StakableUnit>,
    }

    impl Incentives {
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
            period_interval: i64,
            name: String,
            symbol: String,
            dapp_def_address: GlobalAddress,
            info_url: Url,
            id_icon_url: Url,
            transfer_receipt_icon_url: Url,
            unstake_receipt_icon_url: Url,
        ) -> (Global<Incentives>, ResourceAddress) {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Incentives::blueprint_id());

            let id_manager = ResourceBuilder::new_integer_non_fungible::<IncentivesId>(OwnerRole::Fixed(
                rule!(require(controller)),
            ))
            .metadata(metadata!(
                init {
                    "name" => format!("{} Incentives ID", name), updatable;
                    "symbol" => format!("st{}", symbol), updatable;
                    "description" => format!("An ID recording your incentivized stakes in the {} ecosystem.", name), updatable;
                    "icon_url" => id_icon_url.clone(), updatable;
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
                    "name" => format!("{} Incentive Stake Transfer Receipt", name), updatable;
                    "symbol" => format!("staketr{}", symbol), updatable;
                    "description" => format!("An stake transfer receipt used in the {} ecosystem.", name), updatable;
                    "icon_url" => transfer_receipt_icon_url, updatable;
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
                        "name" => format!("{} Incentive Unstake Receipt", name), updatable;
                        "symbol" => format!("unstake{}", symbol), updatable;
                        "description" => format!("An unstake receipt used in the {} ecosystem.", name), updatable;
                        "icon_url" => unstake_receipt_icon_url, updatable;
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

            let stakes: HashMap<ResourceAddress, StakableUnit> = HashMap::new();

            let component = Self {
                next_period: Clock::current_time_rounded_to_seconds()
                    .add_days(period_interval)
                    .unwrap(),
                period_interval,
                current_period: 0,
                max_claim_delay: 5,
                unstake_delay: 7,
                id_manager,
                stake_transfer_receipt_manager,
                stake_transfer_receipt_counter: 0,
                unstake_receipt_manager,
                unstake_receipt_counter: 0,
                id_counter: 0,
                reward_vault: FungibleVault::with_bucket(rewards.as_fungible()),
                stakes,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(controller))))
            .with_address(address_reservation)
            .metadata(metadata! {
                init {
                    "name" => format!("{} Incentives", name), updatable;
                    "description" => format!("Incentives for the {} ecosystem.", name), updatable;
                    "info_url" => info_url, updatable;
                    "icon_url" => id_icon_url, updatable;
                    "stake_receipt" => id_address, updatable;
                    "dapp_definition" => dapp_def_address, updatable;
                }
            })
            .globalize();

            (component, id_address)
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
        /// - the method calculates the number of extra periods that have passed since the last update, because the method might not be called exactly at the end of a period
        /// - if a period has passed, for each stakable token the rewards are calculated and recorded, reward calculation is relatively simple:
        ///    - every stakable has a total amount of reward per period
        ///    - total reward amount is divided by the total amount staked to get the reward per staked token
        /// - the current period is incremented and the next period is set
        pub fn update_period(&mut self) {
            let extra_periods_dec: Decimal = ((Clock::current_time_rounded_to_seconds()
                .seconds_since_unix_epoch
                - self.next_period.seconds_since_unix_epoch)
                / (Decimal::from(self.period_interval) * dec!(86400)))
            .checked_floor()
            .unwrap();

            let extra_periods: i64 = i64::try_from(extra_periods_dec.0 / Decimal::ONE.0).unwrap();

            if Clock::current_time_is_at_or_after(self.next_period, TimePrecision::Second) {
                for (_address, stakable_unit) in self.stakes.iter_mut() {
                    if stakable_unit.amount_staked > dec!(0) {
                        stakable_unit.rewards.insert(
                            self.current_period,
                            stakable_unit.reward_amount / stakable_unit.amount_staked,
                        );
                    } else {
                        stakable_unit.rewards.insert(self.current_period, dec!(0));
                    }
                }

                self.current_period += 1;
                self.next_period = self
                    .next_period
                    .add_days((1 + extra_periods) * self.period_interval)
                    .unwrap();
            }
        }

        /// This method requests an unstake of staked tokens
        ///
        /// ## INPUT
        /// - `id_proof`: the proof of the staking ID
        /// - `address`: the address of the stakable token
        /// - `amount`: the amount of tokens to unstake
        /// - `stake_transfer`: whether to transfer the staked tokens to another user
        ///
        /// ## OUTPUT
        /// - the unstake receipt / transfer receipt
        ///
        /// ## LOGIC
        /// - the method checks the resource to be unstaked
        /// - the method checks the staking ID
        /// - the method checks the staked amount
        /// - the method checks if the staked tokens are locked (then unstaking is not possible)
        /// - if not, tokens are removed from staking ID stake
        /// - if the user wants to transfer the tokens, a transfer receipt is minted
        /// - if the user wants to unstake the tokens, an unstake receipt is minted
        pub fn start_unstake(
            &mut self,
            id_proof: NonFungibleProof,
            address: ResourceAddress,
            amount: Decimal,
            stake_transfer: bool,
        ) -> Bucket {
            let id_proof = id_proof
                .check_with_message(self.id_manager.address(), "Invalid IncentivesId supplied!");

            let id = id_proof.non_fungible::<IncentivesId>().local_id().clone();
            let id_data: IncentivesId = self.id_manager.get_non_fungible_data(&id);

            let mut unstake_amount: Decimal = amount;
            let mut resource_map = id_data.resources.clone();
            let mut resource = resource_map
                .get(&address)
                .expect("Stakable not found in staking ID.")
                .clone();

            assert!(
                resource.amount_staked > dec!(0),
                "No stake available to unstake."
            );

            if let Some(locked_until) = resource.locked_until {
                assert!(
                    Clock::current_time_is_at_or_after(locked_until, TimePrecision::Second),
                    "You cannot unstake tokens currently locked."
                );
            }

            if let Some(voting_until) = resource.voting_until {
                assert!(
                    Clock::current_time_is_at_or_after(voting_until, TimePrecision::Second),
                    "You cannot unstake tokens currently voting in a proposal."
                );
            }

            if amount >= resource.amount_staked {
                unstake_amount = resource.amount_staked;
                resource.amount_staked = dec!(0);
            } else {
                resource.amount_staked -= amount;
            }

            self.stakes.get_mut(&address).unwrap().amount_staked -= unstake_amount;

            resource_map.insert(address, resource);

            self.id_manager
                .update_non_fungible_data(&id, "resources", resource_map);

            if stake_transfer {
                let stake_transfer_receipt = StakeTransferReceipt {
                    address,
                    amount: unstake_amount,
                };
                self.stake_transfer_receipt_counter += 1;
                self.stake_transfer_receipt_manager.mint_non_fungible(
                    &NonFungibleLocalId::integer(self.stake_transfer_receipt_counter),
                    stake_transfer_receipt,
                )
            } else {
                let unstake_receipt = UnstakeReceipt {
                    address,
                    amount: unstake_amount,
                    redemption_time: Clock::current_time_rounded_to_seconds()
                        .add_days(self.unstake_delay)
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

            self.stakes
                .get_mut(&receipt_data.address)
                .unwrap()
                .vault
                .take_advanced(
                    receipt_data.amount,
                    WithdrawStrategy::Rounded(RoundingMode::ToNegativeInfinity),
                )
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

            let id_data = IncentivesId {
                resources: HashMap::new(),
                next_period: self.current_period + 1,
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
        /// - the method checks the staking ID
        /// - the method checks if latest rewards have been claimed, if not, the method fails
        /// - the method checks whether it received tokens or a transfer receipt
        /// - the method adds tokens to an internal vault, or burns the transfer receipt
        /// - if the staked tokens are locked, the method calculates the lock reward and returns it
        /// - the method updates the staking ID
        pub fn stake(
            &mut self,
            stake_bucket: Bucket,
            id_proof: Option<Proof>,
        ) -> (Option<Bucket>, Option<Bucket>) {
            let id: NonFungibleLocalId;
            let mut id_bucket: Option<Bucket> = None;
            let mut lock_reward_bucket: Option<Bucket> = None;

            if let Some(id_proof) = id_proof {
                let id_proof = id_proof.check_with_message(
                    self.id_manager.address(),
                    "Invalid IncentivesId supplied!",
                );
                id = id_proof
                    .as_non_fungible()
                    .non_fungible::<IncentivesId>()
                    .local_id()
                    .clone();
            } else {
                let new_id: Bucket = self.create_id();
                id = new_id
                    .as_non_fungible()
                    .non_fungible::<IncentivesId>()
                    .local_id()
                    .clone();
                id_bucket = Some(new_id);
            }

            let id_data: IncentivesId = self.id_manager.get_non_fungible_data(&id);
            assert!(
                id_data.next_period > self.current_period,
                "Please claim unclaimed rewards on your ID before staking."
            );

            let stake_amount: Decimal;
            let address: ResourceAddress;

            if stake_bucket.resource_address() == self.stake_transfer_receipt_manager.address() {
                (stake_amount, address) =
                    self.stake_transfer_receipt(stake_bucket.as_non_fungible());
            } else {
                (stake_amount, address) = self.stake_tokens(stake_bucket);
            }

            let mut resource_map = id_data.resources.clone();
            resource_map
                .entry(address)
                .and_modify(|resource| {
                    resource.amount_staked += stake_amount;
                })
                .or_insert(Resource {
                    amount_staked: stake_amount,
                    locked_until: None,
                    voting_until: None,
                });

            if let Some(locked_until) = resource_map
                .get(&address)
                .expect("Stakable not found in staking ID.")
                .locked_until
            {
                if locked_until.compare(
                    Clock::current_time_rounded_to_seconds(),
                    TimeComparisonOperator::Gt,
                ) {
                    let stakable = self.stakes.get(&address).unwrap();
                    let seconds_to_unlock = locked_until.seconds_since_unix_epoch
                        - Clock::current_time_rounded_to_seconds().seconds_since_unix_epoch;
                    let seconds_to_unlock_dec = Decimal::from(seconds_to_unlock);
                    let full_days_to_unlock = (seconds_to_unlock_dec / dec!(86400))
                        .checked_floor()
                        .unwrap();
                    let whole_days_to_unlock: i64 =
                        i64::try_from(full_days_to_unlock.0 / Decimal::ONE.0).unwrap();
                    lock_reward_bucket = Some(
                        self.reward_vault
                            .take(
                                (stakable
                                    .lock
                                    .payment
                                    .checked_powi(whole_days_to_unlock)
                                    .unwrap()
                                    * stake_amount)
                                    - stake_amount,
                            )
                            .into(),
                    );
                }
            }

            self.id_manager
                .update_non_fungible_data(&id, "resources", resource_map);

            self.stakes.get_mut(&address).unwrap().amount_staked += stake_amount;

            self.id_manager
                .update_non_fungible_data(&id, "next_period", self.current_period + 1);

            (id_bucket, lock_reward_bucket)
        }

        /// This method claims rewards from a staking ID
        ///
        /// ## INPUT
        /// - `id_proof`: the proof of the staking ID
        ///
        /// ## OUTPUT
        /// - the claimed rewards
        ///
        /// ## LOGIC
        /// - the method updates the component period if necessary
        /// - the method checks the staking ID
        /// - the method checks amount of unclaimed periods
        /// - the method iterates over all staked tokens and calculates the rewards
        /// - the method updates the staking ID to the next period
        /// - the method returns the claimed rewards
        pub fn update_id(&mut self, id_proof: NonFungibleProof) -> FungibleBucket {
            self.update_period();
            let id_proof = id_proof
                .check_with_message(self.id_manager.address(), "Invalid IncentivesId supplied!");
            let id = id_proof.non_fungible::<IncentivesId>().local_id().clone();
            let id_data: IncentivesId = self.id_manager.get_non_fungible_data(&id);

            let mut claimed_weeks: i64 = self.current_period - id_data.next_period + 1;
            if claimed_weeks > self.max_claim_delay {
                claimed_weeks = self.max_claim_delay;
            }

            assert!(claimed_weeks > 0, "Wait longer to claim your rewards.");

            let mut staking_reward: Decimal = dec!(0);

            self.id_manager
                .update_non_fungible_data(&id, "next_period", self.current_period + 1);

            for (address, stakable_unit) in self.stakes.iter() {
                for week in 1..(claimed_weeks + 1) {
                    if stakable_unit
                        .rewards
                        .get(&(self.current_period - week))
                        .is_some()
                    {
                        staking_reward += *stakable_unit
                            .rewards
                            .get(&(self.current_period - week))
                            .unwrap()
                            * id_data
                                .resources
                                .get(address)
                                .map_or(dec!(0), |resource| resource.amount_staked);
                    }
                }
            }

            self.reward_vault.take(staking_reward)
        }

        /// This method locks staked tokens for a certain duration and gives rewards for locking them
        ///
        /// ## INPUT
        /// - `address`: the address of the stakable token
        /// - `id_proof`: the proof of the staking ID
        /// - `days_to_lock`: the duration for which the tokens are locked in days
        ///
        /// ## OUTPUT
        /// - rewards for locking the tokens
        ///
        /// ## LOGIC
        /// - the method checks the staking ID
        /// - the method checks whether this resource address is lockable
        /// - the method checks whether the staking ID tokens are already locked
        /// - the method locks the tokens by updating the staking ID
        /// - the method calculates and returns the rewards for locking the tokens

        pub fn lock_stake(
            &mut self,
            address: ResourceAddress,
            id_proof: NonFungibleProof,
            days_to_lock: i64,
        ) -> FungibleBucket {
            let id_proof = id_proof
                .check_with_message(self.id_manager.address(), "Invalid IncentivesId supplied!");
            let id = id_proof.non_fungible::<IncentivesId>().local_id().clone();
            let stakable = self.stakes.get(&address).unwrap();

            let id_data: IncentivesId = self.id_manager.get_non_fungible_data(&id);
            let mut resource_map = id_data.resources.clone();
            let mut resource = resource_map
                .get(&address)
                .expect("Stakable not found in staking ID.")
                .clone();

            let amount_staked = resource.amount_staked;
            let new_lock: Instant;
            let max_lock: Instant = Clock::current_time_rounded_to_seconds()
                .add_days(stakable.lock.max_duration)
                .unwrap();

            if let Some(locked_until) = resource.locked_until {
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

            resource.locked_until = Some(new_lock);
            resource_map.insert(address, resource);

            self.id_manager
                .update_non_fungible_data(&id, "resources", resource_map);

            self.reward_vault.take(
                (stakable.lock.payment.checked_powi(days_to_lock).unwrap() * amount_staked)
                    - amount_staked,
            )
        }

        /// This method unlocks locked (and, naturally, staked) tokens for a certain duration against payment that's (probably) worth more than the locking reward
        ///
        /// ## INPUT
        /// - `address`: the address of the stakable token
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
        /// - the method checks whether the payment is enough, takes it, and puts it into the reward vault
        /// - the method updates the locking time of the tokens
        /// - the method returns leftover unlock fee

        pub fn unlock_stake(
            &mut self,
            address: ResourceAddress,
            id_proof: NonFungibleProof,
            mut payment: Bucket,
            days_to_unlock: i64,
        ) -> Bucket {
            let id_proof = id_proof
                .check_with_message(self.id_manager.address(), "Invalid IncentivesId supplied!");
            let id = id_proof.non_fungible::<IncentivesId>().local_id().clone();
            let stakable = self.stakes.get(&address).unwrap();

            let id_data: IncentivesId = self.id_manager.get_non_fungible_data(&id);
            let mut resource_map = id_data.resources.clone();
            let mut resource = resource_map
                .get(&address)
                .expect("Stakable not found in staking ID.")
                .clone();

            let amount_staked = resource.amount_staked;
            let necessary_payment =
                (stakable.lock.unlock_payment.checked_powi(days_to_unlock).unwrap() * amount_staked)
                    - amount_staked;
            assert!(
                payment.amount() >= necessary_payment,
                "Payment is not enough to unlock the tokens."
            );
            let to_use_tokens: Bucket = payment.take(necessary_payment);
            self.reward_vault.put(to_use_tokens.as_fungible());

            let new_lock: Instant;
            let min_lock: Instant = Clock::current_time_rounded_to_seconds()
                .add_days(-1)
                .unwrap();

            if let Some(locked_until) = resource.locked_until {
                new_lock = locked_until.add_days(-days_to_unlock).unwrap();
            } else {
                panic!("Tokens not locked.");
            }

            assert!(
                new_lock.compare(min_lock, TimeComparisonOperator::Gte),
                "Unlocking too many days in the past. You're wasting your payment!"
            );

            resource.locked_until = Some(new_lock);
            resource_map.insert(address, resource);

            self.id_manager
                .update_non_fungible_data(&id, "resources", resource_map);

            payment
        }

        //===================================================================
        //                          ADMIN METHODS
        //===================================================================

        /// Method sets the period interval
        pub fn set_period_interval(&mut self, new_interval: i64) {
            self.period_interval = new_interval;
        }

        /// Method puts tokens into the reward vault
        pub fn put_tokens(&mut self, bucket: Bucket) {
            self.reward_vault.put(bucket.as_fungible());
        }

        /// Method removes tokens from the reward vault
        pub fn remove_tokens(&mut self, amount: Decimal) -> Bucket {
            self.reward_vault.take(amount).into()
        }

        /// Method sets the max claim delay, the maximum amount of periods a user can wait before claiming rewards
        pub fn set_max_claim_delay(&mut self, new_delay: i64) {
            self.max_claim_delay = new_delay;
        }

        /// Method sets the unstake delay, the amount of days a user has to wait before claiming unstaked tokens
        pub fn set_unstake_delay(&mut self, new_delay: i64) {
            assert!(new_delay > 0, "Unstake delay must be positive.");
            assert!(
                new_delay <= self.unstake_delay * 2 + 1,
                "Unstake delay cannot be more than twice + 1 the current delay."
            );
            self.unstake_delay = new_delay;
        }

        /// Method adds a stakable resource
        pub fn add_stakable(
            &mut self,
            address: ResourceAddress,
            reward_amount: Decimal,
            payment: Decimal,
            max_duration: i64,
            unlock_payment: Decimal,
        ) {
            let lock: Lock = Lock {
                payment,
                max_duration,
                unlock_payment,
            };

            self.stakes.insert(
                address,
                StakableUnit {
                    address,
                    amount_staked: dec!(0),
                    vault: Vault::new(address),
                    reward_amount,
                    lock,
                    rewards: IncentivesKeyValueStore::new_with_registered_type(),
                },
            );
        }

        /// Method edits a stakable resource
        pub fn edit_stakable(
            &mut self,
            address: ResourceAddress,
            reward_amount: Decimal,
            payment: Decimal,
            max_duration: i64,
            unlock_payment: Decimal,
        ) {
            let lock: Lock = Lock {
                payment,
                max_duration,
                unlock_payment,
            };

            self.stakes.get_mut(&address).unwrap().reward_amount = reward_amount;
            self.stakes.get_mut(&address).unwrap().lock = lock;
        }

        /// Method sets next period to now, making rewards come instantly
        pub fn set_next_period_to_now(&mut self) {
            self.next_period = Clock::current_time_rounded_to_seconds();
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
        /// - the method checks whether a DAO is controlling the staking
        /// - the method updates the voting_until field of the staking ID appropriately

        pub fn vote(
            &mut self,
            address: ResourceAddress,
            voting_until: Instant,
            id: NonFungibleLocalId,
        ) -> Decimal {
            let id_data: IncentivesId = self.id_manager.get_non_fungible_data(&id);

            let mut resource_map = id_data.resources.clone();
            let mut resource = resource_map
                .get(&address)
                .expect("Stakable not found in staking ID.")
                .clone();

            let vote_power: Decimal = resource.amount_staked;

            if resource.voting_until.map_or(true, |voting_until_id| {
                voting_until_id.compare(voting_until, TimeComparisonOperator::Lt)
            }) {
                resource.voting_until = Some(voting_until);
                resource_map.insert(address, resource);
                self.id_manager
                    .update_non_fungible_data(&id, "resources", resource_map);
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

        fn stake_tokens(&mut self, stake_bucket: Bucket) -> (Decimal, ResourceAddress) {
            let address: ResourceAddress = stake_bucket.resource_address();
            assert!(
                self.stakes.get(&address).is_some(),
                "Token supplied does not match requested stakable token."
            );
            let stake_amount: Decimal = stake_bucket.amount();
            self.stakes
                .get_mut(&address)
                .unwrap()
                .vault
                .put(stake_bucket);

            (stake_amount, address)
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

        fn stake_transfer_receipt(
            &mut self,
            receipt: NonFungibleBucket,
        ) -> (Decimal, ResourceAddress) {
            let receipt_data = receipt.non_fungible::<StakeTransferReceipt>().data();
            let address: ResourceAddress = receipt_data.address;
            let stake_amount: Decimal = receipt_data.amount;
            receipt.burn();

            (stake_amount, address)
        }
    }
}
