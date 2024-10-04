//! # DAO maker package
//! 
//! This package contains the blueprints to create a DAO governing any number of components, and was originally built to create the ILIS DAO governing the STAB module, forming the Stabilis protocol.
//! 
//! The DAO can be used to govern any number of components, through its ability to execute any method on any component, using a badge held by the DAO to authorize this method call.
//! 
//! This works through a number of components:
//! 
//! - **DAO component**: The main component of the DAO, which holds its treasury, can give out jobs, airdrop/send tokens, and holds all official DAO information. Here, the DAO's governance token is created as well.
//! - **Governance component**: The component that can be used to create proposals, vote on them, and execute them. It holds badges that can be used to authorize method calls suggested in proposals.
//! - **Staking component**: The component that can be used to stake tokens and receive the DAO's governance token as a reward. Tokens can be locked as well, to receive rewards. Staking the governance token here makes it usable to vote on proposals through the Governance component.
//! - **ReentrancyProxy component**: Sometimes the DAO needs to execute methods that require reentrancy, which is difficult using the Radix Engine. These methods are then forced to go through the Reentrancy Proxy.
//! - **Bootstrap component**: At DAO instantiation, a liquidity bootstrap can take place by creating a Balancer style Liquidity Boostrapping Pool (LBP) to distribute the DAO's governance token.
//! 
//! More information on the components can be found in their respective blueprints / modules.

pub mod bootstrap;
pub mod governance;
pub mod staking;
pub mod dao;
pub mod reentrancy;
pub mod incentives;