// TODO remove once impl is added
#![allow(dead_code)]
#![allow(unused_mut)]
#![allow(unused_variables)]
use crate::altair as spec;

use crate::crypto::fast_aggregate_verify;
use crate::domains::DomainType;
use crate::primitives::{BlsPublicKey, ParticipationFlags, ValidatorIndex};
use crate::signing::compute_signing_root;
use crate::state_transition::{
    invalid_operation_error, Context, InvalidAttestation, InvalidDeposit, InvalidOperation,
    InvalidSyncAggregate, Result,
};
use spec::{
    add_flag, compute_domain, compute_epoch_at_slot, decrease_balance,
    get_attestation_participation_flag_indices, get_attesting_indices,
    get_base_reward_per_increment, get_beacon_committee, get_beacon_proposer_index,
    get_block_root_at_slot, get_committee_count_per_slot, get_current_epoch, get_domain,
    get_indexed_attestation, get_previous_epoch, get_total_active_balance,
    get_validator_from_deposit, has_flag, increase_balance, is_valid_indexed_attestation,
    process_block_header, process_eth1_data, process_operations, process_randao, Attestation,
    BeaconBlock, BeaconState, Deposit, DepositMessage, SyncAggregate,
};
use ssz_rs::prelude::*;
use std::collections::HashSet;
use std::iter::zip;

pub fn process_attestation<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
>(
    state: &mut BeaconState<
        SLOTS_PER_HISTORICAL_ROOT,
        HISTORICAL_ROOTS_LIMIT,
        ETH1_DATA_VOTES_BOUND,
        VALIDATOR_REGISTRY_LIMIT,
        EPOCHS_PER_HISTORICAL_VECTOR,
        EPOCHS_PER_SLASHINGS_VECTOR,
        MAX_VALIDATORS_PER_COMMITTEE,
        SYNC_COMMITTEE_SIZE,
    >,
    attestation: &Attestation<MAX_VALIDATORS_PER_COMMITTEE>,
    context: &Context,
) -> Result<()> {
    let data = &attestation.data;

    let is_previous = data.target.epoch == get_previous_epoch(state, context);
    let current_epoch = get_current_epoch(state, context);
    let is_current = data.target.epoch == current_epoch;
    let valid_target_epoch = is_previous || is_current;
    if !valid_target_epoch {
        return Err(invalid_operation_error(InvalidOperation::Attestation(
            InvalidAttestation::InvalidTargetEpoch {
                target: data.target.epoch,
                current: current_epoch,
            },
        )));
    }

    let attestation_epoch = compute_epoch_at_slot(data.slot, context);
    if data.target.epoch != attestation_epoch {
        return Err(invalid_operation_error(InvalidOperation::Attestation(
            InvalidAttestation::InvalidSlot {
                slot: data.slot,
                epoch: attestation_epoch,
                target: data.target.epoch,
            },
        )));
    }

    let attestation_has_delay = data.slot + context.min_attestation_inclusion_delay <= state.slot;
    let attestation_is_recent = state.slot <= data.slot + context.slots_per_epoch;
    let attestation_is_timely = attestation_has_delay && attestation_is_recent;
    if !attestation_is_timely {
        return Err(invalid_operation_error(InvalidOperation::Attestation(
            InvalidAttestation::NotTimely {
                state_slot: state.slot,
                attestation_slot: data.slot,
                lower_bound: data.slot + context.slots_per_epoch,
                upper_bound: data.slot + context.min_attestation_inclusion_delay,
            },
        )));
    }

    let committee_count = get_committee_count_per_slot(state, data.target.epoch, context);
    if data.index >= committee_count {
        return Err(invalid_operation_error(InvalidOperation::Attestation(
            InvalidAttestation::InvalidIndex {
                index: data.index,
                upper_bound: committee_count,
            },
        )));
    }

    let committee = get_beacon_committee(state, data.slot, data.index, context)?;
    if attestation.aggregation_bits.len() != committee.len() {
        return Err(invalid_operation_error(InvalidOperation::Attestation(
            InvalidAttestation::Bitfield {
                expected_length: committee.len(),
                length: attestation.aggregation_bits.len(),
            },
        )));
    }

    // Participation flag indices
    let inclusion_delay = state.slot - data.slot;
    let participation_flag_indices =
        get_attestation_participation_flag_indices(state, data, inclusion_delay, context)?;

    // Verify signature
    let _ = is_valid_indexed_attestation(
        state,
        &mut get_indexed_attestation(state, attestation, context)?,
        context,
    )?;

    // Update epoch participation flags
    // @dev deviate from the order of the spec to avoid immutable borrow after mutable borrow
    let attesting_indices =
        get_attesting_indices(state, data, &attestation.aggregation_bits, context)?;

    let epoch_participation = if is_current {
        &mut state.current_epoch_participation
    } else {
        &mut state.previous_epoch_participation
    };

    let mut proposer_reward_numerator = 0;
    // @dev this is where I start having trouble
    // the `get_base_reward` below borrows state as immutable, but `epoch_participation` above borrows as mutable
    for index in attesting_indices {
        for (flag_index, weight) in crate::altair::PARTICIPATION_FLAG_WEIGHTS.iter().enumerate() {
            // if flag_index in participation_flag_indices and not has_flag(epoch_participation[index], flag_index):
            if participation_flag_indices.contains(&flag_index)
                && !has_flag(epoch_participation[index], flag_index as u8)
            {
                epoch_participation[index] = add_flag(epoch_participation[index], flag_index as u8);
                // @dev explicit import of `get_base_reward` to disambiguate instead of `spec` import
                // also, this is where the issue with mutable vs immutable borrow occurs
                /*
                proposer_reward_numerator +=
                    crate::altair::helpers::get_base_reward(state, index, context)? * weight;
                */
            }
        }
    }

    // Reward proposer
    let proposer_reward_denominator = (crate::altair::WEIGHT_DENOMINATOR
        - crate::altair::PROPOSER_WEIGHT)
        * crate::altair::WEIGHT_DENOMINATOR
        / crate::altair::PROPOSER_WEIGHT;
    let proposer_reward = proposer_reward_numerator / proposer_reward_denominator;
    increase_balance(
        state,
        get_beacon_proposer_index(state, context)?,
        proposer_reward,
    );
    Ok(())
}

