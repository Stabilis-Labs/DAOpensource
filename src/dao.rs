//! # DAO Blueprint
//!
//! The DAO blueprint is the main component of the DAO, holding all the information about the DAO, its employees, and its announcements.
//! It can be used to hire / fire employees. Airdrop (staked) tokens, send tokens, post / remove announcements, and some more.

use crate::bootstrap::bootstrap::*;
use crate::governance::governance::*;
use crate::incentives::incentives::*;
use crate::reentrancy::reentrancy::*;
use crate::staking::staking::*;
use scrypto::prelude::*;

type AnnouncementType = (String, Option<Vec<File>>);

/// Job structure, holding all information about a job in the DAO component.
#[derive(ScryptoSbor)]
pub struct Job {
    pub employee: Option<Global<Account>>,
    pub last_payment: Instant,
    pub salary: Decimal,
    pub salary_token: ResourceAddress,
    pub duration: i64,
    pub recurring: bool,
    pub title: String,
    pub description: String,
}

/// File structure, holding all information to lookup a file stored on the Radix Ledger.
#[derive(ScryptoSbor)]
pub struct File {
    pub kvs_address: String,
    pub component_address: ComponentAddress,
    pub file_hash: String,
}

#[blueprint]
#[types(
    u64,
    String,
    ResourceAddress,
    Vault,
    Vec<u64>,
    Global<Account>,
    Job,
    AnnouncementType,
)]
mod dao {
    enable_method_auth! {
        methods {
            put_tokens => PUBLIC;
            send_tokens => restrict_to: [OWNER];
            take_tokens => restrict_to: [OWNER];
            create_job => restrict_to: [OWNER];
            employ => restrict_to: [OWNER];
            fire => restrict_to: [OWNER];
            airdrop_tokens => restrict_to: [OWNER];
            airdrop_membered_tokens => restrict_to: [OWNER];
            airdrop_staked_tokens => restrict_to: [OWNER];
            post_announcement => restrict_to: [OWNER];
            remove_announcement => restrict_to: [OWNER];
            set_update_reward => restrict_to: [OWNER];
            add_rewarded_call => restrict_to: [OWNER];
            remove_rewarded_calls => restrict_to: [OWNER];
            set_staking_component => restrict_to: [OWNER];
            set_incentives_component => restrict_to: [OWNER];
            add_claimed_website => restrict_to: [OWNER];
            send_salary_to_employee => PUBLIC;
            rewarded_update => PUBLIC;
            use_raised_liquidity => PUBLIC;
            get_token_amount => PUBLIC;
        }
    }

    extern_blueprint! {
        "package_sim1pkgxxxxxxxxxlckerxxxxxxxxxx000208064247xxxxxxxxxpnfcn6",
        WorkaroundForRegistration {}
    }

    struct Dao {
        /// The staking/membership component of the DAO.
        pub staking: Global<Staking>,
        /// The staking component of the DAO.
        pub incentives: Global<Incentives>,
        /// The bootstrap component of the DAO. Used for the initial bootstrapping of liquidity.
        pub bootstrap: Global<LinearBootstrapPool>,
        /// The mother token of the DAO, used to govern it.
        pub mother_token_address: ResourceAddress,
        /// The vaults of the DAO, storing all fungible and non-fungible tokens.
        pub vaults: KeyValueStore<ResourceAddress, Vault>,
        /// Text announcements of the DAO.
        pub text_announcements: KeyValueStore<u64, (String, Option<Vec<File>>)>,
        /// Counter for the text announcements.
        pub text_announcement_counter: u64,
        /// Last time the staking component was updated.
        pub last_update: Instant,
        /// Reward for updating the staking component.
        pub daily_update_reward: Decimal,
        /// Method calls that are rewarded.
        pub rewarded_calls: HashMap<ComponentAddress, Vec<String>>,
        /// Address of the controller badge.
        pub controller_badge_address: ResourceAddress,
        /// AccountLocker used by the DAO to pay people.
        pub payment_locker: Global<AccountLocker>,
        /// Employees of the DAO and their jobs.
        pub employees: KeyValueStore<Global<Account>, Vec<u64>>,
        /// Jobs of the DAO.
        pub jobs: KeyValueStore<u64, Job>,
        /// Counter for jobs
        pub job_counter: u64,
        /// Governance component of the DAO.
        pub governance: Global<Governance>,
        /// Whether to send LBP liq to dex
        pub send_raised_liquidity_to_dex: bool,
        /// The dapp definition of the DAO.
        pub dapp_def_account: Global<Account>,
    }

