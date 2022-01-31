//! This module provides an implementation of the `phase0` fork
//! of the consensus spec. The primary entrypoints should be one of
//! the "presets" like `mainnet` or `minimal`.
pub mod beacon_block;
pub mod beacon_state;
pub mod configs;
pub mod fork;
pub mod operations;
pub mod presets;
pub mod state_transition;
pub mod validator;

pub mod mainnet {
    use super::*;

    pub use presets::mainnet::*;
    pub use state_transition::Context;

    pub fn apply_block(
        state: &mut BeaconState,
        signed_block: &mut SignedBeaconBlock,
        context: &Context,
    ) -> Result<(), Error> {
        state_transition::state_transition(state, signed_block, Some(true), context)
    }
}

pub mod minimal {}

pub const BASE_REWARDS_PER_EPOCH: u64 = 4;
pub const DEPOSIT_CONTRACT_TREE_DEPTH: usize = 2usize.pow(5);
pub const JUSTIFICATION_BITS_LENGTH: usize = 4;
