//! # Reentrancy Proxy Blueprint
//!
//! Blueprint used to instantiate a ReentrancyProxy component. Through which proposals which would require reentrancy can be executed.
//!
//! The Radix Engine prevents reentrancy by default. So, when a proposal needs to be executed, but it wants to call back into the component, it can't do so directly. Instead, it can use the ReentrancyProxy component to do so.
//! To do this, it sends the ProposalStep to the ReentrancyProxy, which stores it. Then, the ReentrancyProxy can be called to execute the ProposalStep.
//! While the ProposalStep is within the ReentrancyProxy, the proposal cannot be executed further until the ProposalStep is completed.

use scrypto::prelude::*;

type ReentrancyStep = (ScryptoValue, ComponentAddress, String);

#[blueprint]
#[types(u64, ReentrancyStep)]
mod reentrancy {
    enable_method_auth! {
        methods {
            call => PUBLIC;
            send_step => restrict_to: [OWNER];
        }
    }

    /// ReentrancyProxy component, used to execute ProposalSteps that require reentrancy.
    struct ReentrancyProxy {
        ///KVS storing all ProposalSteps to execute as through the ReentrancyProxy, indexed by the proposal ID.
        reentrancies: KeyValueStore<u64, (ScryptoValue, ComponentAddress, String)>,
        ///Badge vault used to authorize the calling of the ProposalSteps. Currently only used for the controller badge of the Governance component.
        badge_vault: Vault,
    }

    impl ReentrancyProxy {
        /// Instantiates a new ReentrancyProxy component.
        ///
        ///  # Input
        /// - `badge`: Badge to use for the badge vault, allowing access to owner methods of the governance component
        ///
        /// # Output
        /// - `Global<ReentrancyProxy>`: The newly instantiated ReentrancyProxy component
        ///
        /// # Logic
        /// - Instantiates a new ReentrancyProxy component with the given badge
        pub fn new(badge: Bucket) -> Global<ReentrancyProxy> {
            let badge_address = badge.resource_address();
            Self {
                reentrancies: ReentrancyProxyKeyValueStore::new_with_registered_type(),
                badge_vault: Vault::with_bucket(badge),
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(badge_address))))
            .globalize()
        }

        /// Sends a ProposalStep to the ReentrancyProxy to be executed.
        ///
        /// # Input
        /// - `proposal_id`: ID of the proposal the step is for
        /// - `component`: Address of the component to call
        /// - `method`: Method to call on the component
        /// - `args`: Arguments to pass to the method
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Stores the ProposalStep in the reentrancies KVS, indexed by the proposal ID
        ///     - This method is called by the Governance component when a proposal step needs to be executed that requires reentrancy
        pub fn send_step(
            &mut self,
            proposal_id: u64,
            component: ComponentAddress,
            method: String,
            args: ScryptoValue,
        ) {
            self.reentrancies
                .insert(proposal_id, (args, component, method));
        }

        /// Executes a ProposalStep stored in the ReentrancyProxy.
        ///
        /// # Input
        /// - `proposal_id`: ID of the proposal to execute the step for
        ///
        /// # Output
        /// - None
        ///
        /// # Logic
        /// - Retrieves the ProposalStep from the reentrancies KVS
        /// - Calls the component with the given method and arguments (and badge authorization)
        /// - Removes the ProposalStep from the reentrancies KVS
        /// - Calls the governance component with the `finish_reentrancy_step` to allow for other steps to be executed again
        pub fn call(&mut self, proposal_id: u64) {
            let (args, component_address, method): (ScryptoValue, ComponentAddress, String) =
                self.reentrancies.get(&proposal_id).unwrap().clone();
            let component: Global<AnyComponent> = Global::from(component_address);
            self.badge_vault
                .as_fungible()
                .authorize_with_amount(dec!("1"), || {
                    component.call::<ScryptoValue, ()>(&method, &args)
                });
            self.reentrancies.remove(&proposal_id);
            self.badge_vault
                .as_fungible()
                .authorize_with_amount(dec!("1"), || {
                    component.call_raw::<()>("finish_reentrancy_step", scrypto_args!(proposal_id))
                });
        }
    }
}