    impl Dao {
        /// Instantiates a new DAO component.
        ///
        /// # Input
        /// - `mother_token_bucket`: Bucket containing the DAO's governance token (aka mother token).
        /// - `founder_allocation`: Percentage of the total supply to allocate to the founder.
        /// - `bootstrap_allocation`: Percentage of the total supply to allocate to the bootstrap pool.
        /// - `staking_allocation`: Percentage of the total supply to allocate to the staking pool.
        /// - `controller_badge`: Controller badge of the DAO.
        /// - `rewarded_calls`: Method calls that are rewarded.
        /// - `dao_name`: Name of the DAO.
        /// - `dao_token_symbol`: Symbol of the DAO governance token.
        /// - `dao_token_icon_url`: Icon URL of the DAO token.
        /// - `proposal_receipt_icon_url`: Icon URL of the proposal receipt.
        /// - `bootstrap_resource1`: Resource for the bootstrap pool.
        ///
        /// # Output
        /// - The DAO component
        /// - the founder allocation bucket
        /// - a bucket that can't be dropped but will be empty
        /// - the bootstrap badge bucket used to reclaim initial bootstrap funds.
        ///
        /// # Logic
        /// - Instantiate an AccountLocker
        /// - Mint DAO governance tokens (referred to as mother tokens)
        /// - Create the LinearBootstrapPool for the initial bootstrap
        /// - Create the Staking component
        /// - Instantiate the Governance component
        /// - Create the vaults for the mother tokens and store them
        /// - Store the rewarded methods
        /// - Instantiate the DAO component
        pub fn instantiate_dao(
            mut mother_token_bucket: Bucket,
            founder_allocation: Decimal,
            bootstrap_allocation: Decimal,
            staking_allocation: Decimal,
            incentives_allocation: Decimal,
            mut controller_badge: Bucket,
            dao_name: String,
            dao_token_symbol: String,
            bootstrap_resource1: Bucket,
            oci_dapp_definition: ComponentAddress,
            send_raised_liquidity_to_dex: bool,
            bootstrap_length: i64,
            daily_update_reward: Decimal,
            incentive_period_interval: i64,
            info_url: Url,
            proposal_receipt_icon_url: Url,
            id_icon_url: Url,
            transfer_receipt_icon_url: Url,
            unstake_receipt_icon_url: Url,
            dao_logo_url: Url,
        ) -> (
            Global<Dao>,
            Global<Staking>,
            Global<Incentives>,
            Global<Governance>,
            Global<ReentrancyProxy>,
            Global<LinearBootstrapPool>,
            Bucket,
            Option<Bucket>,
            Bucket,
            ResourceAddress,
            ResourceAddress,
            ResourceAddress,
        ) {
            let controller_badge_address: ResourceAddress = controller_badge.resource_address();

            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Dao::blueprint_id());

            let dapp_def_account =
                Blueprint::<Account>::create_advanced(OwnerRole::Updatable(rule!(allow_all)), None); // will reset owner role after dapp def metadata has been set
            let dapp_def_address = GlobalAddress::from(dapp_def_account.address());

            let payment_locker = Blueprint::<AccountLocker>::instantiate(
                OwnerRole::Fixed(rule!(require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
                ))),
                rule!(
                    require_amount(dec!("0.75"), controller_badge.resource_address())
                        || require(global_caller(component_address))
                ),
                rule!(
                    require_amount(dec!("0.75"), controller_badge.resource_address())
                        || require(global_caller(component_address))
                ),
                rule!(
                    require_amount(dec!("0.75"), controller_badge.resource_address())
                        || require(global_caller(component_address))
                ),
                rule!(
                    require_amount(dec!("0.75"), controller_badge.resource_address())
                        || require(global_caller(component_address))
                ),
                None,
            );

