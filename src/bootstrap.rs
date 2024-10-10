//! # Linear Bootstrap Pool Blueprint
//!
//! Blueprint can be used to create a Balancer style linear bootstrap pool, where the weights of the pool change linearly over time.
//! This can be used to distribute tokens in a fair way, while only needed a small initial (liquidity) investment.

use scrypto::prelude::*;

#[blueprint]
#[types(
    u64,
    Vec<(Decimal, (Decimal, Decimal))>,
)]
mod bootstrap {
    enable_method_auth! {
        methods {
            remove_liquidity => PUBLIC;
            get_resource1_price => PUBLIC;
            swap => PUBLIC;
            finish_bootstrap => PUBLIC;
            send_raised_liquidity => restrict_to: [OWNER];
            start_bootstrap => restrict_to: [OWNER];
            reclaim_initial => PUBLIC;
        }
    }

    extern_blueprint! {
        "package_tdx_2_1phmkv5tql452y7eev899qngwesfzjn2zdjdd2efh50e73rtq93ne0q",
        //"package_sim1pkgxxxxxxxxxpackgexxxxxxxxx000726633226xxxxxxxxxlk8hc9",
        //"package_rdx1p5l6dp3slnh9ycd7gk700czwlck9tujn0zpdnd0efw09n2zdnn0lzx",
        BasicPool {
            fn instantiate_with_liquidity(a_bucket: Bucket, b_bucket: Bucket, input_fee_rate: Decimal, dapp_definition: ComponentAddress) -> (Global<BasicPool>, Bucket);
        }

        // mainnet oci dapp definition: account_rdx12x2ecj3kp4mhq9u34xrdh7njzyz0ewcz4szv0jw5jksxxssnjh7z6z
    }

    struct LinearBootstrapPool {
        /// The TwoResourcePool component that holds both sides of the pool
        pool_component: Global<TwoResourcePool>,
        /// Fee to be paid on swaps
        fee: Decimal,
        /// Initial weight of the first resource
        initial_weight1: Decimal,
        /// Initial weight of the second resource
        initial_weight2: Decimal,
        /// Target weight of the first resource
        target_weight1: Decimal,
        /// Target weight of the second resource
        target_weight2: Decimal,
        /// Current weight of the first resource
        weight1: Decimal,
        /// Current weight of the second resource
        weight2: Decimal,
        /// Duration of the bootstrap. Amount of days in which the target_weights are reached.
        duration: i64,
        /// Address of the first resource
        resource1: ResourceAddress,
        /// Address of the second resource
        resource2: ResourceAddress,
        /// Start time of the bootstrap
        start: Option<Instant>,
        /// End time of the bootstrap
        end: Option<Instant>,
        /// Initial amount of the resource with the lowest initial weight
        initial_little_amount: Decimal,
        /// Address of the resource with the lowest initial weight
        initial_little_address: ResourceAddress,
        /// Vault holding the LP tokens
        lp_vault: Vault,
        /// Vault holding the reclaimable resource (will be filled with the initial_little_amount of the resource with the lowest initial weight)
        reclaimable_resource: Vault,
        /// Badge holding the bootstrap badge, used to reclaim the reclaimable resource after the bootstrap is finished
        bootstrap_badge_vault: Vault,
        /// dapp definition
        oci_dapp_definition: ComponentAddress,
        /// a record of all purchases / sells, when they happened and the amount of tokens that are now in the pool
        ledger: KeyValueStore<u64, Vec<(Decimal, (Decimal, Decimal))>>,
        /// counter for the ledger, so a single vec doesn't experience some tasty state explosion...
        ledger_counter: u64,
        /// whether initial contribution is returned to the provider
        refund_initial: bool,
        /// vault for resource 1, after bootstrap has ended
        resource1_vault: Vault,
        /// vault for resource 2, after bootstrap has ended
        resource2_vault: Vault,
        /// vault for mother refund
        mother_refund_vault: Vault,
    }