pub fn process_deposit<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
>(
    state: &mut BeaconState<
        SLOTS_PER_HISTORICAL_ROOT,
        HISTORICAL_ROOTS_LIMIT,
        ETH1_DATA_VOTES_BOUND,
        VALIDATOR_REGISTRY_LIMIT,
        EPOCHS_PER_HISTORICAL_VECTOR,
        EPOCHS_PER_SLASHINGS_VECTOR,
        MAX_VALIDATORS_PER_COMMITTEE,
        SYNC_COMMITTEE_SIZE,
    >,
    deposit: &mut Deposit,
    context: &Context,
) -> Result<()> {
    let branch = deposit
        .proof
        .iter()
        .map(|node| Node::from_bytes(node.as_ref().try_into().unwrap()))
        .collect::<Vec<_>>();
    let leaf = deposit.data.hash_tree_root()?;
    let depth = crate::altair::DEPOSIT_CONTRACT_TREE_DEPTH + 1;
    let index = state.eth1_deposit_index as usize;
    let root = &state.eth1_data.deposit_root;
    if !is_valid_merkle_branch(&leaf, branch.iter(), depth, index, root) {
        return Err(invalid_operation_error(InvalidOperation::Deposit(
            InvalidDeposit::InvalidProof {
                leaf,
                branch,
                depth,
                index,
                root: *root,
            },
        )));
    }

    // NOTE: deviate from the order of the spec to avoid mutations
    // that would need to be rolled back upon failure
    let public_key = deposit.data.public_key.clone();
    let amount = deposit.data.amount;
    let validator_public_keys: HashSet<&BlsPublicKey> =
        HashSet::from_iter(state.validators.iter().map(|v| &v.public_key));
    if !validator_public_keys.contains(&public_key) {
        let mut deposit_message = DepositMessage {
            public_key: public_key.clone(),
            withdrawal_credentials: deposit.data.withdrawal_credentials.clone(),
            amount,
        };
        let domain = compute_domain(DomainType::Deposit, None, None, context)?;
        let signing_root = compute_signing_root(&mut deposit_message, domain)?;
        // Initialize validator if the deposit signature is valid
        if public_key.verify_signature(signing_root.as_bytes(), &deposit.data.signature) {
            state
                .validators
                .push(get_validator_from_deposit(deposit, context));
            state.balances.push(amount);
            state
                .previous_epoch_participation
                .push(ParticipationFlags::default());
            state
                .current_epoch_participation
                .push(ParticipationFlags::default());
            state.inactivity_scores.push(u64::default())
        } else {
            return Err(invalid_operation_error(InvalidOperation::Deposit(
                InvalidDeposit::InvalidSignature(deposit.data.signature.clone()),
            )));
        }
    } else {
        let index = state
            .validators
            .iter()
            .position(|v| v.public_key == public_key)
            .unwrap();

        increase_balance(state, index, amount);
    }

    state.eth1_deposit_index += 1;
    Ok(())
}