            controller_badge.authorize_with_all(|| {
                payment_locker.set_metadata("dapp_definition", dapp_def_address);
            });

            let mother_token_address: ResourceAddress = mother_token_bucket.resource_address();

            let founder_allocation_amount: Decimal =
                founder_allocation * mother_token_bucket.amount();
            let staking_allocation_amount: Decimal =
                staking_allocation * mother_token_bucket.amount();
            let incentives_allocation_amount: Decimal =
                incentives_allocation * mother_token_bucket.amount();
            let bootstrap_allocation_amount: Decimal =
                bootstrap_allocation * mother_token_bucket.amount();

            let (bootstrap, non_bucket, bootstrap_badge): (
                Global<LinearBootstrapPool>,
                Option<Bucket>,
                Bucket,
            ) = LinearBootstrapPool::new(
                bootstrap_resource1,
                mother_token_bucket.take(bootstrap_allocation_amount),
                dec!("0.99"),
                dec!("0.01"),
                dec!("0.5"),
                dec!("0.5"),
                dec!("0.002"),
                bootstrap_length,
                oci_dapp_definition,
                true,
                dapp_def_address,
                info_url.clone(),
                controller_badge_address,
            );

            let (staking, voting_id_address, pool_token_address): (
                Global<Staking>,
                ResourceAddress,
                ResourceAddress,
            ) = Staking::new(
                controller_badge.resource_address(),
                mother_token_bucket.take(staking_allocation_amount),
                dao_name.clone(),
                dao_token_symbol.clone(),
                dapp_def_address,
                info_url.clone(),
                id_icon_url.clone(),
                transfer_receipt_icon_url.clone(),
                unstake_receipt_icon_url.clone(),
            );

            let (incentives, incentive_id_address): (Global<Incentives>, ResourceAddress) =
                Incentives::new(
                    controller_badge.resource_address(),
                    mother_token_bucket.take(incentives_allocation_amount),
                    incentive_period_interval, //period interval
                    dao_name.clone(),
                    dao_token_symbol.clone(),
                    dapp_def_address,
                    info_url.clone(),
                    id_icon_url.clone(),
                    transfer_receipt_icon_url.clone(),
                    unstake_receipt_icon_url.clone(),
                );

            let vaults: KeyValueStore<ResourceAddress, Vault> =
                DaoKeyValueStore::new_with_registered_type();

            let founder_allocation_bucket: Bucket =
                mother_token_bucket.take(founder_allocation_amount);

            vaults.insert(
                mother_token_address,
                Vault::with_bucket(mother_token_bucket),
            );

            vaults.insert(
                controller_badge_address,
                Vault::with_bucket(controller_badge.take(1)),
            );

            let (governance, reentrancy): (Global<Governance>, Global<ReentrancyProxy>) =
                Governance::instantiate_governance(
                    controller_badge,
                    dao_name.clone(),
                    dao_token_symbol,
                    proposal_receipt_icon_url,
                    staking,
                    mother_token_address,
                    pool_token_address,
                    voting_id_address,
                    dapp_def_address,
                    info_url.clone(),
                );

            dapp_def_account.set_metadata("account_type", String::from("dapp definition"));
            dapp_def_account.set_metadata("name", dao_name.to_string());
            dapp_def_account.set_metadata("info_url", info_url.clone());
            dapp_def_account.set_metadata("icon_url", dao_logo_url);
            dapp_def_account.set_metadata("claimed_websites", vec![info_url.clone()]);
            dapp_def_account.set_metadata(
                "claimed_entities",
                vec![
                    GlobalAddress::from(component_address.clone()),
                    GlobalAddress::from(governance.address()),
                    GlobalAddress::from(staking.address()),
                    GlobalAddress::from(incentives.address()),
                    GlobalAddress::from(payment_locker.address()),
                    GlobalAddress::from(bootstrap.address()),
                    GlobalAddress::from(reentrancy.address()),
                ],
            );
            dapp_def_account.set_owner_role(rule!(require(controller_badge_address)));