    impl LinearBootstrapPool {
        /// Instantiates a new LinearBootstrapPool component.
        ///
        /// # Input
        /// - `resource1`: First resource of the pool
        /// - `resource2`: Second resource of the pool
        /// - `initial_weight1`: Initial weight of the first resource
        /// - `initial_weight2`: Initial weight of the second resource
        /// - `target_weight1`: Target weight of the first resource
        /// - `target_weight2`: Target weight of the second resource
        /// - `fee`: Fee to be paid on swaps
        /// - `duration`: Duration of the bootstrap. Amount of days in which the target_weights are reached.
        ///
        /// # Output
        /// - `Global<LinearBootstrapPool>`: The newly instantiated LinearBootstrapPool component
        /// - `Option<Bucket>`: Empty bucket that can't be dropped (resource created by the pool component)
        /// - `Bucket`: Bucket containing the bootstrap badge
        ///
        /// # Logic
        /// - Creating a bootstrap badge to reclaim resources after the bootstrap is finished
        /// - Instantiating a TwoResourcePool component with the given resources
        /// - Contributes the resources to the pool component
        /// - Stores resulting lp tokens in the lp_vault
        pub fn new(
            resource1: Bucket,
            resource2: Bucket,
            initial_weight1: Decimal,
            initial_weight2: Decimal,
            target_weight1: Decimal,
            target_weight2: Decimal,
            fee: Decimal,
            duration: i64,
            oci_dapp_definition: ComponentAddress,
            refund_initial: bool,
            dapp_def_address: GlobalAddress,
            info_url: Url,
        ) -> (Global<LinearBootstrapPool>, Option<Bucket>, Bucket) {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(LinearBootstrapPool::blueprint_id());
            let global_component_caller_badge =
                NonFungibleGlobalId::global_caller_badge(component_address);

            let mut bootstrap_badge: Bucket = ResourceBuilder::new_fungible(OwnerRole::None)
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata(metadata! (
                    init {
                        "name" => "bootstrap badge", locked;
                        "symbol" => "BOOT", locked;
                    }
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(require(global_caller(component_address)));
                    minter_updater => rule!(deny_all);
                ))
                .mint_initial_supply(2)
                .into();

            let ledger: KeyValueStore<u64, Vec<(Decimal, (Decimal, Decimal))>> =
                LinearBootstrapPoolKeyValueStore::new_with_registered_type();
            let ledger_counter: u64 = 0;
            ledger.insert(
                ledger_counter,
                vec![(Decimal::zero(), (resource1.amount(), resource2.amount()))],
            );

            let mut pool_component = Blueprint::<TwoResourcePool>::instantiate(
                OwnerRole::Fixed(rule!(require(global_component_caller_badge.clone()))),
                rule!(
                    require(global_component_caller_badge)
                        || require_amount(dec!("2"), bootstrap_badge.resource_address())
                ),
                (resource1.resource_address(), resource2.resource_address()),
                None,
            );

            let resource1_address = resource1.resource_address();
            let resource2_address = resource2.resource_address();

            let (initial_little_amount, initial_little_address, initial_big_address): (
                Decimal,
                ResourceAddress,
                ResourceAddress,
            ) = if initial_weight1 < initial_weight2 {
                (resource2.amount(), resource2_address, resource1_address)
            } else {
                (resource1.amount(), resource1_address, resource2_address)
            };

            let (lp_bucket, little_idiot_bucket): (Bucket, Option<Bucket>) = bootstrap_badge
                .authorize_with_all(|| pool_component.contribute((resource1, resource2)));

            let component = Self {
                pool_component,
                fee,
                initial_weight1,
                target_weight1,
                target_weight2,
                initial_weight2,
                weight1: initial_weight1,
                weight2: initial_weight2,
                duration,
                resource1: resource1_address,
                resource2: resource2_address,
                start: None,
                end: None,
                initial_little_address,
                initial_little_amount,
                lp_vault: Vault::with_bucket(lp_bucket),
                reclaimable_resource: Vault::new(initial_little_address),
                bootstrap_badge_vault: Vault::with_bucket(bootstrap_badge.take(1)),
                oci_dapp_definition,
                ledger,
                ledger_counter,
                refund_initial,
                resource1_vault: Vault::new(resource1_address),
                resource2_vault: Vault::new(resource2_address),
                mother_refund_vault: Vault::new(initial_big_address),
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(
                bootstrap_badge.resource_address()
            ))))
            .with_address(address_reservation)
            .metadata(metadata! {
                init {
                    "name" => "Linear Bootstrap Pool", updatable;
                    "info_url" => info_url, updatable;
                    "dapp_definition" => dapp_def_address, updatable;
                }
            })
            .globalize();

