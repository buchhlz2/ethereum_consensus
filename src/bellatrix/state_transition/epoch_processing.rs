//! WARNING: This file was derived by the `gen-spec` utility. DO NOT EDIT MANUALLY.
use crate::bellatrix as spec;
use crate::primitives::{Epoch, Gwei, ValidatorIndex, GENESIS_EPOCH};
use crate::state_transition::{Context, Error, Result};
use integer_sqrt::IntegerSquareRoot;
use spec::{
    compute_activation_exit_epoch, decrease_balance, get_attesting_indices,
    get_block_root, get_block_root_at_slot, get_current_epoch,
    get_eligible_validator_indices, get_previous_epoch, get_randao_mix,
    get_total_active_balance, get_total_balance, get_validator_churn_limit,
    increase_balance, initiate_validator_exit, is_active_validator,
    is_eligible_for_activation, is_eligible_for_activation_queue, BeaconState,
    Checkpoint, HistoricalBatchAccumulator, BASE_REWARDS_PER_EPOCH,
    JUSTIFICATION_BITS_LENGTH, ParticipationFlags, SyncAggregate,
};
use ssz_rs::prelude::*;
use std::collections::HashSet;
use std::mem;
pub use crate::bellatrix::epoch_processing::process_epoch;
pub fn process_registry_updates<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) {
    let current_epoch = get_current_epoch(state, context);
    for i in 0..state.validators.len() {
        let validator = &mut state.validators[i];
        if is_eligible_for_activation_queue(validator, context) {
            validator.activation_eligibility_epoch = current_epoch + 1;
        }
        if is_active_validator(validator, current_epoch)
            && validator.effective_balance <= context.ejection_balance
        {
            initiate_validator_exit(state, i, context);
        }
    }
    let mut activation_queue = state
        .validators
        .iter()
        .enumerate()
        .filter_map(|(index, validator)| {
            if is_eligible_for_activation(state, validator) { Some(index) } else { None }
        })
        .collect::<Vec<ValidatorIndex>>();
    activation_queue
        .sort_by(|&i, &j| {
            let a = &state.validators[i];
            let b = &state.validators[j];
            (a.activation_eligibility_epoch, i).cmp(&(b.activation_eligibility_epoch, j))
        });
    let activation_exit_epoch = compute_activation_exit_epoch(current_epoch, context);
    for i in activation_queue.into_iter().take(get_validator_churn_limit(state, context))
    {
        let validator = &mut state.validators[i];
        validator.activation_epoch = activation_exit_epoch;
    }
}
pub fn process_slashings<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) -> Result<()> {
    let epoch = get_current_epoch(state, context);
    let total_balance = get_total_active_balance(state, context)?;
    let proportional_slashing_multiplier = context
        .proportional_slashing_multiplier(epoch)?;
    let adjusted_total_slashing_balance = Gwei::min(
        state.slashings.iter().sum::<Gwei>() * proportional_slashing_multiplier,
        total_balance,
    );
    for i in 0..state.validators.len() {
        let validator = &state.validators[i];
        if validator.slashed
            && (epoch + context.epochs_per_slashings_vector / 2)
                == validator.withdrawable_epoch
        {
            let increment = context.effective_balance_increment;
            let penalty_numerator = validator.effective_balance / increment
                * adjusted_total_slashing_balance;
            let penalty = penalty_numerator / total_balance * increment;
            decrease_balance(state, i, penalty);
        }
    }
    Ok(())
}
pub fn process_eth1_data_reset<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) {
    let next_epoch = get_current_epoch(state, context) + 1;
    if next_epoch % context.epochs_per_eth1_voting_period == 0 {
        state.eth1_data_votes.clear();
    }
}
pub fn process_effective_balance_updates<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) {
    let hysteresis_increment = context.effective_balance_increment
        / context.hysteresis_quotient;
    let downward_threshold = hysteresis_increment
        * context.hysteresis_downward_multiplier;
    let upward_threshold = hysteresis_increment * context.hysteresis_upward_multiplier;
    for i in 0..state.validators.len() {
        let validator = &mut state.validators[i];
        let balance = state.balances[i];
        if balance + downward_threshold < validator.effective_balance
            || validator.effective_balance + upward_threshold < balance
        {
            validator
                .effective_balance = Gwei::min(
                balance - balance % context.effective_balance_increment,
                context.max_effective_balance,
            );
        }
    }
}
pub fn process_slashings_reset<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) {
    let next_epoch = get_current_epoch(state, context) + 1;
    let slashings_index = next_epoch % context.epochs_per_slashings_vector;
    state.slashings[slashings_index as usize] = 0;
}
pub fn process_randao_mixes_reset<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) {
    let current_epoch = get_current_epoch(state, context);
    let next_epoch = current_epoch + 1;
    let mix_index = next_epoch % context.epochs_per_historical_vector;
    state
        .randao_mixes[mix_index as usize] = get_randao_mix(state, current_epoch).clone();
}
pub fn process_historical_roots_update<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) -> Result<()> {
    let next_epoch = get_current_epoch(state, context) + 1;
    let epochs_per_historical_root = context.slots_per_historical_root
        / context.slots_per_epoch;
    if next_epoch % epochs_per_historical_root == 0 {
        let mut historical_batch = HistoricalBatchAccumulator {
            block_roots_root: state.block_roots.hash_tree_root()?,
            state_roots_root: state.state_roots.hash_tree_root()?,
        };
        state.historical_roots.push(historical_batch.hash_tree_root()?)
    }
    Ok(())
}
pub fn weigh_justification_and_finalization<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    total_active_balance: Gwei,
    previous_epoch_target_balance: Gwei,
    current_epoch_target_balance: Gwei,
    context: &Context,
) -> Result<()> {
    let previous_epoch = get_current_epoch(state, context);
    let current_epoch = get_current_epoch(state, context);
    let old_previous_justified_checkpoint = state.previous_justified_checkpoint.clone();
    let old_current_justified_checkpoint = state.current_justified_checkpoint.clone();
    state.previous_justified_checkpoint = state.current_justified_checkpoint.clone();
    state.justification_bits.copy_within(..JUSTIFICATION_BITS_LENGTH - 1, 1);
    state.justification_bits.set(0, false);
    if previous_epoch_target_balance * 3 >= total_active_balance * 2 {
        state
            .current_justified_checkpoint = Checkpoint {
            epoch: previous_epoch,
            root: *get_block_root(state, previous_epoch, context)?,
        };
        state.justification_bits.set(1, true);
    }
    if current_epoch_target_balance * 3 >= total_active_balance * 2 {
        state
            .current_justified_checkpoint = Checkpoint {
            epoch: current_epoch,
            root: *get_block_root(state, current_epoch, context)?,
        };
        state.justification_bits.set(0, true);
    }
    let bits = &state.justification_bits;
    if bits[1..4].all() && old_previous_justified_checkpoint.epoch + 3 == current_epoch {
        state.finalized_checkpoint = old_previous_justified_checkpoint.clone();
    }
    if bits[1..3].all() && old_previous_justified_checkpoint.epoch + 2 == current_epoch {
        state.finalized_checkpoint = old_previous_justified_checkpoint;
    }
    if bits[0..3].all() && old_current_justified_checkpoint.epoch + 2 == current_epoch {
        state.finalized_checkpoint = old_current_justified_checkpoint.clone();
    }
    if bits[0..2].all() && old_current_justified_checkpoint.epoch + 1 == current_epoch {
        state.finalized_checkpoint = old_current_justified_checkpoint;
    }
    Ok(())
}
pub fn get_base_reward<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
>(
    state: &BeaconState<
        SLOTS_PER_HISTORICAL_ROOT,
        HISTORICAL_ROOTS_LIMIT,
        ETH1_DATA_VOTES_BOUND,
        VALIDATOR_REGISTRY_LIMIT,
        EPOCHS_PER_HISTORICAL_VECTOR,
        EPOCHS_PER_SLASHINGS_VECTOR,
        MAX_VALIDATORS_PER_COMMITTEE,
        SYNC_COMMITTEE_SIZE,
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    index: ValidatorIndex,
    context: &Context,
) -> Result<Gwei> {
    let total_balance = get_total_active_balance(state, context)?;
    let effective_balance = state.validators[index].effective_balance;
    Ok(
        effective_balance * context.base_reward_factor / total_balance.integer_sqrt()
            / BASE_REWARDS_PER_EPOCH,
    )
}
pub fn get_proposer_reward<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
>(
    state: &BeaconState<
        SLOTS_PER_HISTORICAL_ROOT,
        HISTORICAL_ROOTS_LIMIT,
        ETH1_DATA_VOTES_BOUND,
        VALIDATOR_REGISTRY_LIMIT,
        EPOCHS_PER_HISTORICAL_VECTOR,
        EPOCHS_PER_SLASHINGS_VECTOR,
        MAX_VALIDATORS_PER_COMMITTEE,
        SYNC_COMMITTEE_SIZE,
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    attesting_index: ValidatorIndex,
    context: &Context,
) -> Result<Gwei> {
    Ok(
        get_base_reward(state, attesting_index, context)?
            / context.proposer_reward_quotient,
    )
}
pub fn get_finality_delay<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
>(
    state: &BeaconState<
        SLOTS_PER_HISTORICAL_ROOT,
        HISTORICAL_ROOTS_LIMIT,
        ETH1_DATA_VOTES_BOUND,
        VALIDATOR_REGISTRY_LIMIT,
        EPOCHS_PER_HISTORICAL_VECTOR,
        EPOCHS_PER_SLASHINGS_VECTOR,
        MAX_VALIDATORS_PER_COMMITTEE,
        SYNC_COMMITTEE_SIZE,
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) -> Epoch {
    get_previous_epoch(state, context) - state.finalized_checkpoint.epoch
}
pub fn is_in_inactivity_leak<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
>(
    state: &BeaconState<
        SLOTS_PER_HISTORICAL_ROOT,
        HISTORICAL_ROOTS_LIMIT,
        ETH1_DATA_VOTES_BOUND,
        VALIDATOR_REGISTRY_LIMIT,
        EPOCHS_PER_HISTORICAL_VECTOR,
        EPOCHS_PER_SLASHINGS_VECTOR,
        MAX_VALIDATORS_PER_COMMITTEE,
        SYNC_COMMITTEE_SIZE,
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) -> bool {
    get_finality_delay(state, context) > context.min_epochs_to_inactivity_penalty
}
pub fn process_justification_and_finalization<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) -> Result<()> {
    let current_epoch = get_current_epoch(state, context);
    if current_epoch <= GENESIS_EPOCH + 1 {
        return Ok(());
    }
    let previous_indices = get_unslashed_participating_indices(
        state,
        TIMELY_TARGET_FLAG_INDEX,
        get_previous_epoch(state, context),
        context,
    )?;
    let current_indices = get_unslashed_participating_indices(
        state,
        TIMELY_TARGET_FLAG_INDEX,
        get_previous_epoch(state, context),
        context,
    )?;
    let total_active_balance = get_total_active_balance(state, context)?;
    let previous_target_balance = get_total_balance(state, &previous_indices, context)?;
    let current_target_balance = get_total_balance(state, &current_indices, context)?;
    weigh_justification_and_finalization(
        state,
        total_active_balance,
        previous_target_balance,
        current_target_balance,
        context,
    )
}
pub fn process_inactivity_updates<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) -> Result<()> {
    let current_epoch = get_current_epoch(state, context);
    if current_epoch == GENESIS_EPOCH {
        return Ok(());
    }
    let eligible_validator_indices = get_eligible_validator_indices(state, context)
        .collect::<Vec<_>>();
    for index in eligible_validator_indices {
        if get_unslashed_participating_indices(
                state,
                TIMELY_TARGET_FLAG_INDEX,
                get_previous_epoch(state, context),
                context,
            )?
            .contains(&index)
        {
            state.inactivity_scores[index]
                -= u64::min(1, state.inactivity_scores[index]);
        } else {
            state.inactivity_scores[index] += context.inactivity_score_bias;
        }
        if !is_in_inactivity_leak(state, context) {
            state.inactivity_scores[index]
                -= u64::min(1, state.inactivity_scores[index]);
        }
    }
    Ok(())
}
pub fn process_rewards_and_penalties<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) -> Result<()> {
    let current_epoch = get_current_epoch(state, context);
    if current_epoch == GENESIS_EPOCH {
        return Ok(());
    }
    let mut deltas = Vec::new();
    for flag_index in 0..PARTICIPATION_FLAG_WEIGHTS.len() {
        let flag_index_delta = get_flag_index_deltas(state, flag_index, context)?;
        deltas.push(flag_index_delta);
    }
    deltas.push(get_inactivity_penalty_deltas(state, context)?);
    for (rewards, penalties) in deltas.iter() {
        for index in 0..state.validators.len() {
            increase_balance(state, index, rewards[index]);
            decrease_balance(state, index, penalties[index]);
        }
    }
    Ok(())
}
pub fn process_participation_flag_updates<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
) -> Result<()> {
    let current_participation = mem::take(&mut state.current_epoch_participation);
    state.previous_epoch_participation = current_participation;
    let rotate_participation = vec![
        ParticipationFlags::default(); state.validators.len()
    ];
    state
        .current_epoch_participation = rotate_participation
        .try_into()
        .expect("should convert from Vec to List");
    Ok(())
}
pub fn process_sync_committee_updates<
    const SLOTS_PER_HISTORICAL_ROOT: usize,
    const HISTORICAL_ROOTS_LIMIT: usize,
    const ETH1_DATA_VOTES_BOUND: usize,
    const VALIDATOR_REGISTRY_LIMIT: usize,
    const EPOCHS_PER_HISTORICAL_VECTOR: usize,
    const EPOCHS_PER_SLASHINGS_VECTOR: usize,
    const MAX_VALIDATORS_PER_COMMITTEE: usize,
    const SYNC_COMMITTEE_SIZE: usize,
    const BYTES_PER_LOGS_BLOOM: usize,
    const MAX_EXTRA_DATA_BYTES: usize,
    const MAX_BYTES_PER_TRANSACTION: usize,
    const MAX_TRANSACTIONS_PER_PAYLOAD: usize,
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
        BYTES_PER_LOGS_BLOOM,
        MAX_EXTRA_DATA_BYTES,
        MAX_BYTES_PER_TRANSACTION,
        MAX_TRANSACTIONS_PER_PAYLOAD,
    >,
    context: &Context,
) -> Result<()> {
    let next_sync_committee = mem::take(&mut state.next_sync_committee);
    let next_epoch = get_current_epoch(state, context) + 1;
    if next_epoch % context.epochs_per_sync_committee_period == 0 {
        state.current_sync_committee = next_sync_committee;
        state.next_sync_committee = get_next_sync_committee(state, context)?;
    }
    Ok(())
}