            let dao = Self {
                payment_locker,
                staking,
                incentives,
                bootstrap,
                mother_token_address,
                vaults,
                text_announcements: DaoKeyValueStore::new_with_registered_type(),
                text_announcement_counter: 0,
                last_update: Clock::current_time_rounded_to_seconds(),
                daily_update_reward,
                rewarded_calls: HashMap::new(),
                controller_badge_address,
                employees: DaoKeyValueStore::new_with_registered_type(),
                jobs: DaoKeyValueStore::new_with_registered_type(),
                job_counter: 0,
                governance,
                send_raised_liquidity_to_dex,
                dapp_def_account,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(controller_badge_address))))
            .with_address(address_reservation)
            .metadata(metadata! {
                init {
                    "name" => dao_name.to_string(), updatable;
                    "info_url" => info_url, updatable;
                    "dapp_definition" => dapp_def_address, updatable;
                }
            })
            .globalize();

            (
                dao,
                staking,
                incentives,
                governance,
                reentrancy,
                bootstrap,
                founder_allocation_bucket,
                non_bucket,
                bootstrap_badge,
                voting_id_address,
                incentive_id_address,
                pool_token_address,
            )
        }

        /// Finishes the bootstrap and stores the resulting tokens in the appropriate vaults
        pub fn use_raised_liquidity(&mut self) {
            let (lp_tokens, mother_tokens, resource1, resource2): (
                Option<Bucket>,
                Option<Bucket>,
                Option<Bucket>,
                Option<Bucket>,
            ) = self
                .vaults
                .get_mut(&self.controller_badge_address)
                .unwrap()
                .as_fungible()
                .authorize_with_amount(dec!(1), || {
                    self.bootstrap
                        .send_raised_liquidity(self.send_raised_liquidity_to_dex)
                });
            if mother_tokens.is_some() {
                self.put_tokens(mother_tokens.unwrap());
            }
            if lp_tokens.is_some() {
                self.put_tokens(lp_tokens.unwrap());
            }
            if resource1.is_some() {
                self.put_tokens(resource1.unwrap());
            }
            if resource2.is_some() {
                self.put_tokens(resource2.unwrap());
            }
        }

        /// Puts tokens into the DAO treasury
        ///
        /// # Input
        /// - `tokens`: Tokens to put into the treasury
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - If the resource address of the tokens is already in the vaults, put the tokens into the vault
        /// - Otherwise, create a new vault with the tokens and store it
        pub fn put_tokens(&mut self, tokens: Bucket) {
            if self.vaults.get(&tokens.resource_address()).is_some() {
                self.vaults
                    .get_mut(&tokens.resource_address())
                    .unwrap()
                    .put(tokens);
            } else {
                self.vaults
                    .insert(tokens.resource_address(), Vault::with_bucket(tokens));
            };
        }

        /// Sends tokens from the DAO treasury to a receiver
        ///
        /// # Input
        /// - `address`: Address of the tokens to send
        /// - `tokens`: Tokens to send
        /// - `receiver_address`: Component address to send tokens to
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Take the tokens from the vault
        /// - Send the tokens to the receiver using the `put_tokens` method of the receiver component
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

        /// Takes tokens from the DAO treasury
        ///
        /// # Input
        /// - `address`: Address of the tokens to take
        /// - `tokens`: Tokens to take
        ///
        /// # Output
        /// - The tokens taken
        ///
        /// # Logic
        /// - Take the tokens from the vault
        /// - Return the tokens taken
        pub fn take_tokens(
            &mut self,
            address: ResourceAddress,
            tokens: ResourceSpecifier,
        ) -> Bucket {
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
            payment
        }

        /// Staking tokens to receive a Membership ID through the Staking component, and then airdropping them using the Payment Locker
        ///
        /// # Input
        /// - `claimants`: Claimants and the amount of tokens to airdrop to them
        /// - `lock_duration`: Duration to lock the tokens for
        /// - `vote_duration`: Duration to vote for (a way to lock the tokens, without ability to unlock)
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Assert that there are less than 21 claimants as airdropping too many at a time fails
        /// - Create a bucket to store the NFTs to airdrop
        /// - Create a map of claimants and their NFTs
        /// - For each claimant, stake the tokens, lock/vote them if necessary, store the NFTs in the created bucket, and add the claimant to the map
        /// - Airdrop the NFTs using the map of claimants and bucket, through the Payment Locker
        pub fn airdrop_membered_tokens(
            &mut self,
            claimants: IndexMap<Global<Account>, Decimal>,
            lock_duration: i64,
            vote_duration: i64,
        ) {
            assert!(
                claimants.len() < 10,
                "Too many accounts to airdrop to! Try at most 10."
            );
            let mut to_airdrop_nfts: Option<Bucket> = None;
            let mut airdrop_map: IndexMap<Global<Account>, ResourceSpecifier> = IndexMap::new();

            for (receiver, amount) in claimants {
                let payment: Bucket = self
                    .vaults
                    .get_mut(&self.mother_token_address)
                    .unwrap()
                    .as_fungible()
                    .take(amount)
                    .into();

                let (id_option, _empty_bucket): (Option<Bucket>, Option<Bucket>) =
                    self.staking.stake(payment, None);
                let staking_id: Bucket = id_option.unwrap();
                let staking_id_id: NonFungibleLocalId =
                    staking_id.as_non_fungible().non_fungible_local_id();

                if lock_duration > 0 {
                    let staking_proof: NonFungibleProof =
                        staking_id.as_non_fungible().create_proof_of_all();
                    self.staking.lock_stake(staking_proof, lock_duration, false);
                }
                if vote_duration > 0 {
                    self.vaults
                        .get_mut(&self.controller_badge_address)
                        .unwrap()
                        .as_fungible()
                        .authorize_with_amount(dec!(1), || {
                            self.staking.vote(
                                Clock::current_time_rounded_to_seconds()
                                    .add_days(vote_duration)
                                    .unwrap(),
                                staking_id_id.clone(),
                            )
                        });
                }
                let mut ids: IndexSet<NonFungibleLocalId> = IndexSet::new();
                ids.insert(staking_id_id);
                airdrop_map.insert(receiver, ResourceSpecifier::NonFungible(ids));

                match &mut to_airdrop_nfts {
                    Some(bucket) => bucket.put(staking_id),
                    None => to_airdrop_nfts = Some(staking_id),
                }
            }
            if let Some(to_airdrop_nfts) = to_airdrop_nfts {
                self.payment_locker
                    .airdrop(airdrop_map, to_airdrop_nfts, true);
            }
        }

        /// Staking tokens to receive a Staking ID through the Staking component, and then airdropping them using the Payment Locker
        ///
        /// # Input
        /// - `claimants`: Claimants and the amount of tokens to airdrop to them
        /// - `address`: Address of the tokens to airdrop
        /// - `lock_duration`: Duration to lock the tokens for
        /// - `vote_duration`: Duration to vote for (a way to lock the tokens, without ability to unlock)
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Assert that there are less than 21 claimants as airdropping too many at a time fails
        /// - Create a bucket to store the NFTs to airdrop
        /// - Create a map of claimants and their NFTs
        /// - For each claimant, stake the tokens, lock/vote them if necessary, store the NFTs in the created bucket, and add the claimant to the map
        /// - Airdrop the NFTs using the map of claimants and bucket, through the Payment Locker
        pub fn airdrop_staked_tokens(
            &mut self,
            claimants: IndexMap<Global<Account>, Decimal>,
            address: ResourceAddress,
            lock_duration: i64,
            vote_duration: i64,
        ) {
            assert!(
                claimants.len() < 10,
                "Too many accounts to airdrop to! Try at most 10."
            );
            let mut to_airdrop_nfts: Option<Bucket> = None;
            let mut airdrop_map: IndexMap<Global<Account>, ResourceSpecifier> = IndexMap::new();

            for (receiver, amount) in claimants {
                let payment: Bucket = self
                    .vaults
                    .get_mut(&address)
                    .unwrap()
                    .as_fungible()
                    .take_advanced(
                        amount,
                        WithdrawStrategy::Rounded(RoundingMode::ToNegativeInfinity),
                    )
                    .into();

                let (id_option, _empty_bucket): (Option<Bucket>, Option<Bucket>) =
                    self.incentives.stake(payment, None);
                let staking_id: Bucket = id_option.unwrap();
                let staking_id_id: NonFungibleLocalId =
                    staking_id.as_non_fungible().non_fungible_local_id();

                if lock_duration > 0 {
                    let staking_proof: NonFungibleProof =
                        staking_id.as_non_fungible().create_proof_of_all();
                    let locking_reward: Bucket = self
                        .incentives
                        .lock_stake(address, staking_proof, lock_duration)
                        .into();
                    self.put_tokens(locking_reward);
                }
                if vote_duration > 0 {
                    self.vaults
                        .get_mut(&self.controller_badge_address)
                        .unwrap()
                        .as_fungible()
                        .authorize_with_amount(dec!(1), || {
                            self.incentives.vote(
                                address,
                                Clock::current_time_rounded_to_seconds()
                                    .add_days(vote_duration)
                                    .unwrap(),
                                staking_id_id.clone(),
                            )
                        });
                }
                let mut ids: IndexSet<NonFungibleLocalId> = IndexSet::new();
                ids.insert(staking_id_id);
                airdrop_map.insert(receiver, ResourceSpecifier::NonFungible(ids));

                match &mut to_airdrop_nfts {
                    Some(bucket) => bucket.put(staking_id),
                    None => to_airdrop_nfts = Some(staking_id),
                }
            }
            if let Some(to_airdrop_nfts) = to_airdrop_nfts {
                self.payment_locker
                    .airdrop(airdrop_map, to_airdrop_nfts, true);
            }
        }

        /// Airdropping tokens through the Payment Locker
        ///
        /// # Input
        /// - `claimants`: Claimants and amount/id of tokens to airdrop to them
        /// - `address`: Address of the tokens to airdrop
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Assert that there are less than 31 claimants as airdropping too many at a time fails
        /// - Create a bucket to store the tokens to airdrop
        /// - For each claimant take their to be airdropped tokens from the vault and put them in the bucket
        /// - Airdrop the tokens using the map of claimants and bucket, through the Payment Locker
        pub fn airdrop_tokens(
            &mut self,
            claimants: IndexMap<Global<Account>, ResourceSpecifier>,
            address: ResourceAddress,
        ) {
            assert!(
                claimants.len() < 15,
                "Too many accounts to airdrop to! Try at most 15."
            );
            let mut to_airdrop_tokens: Option<Bucket> = None;

            for (_receiver, specifier) in &claimants {
                match specifier {
                    ResourceSpecifier::Fungible(amount) => {
                        let payment: Bucket = self
                            .vaults
                            .get_mut(&address)
                            .unwrap()
                            .as_fungible()
                            .take_advanced(
                                *amount,
                                WithdrawStrategy::Rounded(RoundingMode::ToNegativeInfinity),
                            )
                            .into();
                        match &mut to_airdrop_tokens {
                            Some(bucket) => bucket.put(payment),
                            None => to_airdrop_tokens = Some(payment),
                        }
                    }
                    ResourceSpecifier::NonFungible(ids) => {
                        let payment: Bucket = self
                            .vaults
                            .get_mut(&address)
                            .unwrap()
                            .as_non_fungible()
                            .take_non_fungibles(&ids)
                            .into();
                        match &mut to_airdrop_tokens {
                            Some(bucket) => bucket.put(payment),
                            None => to_airdrop_tokens = Some(payment),
                        }
                    }
                }
            }
            if let Some(to_airdrop_tokens) = to_airdrop_tokens {
                self.payment_locker
                    .airdrop(claimants, to_airdrop_tokens, true);
            }
        }

        /// Creates a job (and can immediately employ if so desired)
        ///
        /// # Input
        /// - `job`: Job to create
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - If the job has an employee, add the job to the employee's jobs in the employees KVS
        /// - Insert the job in the jobs KVS.
        pub fn create_job(
            &mut self,
            employee: Option<Global<Account>>,
            salary: Decimal,
            salary_token: ResourceAddress,
            duration: i64,
            recurring: bool,
            title: String,
            description: String,
        ) {
            let job = Job {
                employee,
                last_payment: Clock::current_time_rounded_to_seconds(),
                salary,
                salary_token,
                duration,
                recurring,
                title,
                description,
            };
            if let Some(employee) = job.employee {
                if self.employees.get(&employee).is_some() {
                    self.employees
                        .get_mut(&employee)
                        .unwrap()
                        .push(self.job_counter);
                } else {
                    self.employees.insert(employee, vec![self.job_counter]);
                }
            }
            self.jobs.insert(self.job_counter, job);
            self.job_counter += 1;
        }

        /// Employ a new employee
        ///
        /// # Input
        /// - `job`: Job to employ the employee for
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Check whether job_id corresponds to existing job and job is not taken.
        /// - Assign job to employee in jobs KVS.
        /// - Add the job_id to the employee's jobs in the employees KVS.
        pub fn employ(&mut self, job_id: u64, employee: Global<Account>) {
            assert!(self.jobs.get(&job_id).is_some(), "Job does not exist");
            assert!(
                self.jobs.get(&job_id).unwrap().employee.is_none(),
                "Job is already taken"
            );

            let mut job = self.jobs.get_mut(&job_id).unwrap();
            job.employee = Some(employee);
            job.last_payment = Clock::current_time_rounded_to_seconds();

            if self.employees.get(&employee).is_some() {
                self.employees.get_mut(&employee).unwrap().push(job_id);
            } else {
                self.employees.insert(employee, vec![job_id]);
            }
        }

        /// Send salary to an employee
        ///
        /// # Input
        /// - `employee`: Employee to send the salary to
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Get the employees jobs from the employees KVS
        /// - For each job:
        /// - Calculate the periods worked by the employee
        /// - Take the salary from the vault
        /// - Trying to airdrop the salary to the employee, but storing it in the Payment Locker if it fails
        /// - Update the last payment time of the job
        /// - If the job is not recurring, remove it from the employees jobs (and update job accordingly)
        pub fn send_salary_to_employee(
            &mut self,
            employee: Global<Account>,
            single_job: Option<u64>,
        ) {
            let mut employee_jobs = self.employees.get_mut(&employee).unwrap();
            let mut jobs_to_remove: Vec<u64> = Vec::new();

            for job_id in employee_jobs.iter() {
                if let Some(single_job) = single_job {
                    assert!(
                        employee_jobs.contains(&single_job),
                        "Employee does not have this job"
                    );
                    if single_job != *job_id {
                        continue;
                    }
                }

                let mut job = self.jobs.get_mut(job_id).unwrap();

                let periods_worked: Decimal = ((Clock::current_time_rounded_to_seconds()
                    .seconds_since_unix_epoch
                    - job.last_payment.seconds_since_unix_epoch)
                    / (Decimal::from(job.duration) * dec!(86400)))
                .checked_floor()
                .unwrap();

                let whole_periods_worked: i64 =
                    i64::try_from(periods_worked.0 / Decimal::ONE.0).unwrap();

                if whole_periods_worked > 0 {
                    let payment: Bucket = self
                        .vaults
                        .get_mut(&job.salary_token)
                        .unwrap()
                        .as_fungible()
                        .take_advanced(
                            job.salary * whole_periods_worked,
                            WithdrawStrategy::Rounded(RoundingMode::ToNegativeInfinity),
                        )
                        .into();

                    self.payment_locker.store(employee, payment, true);

                    job.last_payment = job
                        .last_payment
                        .add_days(whole_periods_worked * job.duration)
                        .unwrap();

                    if !job.recurring {
                        job.employee = None;
                        jobs_to_remove.push(*job_id);
                    }
                }
            }

            for job_id in jobs_to_remove {
                employee_jobs.retain(|&x| x != job_id);
            }
        }

        /// Fire an employee
        ///
        /// # Input
        /// - `employee`: Employee to fire
        /// - `salary_modifier`: Modifier for the firing 'bonus'
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Send unclaimed salary to employee
        /// - Take one more salary from the vault, multiplied by the salary_modifier
        /// - Send this final payment to the employee through the Payment Locker
        /// - Remove the job from the employees jobs and modify job accordingly
        pub fn fire(
            &mut self,
            employee: Global<Account>,
            job_id: u64,
            salary_modifier: Option<Decimal>,
        ) {
            self.send_salary_to_employee(employee, Some(job_id));
            let mut job = self.jobs.get_mut(&job_id).expect("Job does not exist");
            let mut employee_jobs = self.employees.get_mut(&employee).unwrap();
            let payment: Bucket = self
                .vaults
                .get_mut(&job.salary_token)
                .unwrap()
                .as_fungible()
                .take_advanced(
                    job.salary * salary_modifier.unwrap_or(dec!(1)),
                    WithdrawStrategy::Rounded(RoundingMode::ToNegativeInfinity),
                )
                .into();

            self.payment_locker.store(employee, payment, true);

            job.employee = None;
            employee_jobs.retain(|&x| x != job_id);
        }

        /// Post an announcement to the DAO
        pub fn post_announcement(&mut self, announcement: String, files: Option<Vec<File>>) {
            self.text_announcements
                .insert(self.text_announcement_counter, (announcement, files));
            self.text_announcement_counter += 1;
        }

        /// Remove an announcement from the DAO
        pub fn remove_announcement(&mut self, announcement_id: u64) {
            self.text_announcements.remove(&announcement_id);
        }

        /// Call the rewarded methods
        ///
        /// # Input
        /// - None
        ///
        /// # Output
        /// - The amount of tokens rewarded
        ///
        /// # Logic
        /// - Calculate the time passed since the last update
        /// - Call all rewarded methods
        /// - Update the staking component (a standard rewarded method)
        pub fn rewarded_update(&mut self) -> Bucket {
            let passed_minutes: Decimal = (Clock::current_time_rounded_to_seconds()
                .seconds_since_unix_epoch
                - self.last_update.seconds_since_unix_epoch)
                / dec!(60);

            for (component_address, methods) in self.rewarded_calls.iter() {
                let component: Global<AnyComponent> = Global::from(component_address.clone());
                for method in methods {
                    component.call_raw::<()>(method, scrypto_args!());
                }
            }
            self.staking.update_period();
            self.incentives.update_period();
            self.last_update = Clock::current_time_rounded_to_seconds();

            self.vaults
                .get_mut(&self.mother_token_address)
                .unwrap()
                .take((passed_minutes * self.daily_update_reward) / (dec!(24) * dec!(60)))
        }

        /// Add a rewarded method call
        pub fn add_rewarded_call(&mut self, component: ComponentAddress, methods: Vec<String>) {
            self.rewarded_calls.insert(component, methods);
        }

        /// Remove a rewarded method call
        pub fn remove_rewarded_calls(&mut self, component: ComponentAddress) {
            self.rewarded_calls.remove(&component);
        }

        /// Set the staking component
        pub fn set_staking_component(&mut self, staking_component: ComponentAddress) {
            self.staking = staking_component.into();
        }

        /// Set the staking component
        pub fn set_incentives_component(&mut self, incentives_component: ComponentAddress) {
            self.incentives = incentives_component.into();
        }

        /// Set the reward for calling the rewarded methods
        pub fn set_update_reward(&mut self, reward: Decimal) {
            self.daily_update_reward = reward;
        }

        /// Get the amount of tokens in possession of the DAO
        pub fn get_token_amount(&self, address: ResourceAddress) -> Decimal {
            self.vaults.get(&address).unwrap().as_fungible().amount()
        }

        /// Adds claimed website to the dapp definition
        pub fn add_claimed_website(&mut self, website: Url) {
            let badge_vault = self
                .vaults
                .get_mut(&self.controller_badge_address)
                .unwrap()
                .as_fungible();
            match self.dapp_def_account.get_metadata("claimed_websites") {
                Ok(Some(claimed_websites)) => {
                    let mut claimed_websites: Vec<Url> = claimed_websites;
                    claimed_websites.push(website);
                    badge_vault.authorize_with_amount(dec!("1"), || {
                        self.dapp_def_account
                            .set_metadata("claimed_websites", claimed_websites);
                    });
                }
                Ok(None) | Err(_) => {
                    badge_vault.authorize_with_amount(dec!("1"), || {
                        self.dapp_def_account
                            .set_metadata("claimed_websites", vec![website]);
                    });
                }
            }
        }
    }
}