            (component, little_idiot_bucket, bootstrap_badge)
        }

        /// Removes liquidity from the pool.
        ///
        /// # Input
        /// - `pool_units`: Amount of LP tokens to redeem
        ///
        /// # Output
        /// - `Bucket`: Bucket containing the first resource
        /// - `Bucket`: Bucket containing the second resource
        ///
        /// # Logic
        /// - Updates the weights of the pool
        /// - Redeems the pool units from the pool component
        pub fn remove_liquidity(&mut self, pool_units: Bucket) -> (Bucket, Bucket) {
            self.set_weights();
            self.pool_component.redeem(pool_units)
        }

        /// Swaps one resource for another.
        ///
        /// # Input
        /// - `input_bucket`: Bucket containing the input resource
        ///
        /// # Output
        /// - `Bucket`: Bucket containing the output resource
        ///
        /// # Logic
        /// - Updates the weights of the pool
        /// - Calculates the output amount based on the input amount and the reserves
        /// - Deposits the input resource in the pool
        /// - Withdraws the output resource from the pool
        /// - Calculates the output resource
        /// - Updates the ledger with the new reserves, used to keep track of price history
        /// - Returns the output resource
        pub fn swap(&mut self, input_bucket: Bucket) -> Bucket {
            assert!(self.end.is_none(), "Bootstrap already finished.");
            self.set_weights();
            let mut reserves = self.vault_reserves();

            let input_reserves = reserves
                .swap_remove(&input_bucket.resource_address())
                .expect("Resource does not belong to the pool");
            let (output_resource_address, output_reserves) = reserves.into_iter().next().unwrap();

            let input_amount = input_bucket.amount();

            // Get the weights based on the resource
            let (input_weight, output_weight) = if input_bucket.resource_address() == self.resource1
            {
                (self.weight1, self.weight2)
            } else {
                (self.weight2, self.weight1)
            };

            // Balancer-style swap formula considering weights
            let output_amount =
                (input_amount * output_reserves * output_weight * (dec!("1") - self.fee))
                    / (input_reserves * input_weight
                        + input_amount * output_weight * (dec!("1") - self.fee));

            self.deposit(input_bucket);
            let return_bucket: Bucket = self.withdraw(output_resource_address, output_amount);

            reserves = self.vault_reserves();
            let resource1_reserve = *reserves.get(&self.resource1).unwrap();
            let resource2_reserve = *reserves.get(&self.resource2).unwrap();
            let progress = self.get_progress();

            if self.ledger.get(&self.ledger_counter).is_some() {
                let mut ledger_vector = self.ledger.get_mut(&self.ledger_counter).unwrap();
                if ledger_vector.len() > 99 {
                    self.ledger_counter += 1;
                }
                ledger_vector.push((progress, (resource1_reserve, resource2_reserve)));
            } else {
                self.ledger.insert(
                    self.ledger_counter,
                    vec![(progress, (resource1_reserve, resource2_reserve))],
                );
            }

            if self.get_progress() >= dec!(1) {
                self.finish_bootstrap();
            }

            return_bucket
        }

        /// Returns the price of the first resource in the pool.
        ///
        /// # Input
        /// - None
        ///
        /// # Output
        /// - `Decimal`: Price of the first resource
        ///
        /// # Logic
        /// - Updates the weights of the pool
        /// - Calculates the price of the first resource based on the reserves and the weights
        pub fn get_resource1_price(&mut self) -> Decimal {
            self.set_weights();
            let reserves = self.vault_reserves();
            let resource1_reserve = *reserves.get(&self.resource1).unwrap();
            let resource2_reserve = *reserves.get(&self.resource2).unwrap();
            let weighted_price =
                (resource2_reserve * self.weight2) / (resource1_reserve * self.weight1);
            weighted_price
        }

        /// Finishes the bootstrap.
        ///
        /// # Input
        /// - None
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Redeems the LP tokens from the pool component
        /// - Checks which resource has the initial_little_amount and puts it in the reclaimable_resource vault
        /// - Puts the other resource in the resource vaults and mother refund vault, to send to Dex and DAO respectively
        pub fn finish_bootstrap(&mut self) {
            let progress = self.get_progress();
            assert!(self.end.is_none(), "Bootstrap already finished before.");
            assert!(progress >= dec!(1), "Bootstrap not ready to finish yet.");
            self.end = Some(Clock::current_time_rounded_to_seconds());

            let (mut resource1, mut resource2): (Bucket, Bucket) =
                self.pool_component.redeem(self.lp_vault.take_all());

            if self.refund_initial {
                if resource1.resource_address() == self.initial_little_address {
                    let frac_resource2_resource1 = resource2.amount() / resource1.amount();
                    let mother_refund_bucket: Bucket =
                        resource2.take(self.initial_little_amount * frac_resource2_resource1);
                    self.mother_refund_vault.put(mother_refund_bucket);
                    self.reclaimable_resource
                        .put(resource1.take(self.initial_little_amount));
                } else {
                    let frac_resource1_resource2 = resource1.amount() / resource2.amount();
                    let mother_refund_bucket: Bucket =
                        resource1.take(self.initial_little_amount * frac_resource1_resource2);
                    self.mother_refund_vault.put(mother_refund_bucket);
                    self.reclaimable_resource
                        .put(resource2.take(self.initial_little_amount));
                }
            }

            self.resource1_vault.put(resource1);
            self.resource2_vault.put(resource2);
        }

        /// Sends raised liquidity to DEX and mother token refund to DAO.
        ///
        /// # Input
        /// - None
        ///
        /// # Output
        /// - LP tokens
        /// - Mother refund
        ///
        /// # Logic
        /// - Checks if the bootstrap has already finished
        /// - Sends the LP tokens to the DEX
        /// - Returns mother refund and resulting LP tokens
        pub fn send_raised_liquidity(
            &mut self,
            to_dex: bool,
        ) -> (
            Option<Bucket>,
            Option<Bucket>,
            Option<Bucket>,
            Option<Bucket>,
        ) {
            assert!(self.end.is_some(), "Bootstrap not finished yet.");
            let mut mother_refund: Option<Bucket> = None;
            let mut resource1: Option<Bucket> = None;
            let mut resource2: Option<Bucket> = None;
            let mut lp_tokens: Option<Bucket> = None;

            if self.mother_refund_vault.amount() > dec!(0) {
                mother_refund = Some(self.mother_refund_vault.take_all());
            }
            if to_dex {
                let (_component, oci_lp_tokens) =
                    Blueprint::<BasicPool>::instantiate_with_liquidity(
                        self.resource1_vault.take_all(),
                        self.resource2_vault.take_all(),
                        self.fee,
                        self.oci_dapp_definition,
                    );
                lp_tokens = Some(oci_lp_tokens);
            } else {
                resource1 = Some(self.resource1_vault.take_all());
                resource2 = Some(self.resource2_vault.take_all());
            }

            (lp_tokens, mother_refund, resource1, resource2)
        }

        /// Starts the bootstrap.
        ///
        /// # Input
        /// - None
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Checks if the bootstrap has already started
        /// - Sets the start time of the bootstrap
        pub fn start_bootstrap(&mut self) {
            assert!(self.start.is_none(), "Bootstrap already started");
            self.start = Some(Clock::current_time_rounded_to_seconds());
        }

        /// Reclaims the initial resources.
        ///
        /// # Input
        /// - `boot_badge`: Bucket containing the bootstrap badge
        ///
        /// # Output
        /// - `Bucket`: Bucket containing the initial resources
        ///
        /// # Logic
        /// - Checks if the bootstrap badge is correct
        /// - Puts the bootstrap badge in the bootstrap_badge_vault
        /// - Takes all resources from the reclaimable_resource vault
        pub fn reclaim_initial(&mut self, boot_badge: Bucket) -> Bucket {
            assert!(self.end.is_some(), "Bootstrap not finished yet.");
            assert!(boot_badge.resource_address() == self.bootstrap_badge_vault.resource_address());
            self.bootstrap_badge_vault.put(boot_badge);
            self.reclaimable_resource.take_all()
        }

        fn set_weights(&mut self) {
            let progress: Decimal = self.get_progress();

            if progress >= dec!(1) {
                self.weight1 = self.target_weight1;
                self.weight2 = self.target_weight2;
            } else {
                self.weight1 =
                    self.initial_weight1 + (self.target_weight1 - self.initial_weight1) * progress;
                self.weight2 =
                    self.initial_weight2 + (self.target_weight2 - self.initial_weight2) * progress;
            }
        }

        /// Returns the progress of the bootstrap.
        ///
        /// # Input
        /// - None
        ///
        /// # Output
        /// - `Decimal`: Progress of the bootstrap (0 to 1)
        ///
        /// # Logic
        /// - Calculates the elapsed time since the start of the bootstrap
        /// - Calculates the time to elapse until the end of the bootstrap
        /// - Returns the progress as a decimal between 0 and 1
        fn get_progress(&self) -> Decimal {
            let start = self.start.expect("LBP hasn't started yet.");
            let elapsed_time = Clock::current_time_rounded_to_seconds().seconds_since_unix_epoch
                - start.seconds_since_unix_epoch;
            let time_to_elapse = start
                .add_days(self.duration)
                .unwrap()
                .seconds_since_unix_epoch
                - start.seconds_since_unix_epoch;
            Decimal::from(elapsed_time) / Decimal::from(time_to_elapse)
        }

        /// Returns the reserves of the pool.
        fn vault_reserves(&self) -> IndexMap<ResourceAddress, Decimal> {
            self.pool_component.get_vault_amounts()
        }

        /// Deposits a bucket in the pool.
        fn deposit(&mut self, bucket: Bucket) {
            self.pool_component.protected_deposit(bucket)
        }

        /// Withdraws a bucket from the pool.
        fn withdraw(&mut self, resource_address: ResourceAddress, amount: Decimal) -> Bucket {
            self.pool_component.protected_withdraw(
                resource_address,
                amount,
                WithdrawStrategy::Rounded(RoundingMode::ToZero),
            )
        }
    }
}
