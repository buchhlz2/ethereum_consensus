//! This module provides an implementation of the `altair` fork
//! of the consensus spec. The primary entrypoints should be one of
//! the "presets" like `mainnet` or `minimal`.
mod beacon_block;
mod beacon_state;
pub mod block_processing;
pub mod epoch_processing;
pub mod genesis;
pub mod helpers;
pub mod light_client;
mod presets;
mod sync;
mod validator;

pub mod state_transition;
pub use state_transition::{
    block_processing::*, epoch_processing::*, genesis::*, helpers::*, slot_processing::*, *,
};

pub use beacon_block::*;
pub use beacon_state::*;
pub use presets::Preset;
pub use sync::*;
pub use validator::*;

pub use crate::phase0::{
    Attestation, AttestationData, AttesterSlashing, BeaconBlockHeader, Checkpoint, Deposit,
    DepositData, DepositMessage, Eth1Data, Fork, ForkData, HistoricalBatchAccumulator,
    IndexedAttestation, ProposerSlashing, SignedVoluntaryExit, Validator, BASE_REWARDS_PER_EPOCH,
    DEPOSIT_CONTRACT_TREE_DEPTH, JUSTIFICATION_BITS_LENGTH,
};

pub mod mainnet {
    pub use super::presets::mainnet::*;
}

pub mod minimal {}

pub const TIMELY_SOURCE_FLAG_INDEX: usize = 0;
pub const TIMELY_TARGET_FLAG_INDEX: usize = 1;
pub const TIMELY_HEAD_FLAG_INDEX: usize = 2;
pub const TIMELY_SOURCE_WEIGHT: u64 = 14;
pub const TIMELY_TARGET_WEIGHT: u64 = 26;
pub const TIMELY_HEAD_WEIGHT: u64 = 14;
pub const SYNC_REWARD_WEIGHT: u64 = 2;
pub const PROPOSER_WEIGHT: u64 = 8;
pub const WEIGHT_DENOMINATOR: u64 = 64;
pub const PARTICIPATION_FLAG_WEIGHTS: [u64; 3] = [
    TIMELY_SOURCE_WEIGHT,
    TIMELY_TARGET_WEIGHT,
    TIMELY_HEAD_WEIGHT,
];