pub fn process_sync_aggregate<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
>(
    state: &mut BeaconState<
        SLOTS_PER_HISTORICAL_ROOT,
        HISTORICAL_ROOTS_LIMIT,
        ETH1_DATA_VOTES_BOUND,
        VALIDATOR_REGISTRY_LIMIT,
        EPOCHS_PER_HISTORICAL_VECTOR,
        EPOCHS_PER_SLASHINGS_VECTOR,
        MAX_VALIDATORS_PER_COMMITTEE,
        SYNC_COMMITTEE_SIZE,
    >,
    sync_aggregate: &SyncAggregate<SYNC_COMMITTEE_SIZE>,
    context: &Context,
) -> Result<()> {
    // Verify sync committee aggregate signature signing over the previous slot block root
    let committee_public_keys = &state.current_sync_committee.public_keys;
    let participant_public_keys = zip(
        committee_public_keys.iter(),
        sync_aggregate.sync_committee_bits.iter(),
    )
    .filter_map(
        |(public_key, bit)| {
            if *bit {
                Some(public_key)
            } else {
                None
            }
        },
    )
    .collect::<Vec<_>>();
    let previous_slot = u64::max(state.slot, 1) - 1;
    let domain = get_domain(
        state,
        DomainType::SyncCommittee,
        Some(compute_epoch_at_slot(previous_slot, context)),
        context,
    )?;
    let mut root_at_slot = *get_block_root_at_slot(state, previous_slot)?;
    let signing_root = compute_signing_root(&mut root_at_slot, domain)?;
    if !fast_aggregate_verify(
        participant_public_keys.as_slice(),
        signing_root.as_ref(),
        &sync_aggregate.sync_committee_signature,
    ) {
        return Err(invalid_operation_error(InvalidOperation::SyncAggregate(
            InvalidSyncAggregate::InvalidSignature {
                signature: sync_aggregate.sync_committee_signature.clone(),
                root: signing_root,
            },
        )));
    }

    // Compute participant and proposer rewards
    let total_active_increments =
        get_total_active_balance(state, context)? / context.effective_balance_increment;
    let total_base_rewards =
        get_base_reward_per_increment(state, context)? * total_active_increments;
    let max_participant_rewards = total_base_rewards * crate::altair::SYNC_REWARD_WEIGHT
        / crate::altair::WEIGHT_DENOMINATOR
        / context.slots_per_epoch;
    let participant_reward = max_participant_rewards / context.sync_committee_size as u64;
    let proposer_reward = participant_reward * crate::altair::PROPOSER_WEIGHT
        / (crate::altair::WEIGHT_DENOMINATOR - crate::altair::PROPOSER_WEIGHT);

    // Apply participant and proposer rewards
    // @dev usage of clone here
    let mut all_public_keys = state.validators.iter().map(|v| v.public_key.clone());
    let mut committee_indices: Vec<ValidatorIndex> = Vec::default();
    for public_key in state.current_sync_committee.public_keys.iter() {
        committee_indices.push(
            all_public_keys
                .position(|pk| pk == *public_key)
                .expect("validator public_key should exist"),
        );
    }
    for (participant_index, participation_bit) in zip(
        committee_indices.iter(),
        sync_aggregate.sync_committee_bits.iter(),
    ) {
        if *participation_bit {
            increase_balance(state, *participant_index, participant_reward);
            increase_balance(
                state,
                get_beacon_proposer_index(state, context)?,
                proposer_reward,
            );
        } else {
            decrease_balance(state, *participant_index, participant_reward);
        }
    }

    Ok(())
}

pub fn process_block<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const MAX_PROPOSER_SLASHINGS: usize,
    const MAX_ATTESTER_SLASHINGS: usize,
    const MAX_ATTESTATIONS: usize,
    const MAX_DEPOSITS: usize,
    const MAX_VOLUNTARY_EXITS: usize,
    const SYNC_COMMITTEE_SIZE: usize,
>(
    state: &mut BeaconState<
        SLOTS_PER_HISTORICAL_ROOT,
        HISTORICAL_ROOTS_LIMIT,
        ETH1_DATA_VOTES_BOUND,
        VALIDATOR_REGISTRY_LIMIT,
        EPOCHS_PER_HISTORICAL_VECTOR,
        EPOCHS_PER_SLASHINGS_VECTOR,
        MAX_VALIDATORS_PER_COMMITTEE,
        SYNC_COMMITTEE_SIZE,
    >,
    block: &mut BeaconBlock<
        MAX_PROPOSER_SLASHINGS,
        MAX_VALIDATORS_PER_COMMITTEE,
        MAX_ATTESTER_SLASHINGS,
        MAX_ATTESTATIONS,
        MAX_DEPOSITS,
        MAX_VOLUNTARY_EXITS,
        SYNC_COMMITTEE_SIZE,
    >,
    context: &Context,
) -> Result<()> {
    process_block_header(state, block, context)?;
    process_randao(state, &block.body, context)?;
    process_eth1_data(state, &block.body, context);
    process_operations(state, &mut block.body, context)?;
    process_sync_aggregate(state, &block.body.sync_aggregate, context)?;
    Ok(())
}
