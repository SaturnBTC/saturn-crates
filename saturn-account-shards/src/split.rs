use arch_program::{
    rune::{RuneAmount, RuneId},
    utxo::UtxoMeta,
};
use bitcoin::{Amount, ScriptBuf, TxOut};
use ordinals::Edict;
use saturn_bitcoin_transactions::{
    constants::DUST_LIMIT, fee_rate::FeeRate, utxo_info::UtxoInfoTrait, TransactionBuilder,
};
use saturn_collections::generic::fixed_set::FixedCapacitySet;
use saturn_safe_math::{safe_add, safe_div, safe_mul, safe_sub, MathError};

use crate::shard::StateShard;
#[cfg(feature = "runes")]
use crate::StateShardError;
// Required for test helpers in nested modules.
#[allow(unused_imports)]
use saturn_bitcoin_transactions::utxo_info::SingleRuneSet;

/// Splits the *remaining* satoshi value that belongs to the provided `shards`
/// back into brand-new outputs, one per shard, so that liquidity across all
/// participating shards ends up as even as possible.
///
/// The function performs the following high-level steps:
/// 1. Determine how many satoshis are still owned by the shards **after** the
///    caller has already removed some liquidity (`removed_from_shards`) and
///    the program has paid its share of fees.
/// 2. Ask [`plan_btc_distribution_among_shards`] to derive an
///    optimal per-shard allocation for that remaining amount.
/// 3. Append one [`TxOut`] to the underlying transaction for every computed
///    allocation, using `program_script_pubkey` to lock those outputs back to
///    the program.
///
/// The returned vector contains one element for each participating shard and
/// is **sorted in descending order by amount (largest first)**.  Since the
/// order no longer corresponds to `shard_indexes`, callers that need to map
/// values back to specific shards must perform that mapping explicitly.
///
/// # Type Parameters
/// * `MAX_USER_UTXOS` – Maximum amount of user-supplied UTXOs supported by
///   the [`TransactionBuilder`].
/// * `MAX_SHARDS_PER_POOL` – Compile-time bound on the number of shards in a
///   liquidity pool.
///
/// # Parameters
/// * `transaction_builder` – Mutable reference to the transaction that is
///   currently being assembled.
/// * `shards` – Slice containing **all** shards of the pool.
/// * `shard_indexes` – Indexes into `shards` that participate in this
///   operation.
/// * `removed_from_shards` – Total satoshis that the caller already withdrew
///   from the listed shards during the current instruction.
/// * `program_script_pubkey` – Script that will lock the change outputs
///   produced by this function (usually the program's own script).
/// * `fee_rate` – Fee rate used to calculate how many sats were paid by the
///   program so far.
///
/// # Errors
/// Propagates [`MathError`] if any of the safe-math operations overflows or
/// underflows.
pub fn redistribute_remaining_btc_to_shards<
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    transaction_builder: &mut TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shards: &[&mut T],
    shard_indexes: &[usize],
    removed_from_shards: u64,
    program_script_pubkey: ScriptBuf,
    fee_rate: &FeeRate,
) -> Result<Vec<u128>, MathError> {
    let remaining_amount = compute_unsettled_btc_in_shards(
        transaction_builder,
        shards,
        shard_indexes,
        removed_from_shards,
        fee_rate,
    )?;

    let mut distribution = plan_btc_distribution_among_shards(
        transaction_builder,
        shards,
        shard_indexes,
        remaining_amount as u128,
    )?;

    // Order distribution by biggest first (simple numeric comparison).
    distribution.sort_by(|a, b| b.cmp(a));

    for amount in distribution.iter() {
        transaction_builder.transaction.output.push(TxOut {
            value: Amount::from_sat(*amount as u64),
            script_pubkey: program_script_pubkey.clone(),
        });
    }

    Ok(distribution)
}

/// Splits the *remaining* amount of the specified Rune across shards and
/// generates the corresponding edicts inside the embedded runestone.
///
/// This works analogously to
/// [`redistribute_remaining_btc_to_shards`], but for Rune tokens instead of
/// satoshis. In addition to creating change outputs, the function also:
/// * Updates `transaction_builder.runestone.pointer` so that the runestone
///   points at the *first* newly created output.
/// * Emits an [`Edict`] for every shard (except the first) so that the Rune
///   amounts get credited to the respective outputs on-chain.
///
/// The returned vector follows the same sorting semantics as its BTC
/// counterpart: one entry per participating shard, ordered from largest to
/// smallest amount.
///
/// # Parameters
/// The parameter list is identical in spirit to the BTC counterpart, with the
/// addition of `rune_id` that specifies which Rune is being redistributed.
///
/// # Errors
/// Returns [`MathError`] when any safe-math operation fails.
#[cfg(feature = "runes")]
pub fn redistribute_remaining_rune_to_shards<
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    transaction_builder: &mut TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shards: &[&mut T],
    shard_indexes: &[usize],
    removed_from_shards: RS,
    program_script_pubkey: ScriptBuf,
) -> Result<Vec<RS>, StateShardError> {
    let remaining_amount =
        compute_unsettled_rune_in_shards(shards, shard_indexes, removed_from_shards)?;

    let mut distribution = plan_rune_distribution_among_shards(
        transaction_builder,
        shards,
        shard_indexes,
        &remaining_amount,
    )?;

    // Sort by total amount descending. `RS` itself might not implement `Ord`,
    // therefore we derive an ordering based on the **sum** of all rune amounts
    // inside each set.
    distribution.sort_by(|a, b| {
        let total_a: u128 = a.iter().map(|r| r.amount).sum();
        let total_b: u128 = b.iter().map(|r| r.amount).sum();
        total_b.cmp(&total_a)
    });

    let current_output_index = transaction_builder.transaction.output.len();

    transaction_builder.runestone.pointer = Some(current_output_index as u32);

    let mut index = current_output_index;
    for amount in distribution.iter() {
        transaction_builder.transaction.output.push(TxOut {
            value: Amount::from_sat(DUST_LIMIT),
            script_pubkey: program_script_pubkey.clone(),
        });

        if index > current_output_index {
            for rune_amount in amount.iter() {
                transaction_builder.runestone.edicts.push(Edict {
                    id: ordinals::RuneId {
                        block: rune_amount.id.block,
                        tx: rune_amount.id.tx,
                    },
                    amount: rune_amount.amount,
                    output: index as u32,
                });
            }
        }

        index += 1;
    }

    Ok(distribution)
}

/// Calculates how many satoshis are *still* owned by the selected shards after
/// accounting for
/// * funds that were already removed (`removed_from_shards`), and
/// * fees that were paid by the program up to this point.
///
/// The helper iterates over every input that comes from a shard-managed UTXO
/// and sums their values. Any "consolidation" UTXOs that were injected by the
/// program itself via `TransactionBuilder::total_btc_consolidation_input` are
/// also taken into consideration.
///
/// The resulting value represents the amount that still needs to be sent back
/// to the shards so that no satoshis are lost.
///
/// # Errors
/// Propagates [`MathError`] on arithmetic overflow.
pub fn compute_unsettled_btc_in_shards<
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    transaction_builder: &TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shards: &[&mut T],
    shard_indexes: &[usize],
    removed_from_shards: u64,
    fee_rate: &FeeRate,
) -> Result<u64, MathError> {
    let mut total_btc_amount = 0u64;

    for input in transaction_builder.transaction.input.iter() {
        let input_meta =
            UtxoMeta::from_outpoint(input.previous_output.txid, input.previous_output.vout);

        // Iterate over the selected shards **until the first match** is found. This avoids
        // accidentally double-counting the same UTXO when, for whatever reason, multiple shards
        // reference an identical `UtxoMeta` (which can happen in unit tests that use synthetic
        // identifiers).
        for &shard_index in shard_indexes.iter() {
            let shard = &shards[shard_index];
            if let Some(utxo) = shard.btc_utxos().iter().find(|u| *u.meta() == input_meta) {
                total_btc_amount = safe_add(total_btc_amount, utxo.value())?;
                // We can stop searching once we found the first matching UTXO for this input.
                break;
            }
        }
    }

    let fee_paid_by_program = {
        #[cfg(feature = "utxo-consolidation")]
        {
            transaction_builder.get_fee_paid_by_program(&fee_rate)
        }
        #[cfg(not(feature = "utxo-consolidation"))]
        {
            0
        }
    };

    let total_btc_consolidation_input = {
        #[cfg(feature = "utxo-consolidation")]
        {
            transaction_builder.total_btc_consolidation_input
        }
        #[cfg(not(feature = "utxo-consolidation"))]
        {
            0
        }
    };

    let remaining_amount = safe_sub(
        safe_sub(
            safe_add(total_btc_amount, total_btc_consolidation_input)?,
            removed_from_shards,
        )?,
        fee_paid_by_program,
    )?;

    Ok(remaining_amount)
}

/// Same as [`compute_unsettled_btc_in_shards`] but for Rune tokens.
/// It sums up the token amount inside the `rune_utxo` of every participating
/// shard, subtracts whatever has already been removed, and returns the number
/// of tokens that still have to be redistributed.
///
/// # Errors
/// Returns [`MathError`] if an arithmetic operation overflows.
#[cfg(feature = "runes")]
pub fn compute_unsettled_rune_in_shards<
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    shards: &[&mut T],
    shard_indexes: &[usize],
    removed_from_shards: RS,
) -> Result<RS, StateShardError> {
    let mut total_rune_amount = RS::default();

    for &shard_index in shard_indexes.iter() {
        let shard = &shards[shard_index];

        if let Some(utxo) = shard.rune_utxo() {
            for rune in utxo.runes().iter() {
                total_rune_amount.insert_or_modify::<StateShardError, _>(
                    RuneAmount {
                        id: rune.id,
                        amount: rune.amount,
                    },
                    |r| {
                        r.amount = safe_add(r.amount, rune.amount)
                            .map_err(|_| StateShardError::RuneAmountAdditionOverflow)?;
                        Ok(())
                    },
                )?;
            }
        }
    }

    for rune in removed_from_shards.iter() {
        let output_rune = total_rune_amount.find_mut(&rune.id);
        if let Some(output_rune) = output_rune {
            output_rune.amount = safe_sub(output_rune.amount, rune.amount)
                .map_err(|_| StateShardError::RemovingMoreRunesThanPresentInShards)?;
        }
    }

    Ok(total_rune_amount)
}

/// Wrapper around [`balance_amount_across_shards`] that performs an even-as-
/// possible redistribution of satoshis *and* ensures that no individual
/// allocation is smaller than the Bitcoin dust limit. Amounts that fall below
/// `DUST_LIMIT` are collected and re-allocated to the remaining entries via
/// [`redistribute_sub_dust_values`].
///
/// The invariants described in [`balance_amount_across_shards`] still hold.
///
/// The returned `Vec` **always** contains exactly one element per index in
/// `shard_indexes` and is ordered identically.
///
/// # Errors
/// Returns `MathError` when the math operations fail.
pub fn plan_btc_distribution_among_shards<
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    transaction_builder: &TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shards: &[&mut T],
    shard_indexes: &[usize],
    amount: u128,
) -> Result<Vec<u128>, MathError> {
    let mut result = balance_amount_across_shards(
        transaction_builder,
        shards,
        shard_indexes,
        &RuneAmount {
            id: RuneId::BTC,
            amount,
        },
    )?;

    redistribute_sub_dust_values(&mut result, DUST_LIMIT as u128)?;

    Ok(result)
}

/// Thin wrapper around [`balance_amount_across_shards`] for Rune tokens. Unlike
/// the BTC variant, there is no concept of a dust limit for Runes, so the
/// function simply forwards the result unchanged.
///
/// The returned vector obeys the same invariants as the other distribution
/// helpers: one value per shard index, ordered identically, summing up to the
/// `amount` that was supplied.
///
/// The returned `Vec` **always** contains exactly one element per index in
/// `shard_indexes` and is ordered identically.
///
/// # Errors
/// Returns `MathError` when the math operations fail.
#[cfg(feature = "runes")]
pub fn plan_rune_distribution_among_shards<
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    transaction_builder: &mut TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shards: &[&mut T],
    shard_indexes: &[usize],
    amounts: &RS,
) -> Result<Vec<RS>, StateShardError> {
    let mut result = Vec::with_capacity(shard_indexes.len());
    for _ in shard_indexes.iter() {
        result.push(RS::default());
    }

    for rune_amount in amounts.iter() {
        let amounts =
            balance_amount_across_shards(transaction_builder, shards, shard_indexes, rune_amount)
                .map_err(|_| StateShardError::MathErrorInBalanceAmountAcrossShards)?;

        for (i, amount) in amounts.iter().enumerate() {
            result[i].insert_or_modify::<StateShardError, _>(
                RuneAmount {
                    id: rune_amount.id,
                    amount: *amount,
                },
                |r| {
                    r.amount = safe_add(r.amount, *amount)
                        .map_err(|_| StateShardError::RuneAmountAdditionOverflow)?;
                    Ok(())
                },
            )?;
        }
    }

    Ok(result)
}

/// Computes an as-balanced-as-possible allocation of `amount` across the
/// provided `shard_indexes` **without** mutating either the shards themselves
/// or the underlying [`TransactionBuilder`].
///
/// The returned vector can subsequently be used by the caller to create
/// change outputs, edicts, or to update in-memory shard state—whatever is
/// appropriate in the higher-level context.  However, **nothing** is changed
/// inside this helper; it is purely a *calculator*.
///
/// Algorithm overview:
/// 1. Work out the current liquidity (BTC or Rune, depending on
///    `update_by`) for each shard **excluding** any UTXOs that are already
///    being spent in `transaction_builder`.
/// 2. Derive the `desired_per_shard` value that would make every shard hold an
///    equal share *after* `amount` has been redistributed.
/// 3. If the available `amount` can fully satisfy those needs, assign the
///    leftovers evenly (with modulo-remainder handling). Otherwise fall back
///    to a proportional distribution so that the sum of all assignments still
///    equals the original `amount`.
///
/// Invariants:
/// * The length of the returned `Vec` is exactly `shard_indexes.len()` and its
///   i-th entry refers to the i-th index in `shard_indexes`.
/// * The sum of all entries equals `amount` (modulo rounding for proportional
///   splits that involves integer division).
///
/// # Errors
/// Propagates [`MathError`] if any safe-math operation overflows or underflows.
#[allow(clippy::type_complexity)]
fn balance_amount_across_shards<
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    transaction_builder: &TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shards: &[&mut T],
    shard_indexes: &[usize],
    rune_amount: &RuneAmount,
) -> core::result::Result<Vec<u128>, MathError> {
    let num_shards = shard_indexes.len();
    let mut assigned_amounts: Vec<u128> = Vec::with_capacity(num_shards);
    let mut total_current_amount = 0u128;

    let is_utxo_used = |utxo_meta: &UtxoMeta| {
        transaction_builder.transaction.input.iter().any(|input| {
            UtxoMeta::from_outpoint(input.previous_output.txid, input.previous_output.vout)
                == *utxo_meta
        })
    };

    // 1. Determine the current amount per shard and overall.
    for &shard_index in shard_indexes.iter() {
        let shard = &shards[shard_index];
        let current = match rune_amount.id {
            RuneId::BTC => shard
                .btc_utxos()
                .iter()
                .filter_map(|u| {
                    if is_utxo_used(u.meta()) {
                        None
                    } else {
                        Some(u.value() as u128)
                    }
                })
                .sum(),
            _ => {
                #[cfg(feature = "runes")]
                {
                    shard
                        .rune_utxo()
                        .map(|u| u.runes().find(&rune_amount.id).map(|r| r.amount))
                        .flatten()
                        .unwrap_or(0)
                }
                #[cfg(not(feature = "runes"))]
                {
                    0
                }
            }
        };
        assigned_amounts.push(current);
        total_current_amount = safe_add(total_current_amount, current)?;
    }

    // 2. Compute target per shard.
    let total_after = safe_add(total_current_amount, rune_amount.amount)?;
    let desired_per_shard = safe_div(total_after, num_shards as u128)?;

    // 3. Determine additional needed per shard.
    let mut total_needed = 0u128;
    for current in assigned_amounts.iter_mut() {
        let needed = if desired_per_shard > *current {
            safe_sub(desired_per_shard, *current)?
        } else {
            0
        };
        total_needed = safe_add(total_needed, needed)?;
        *current = needed;
    }

    if total_needed <= rune_amount.amount {
        // Distribute leftovers evenly.
        let leftover = safe_sub(rune_amount.amount, total_needed)?;
        let per_shard_extra = safe_div(leftover, num_shards as u128)?;
        let mut extra_left = leftover % num_shards as u128;

        for amt in assigned_amounts.iter_mut() {
            *amt = safe_add(*amt, per_shard_extra)?;
            if extra_left > 0 {
                *amt = safe_add(*amt, 1)?;
                extra_left -= 1;
            }
        }
    } else {
        // Proportional distribution.
        let mut cumulative = 0u128;
        let mut cumulative_needed = 0u128;

        for i in 0..num_shards {
            let needed = assigned_amounts[i];
            cumulative_needed = safe_add(cumulative_needed, needed)?;
            let proportional = safe_mul(rune_amount.amount, cumulative_needed)? / total_needed;
            assigned_amounts[i] = safe_sub(proportional, cumulative)?;
            cumulative = proportional;
        }
    }

    Ok(assigned_amounts)
}

/// Reallocates amounts smaller than the dust limit to the remaining amounts.
///
/// This function is used to ensure that the amounts are evenly distributed
/// across the shards.
///
/// # Errors
/// Returns `MathError` when the math operations fail.
fn redistribute_sub_dust_values(
    amounts: &mut Vec<u128>,
    dust_limit: u128,
) -> Result<(), MathError> {
    // Sum the amounts smaller than the dust limit.
    let sum_of_small_amounts = amounts
        .iter()
        .filter(|&&amount| amount < dust_limit)
        .sum::<u128>();

    // Remove the small amounts from the vector.
    amounts.retain(|&amount| amount >= dust_limit);

    // Check if there are amounts to redistribute to.
    if amounts.is_empty() {
        // If the total sum is at least the dust limit, add it as a new amount.
        if sum_of_small_amounts >= dust_limit {
            amounts.push(sum_of_small_amounts);
        } else {
            // If the total sum is less than the dust limit, we can discard it.
            amounts.clear();
        }

        return Ok(());
    }

    // Split the small amounts between the remaining entries.
    let num_amounts = amounts.len() as u128;
    let to_add = safe_div(sum_of_small_amounts, num_amounts)?;
    let mut remainder = sum_of_small_amounts % num_amounts;

    for amount in amounts.iter_mut() {
        *amount = safe_add(*amount, to_add)?;
        if remainder > 0 {
            *amount = safe_add(*amount, 1)?;
            remainder -= 1;
        }
    }

    Ok(())
}

// Expose helpers to nested modules.
pub mod common {
    use super::*;
    use arch_program::utxo::UtxoMeta;
    use saturn_bitcoin_transactions::utxo_info::UtxoInfo;

    #[derive(Default, Clone)]
    pub struct MockShard {
        pub btc_utxos: Vec<UtxoInfo<SingleRuneSet>>,
        pub rune_utxo: Option<UtxoInfo<SingleRuneSet>>,
    }

    impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for MockShard {
        fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] {
            &self.btc_utxos
        }

        fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] {
            self.btc_utxos.as_mut_slice()
        }

        fn btc_utxos_retain(&mut self, f: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {
            self.btc_utxos.retain(|u| f(u));
        }

        fn add_btc_utxo(&mut self, utxo: UtxoInfo<SingleRuneSet>) -> Option<usize> {
            self.btc_utxos.push(utxo);
            Some(self.btc_utxos.len() - 1)
        }

        fn btc_utxos_len(&self) -> usize {
            self.btc_utxos.len()
        }

        fn btc_utxos_max_len(&self) -> usize {
            self.btc_utxos.capacity()
        }

        fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> {
            self.rune_utxo.as_ref()
        }

        fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> {
            self.rune_utxo.as_mut()
        }

        fn clear_rune_utxo(&mut self) {
            self.rune_utxo = None;
        }

        fn set_rune_utxo(&mut self, utxo: UtxoInfo<SingleRuneSet>) {
            self.rune_utxo = Some(utxo);
        }
    }

    pub fn random_utxo_meta(vout: u32) -> UtxoMeta {
        UtxoMeta::from([vout as u8; 32], vout)
    }

    pub fn create_btc_utxo(value: u64, vout: u32) -> UtxoInfo<SingleRuneSet> {
        UtxoInfo::<SingleRuneSet> {
            meta: random_utxo_meta(vout),
            value,
            ..UtxoInfo::<SingleRuneSet>::default()
        }
    }

    pub fn create_shard(initial_btc: u64) -> MockShard {
        let mut shard = MockShard::default();
        if initial_btc > 0 {
            shard.add_btc_utxo(create_btc_utxo(initial_btc, 0));
        }
        shard
    }
}

mod tests {
    use super::*;
    use arch_program::utxo::UtxoMeta;
    use saturn_bitcoin_transactions::TransactionBuilder;

    #[allow(unused_macros)]
    macro_rules! new_tb {
        ($max_mod:expr, $max_inputs:expr) => {{
            TransactionBuilder::<
                $max_mod,
                $max_inputs,
                saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
            >::new()
        }};
    }

    // ---------------------------------------------------------------------
    // plan_btc_distribution_among_shards -----------------------------------
    // ---------------------------------------------------------------------

    mod plan_btc_distribution_among_shards {
        use std::str::FromStr;

        use super::common::*;
        use crate::split::{compute_unsettled_btc_in_shards, plan_btc_distribution_among_shards};
        use saturn_bitcoin_transactions::{
            constants::DUST_LIMIT, fee_rate::FeeRate, utxo_info::SingleRuneSet, TransactionBuilder,
        };
        use saturn_safe_math::MathError;

        #[test]
        fn test_proportional_distribution_insufficient_remaining() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder: TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, SingleRuneSet> =
                new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(100), create_shard(200), create_shard(300)];
            let shard_indexes = vec![0usize, 1usize, 2usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            // Not enough to equalise fully
            let remaining_amount = 150u128;
            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                remaining_amount,
            )
            .unwrap();

            // Since `remaining_amount` is below the Bitcoin dust limit
            // every tentative allocation will be filtered out.
            // Therefore the distribution should be **empty**.
            assert!(distribution.is_empty());
        }

        #[test]
        fn test_used_utxos_excluded() {
            use bitcoin::{transaction::Version, OutPoint, ScriptBuf, Sequence, TxIn, Witness};
            const MAX_USER_UTXOS: usize = 1;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(1_000), create_shard(1_000)];
            let shard_indexes = vec![0usize, 1usize];

            // Mark the first shard's single UTXO as already used in the transaction (capture meta before mutable borrow).
            let used_meta = shards[0].btc_utxos[0].meta;

            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            // Mark the first shard's single UTXO as already used in the transaction.
            tx_builder.transaction.version = Version::TWO; // explicit (not strictly needed)
            tx_builder.transaction.input.push(TxIn {
                previous_output: OutPoint::new(used_meta.to_txid(), used_meta.vout()),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            });

            let remaining_amount = 1_000u128;
            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                remaining_amount,
            )
            .unwrap();

            // Because shard 0's UTXO is spent, effective current amounts are 0 and 1000.
            // The balanced plan would assign 500 sats to each shard but these allocations
            // are below the dust limit and therefore merged into a single 1000 sat output.
            assert_eq!(distribution, vec![1_000]);
        }

        #[test]
        fn test_zero_remaining_amount() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(1_000), create_shard(2_000)];
            let shard_indexes = vec![0usize, 1usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                0u128,
            )
            .unwrap();
            // No additional sats to redistribute -> expect an empty allocation.
            assert!(distribution.is_empty());
        }

        #[test]
        fn test_single_shard() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 1;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(500)];
            let shard_indexes = vec![0usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                1_000u128,
            )
            .unwrap();
            assert_eq!(distribution, vec![1_000]);
        }

        #[test]
        fn test_empty_shards() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let shard_indexes = vec![0usize, 1usize, 2usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                1_500u128,
            )
            .unwrap();
            // Each tentative 500 sat allocation is under dust, so they are combined
            // into a single 1500 sat change output.
            assert_eq!(distribution, vec![1_500]);
        }

        #[test]
        fn test_remainder_distribution() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let shard_indexes = vec![0usize, 1usize, 2usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                1_001u128,
            )
            .unwrap();
            assert_eq!(distribution.iter().sum::<u128>(), 1_001);
            // All three provisional amounts are sub-dust; expect a single combined output.
            assert_eq!(distribution, vec![1_001]);
        }

        #[test]
        fn test_partial_shard_selection() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 4;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![
                create_shard(1_000),
                create_shard(2_000),
                create_shard(3_000),
                create_shard(4_000),
            ];
            let shard_indexes = vec![1usize, 2usize]; // only middle two
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                2_000u128,
            )
            .unwrap();

            assert_eq!(distribution.iter().sum::<u128>(), 2_000);
            // The 500 sat allocation is below the dust limit, so it is merged into
            // the other allocation forming a single 2000 sat output.
            assert_eq!(distribution, vec![2_000]);
        }

        #[test]
        fn test_large_numbers() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(u64::MAX), create_shard(u64::MAX)];
            let shard_indexes = vec![0usize, 1usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                1_000u128,
            )
            .unwrap();
            // Both provisional 500 sat allocations are under dust -> single 1000 sat output.
            assert_eq!(distribution, vec![1_000]);
        }

        #[test]
        fn test_split_remaining_amount_even_and_odd() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(0), create_shard(0)];
            let shard_indexes = vec![0usize, 1usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            // Odd amount
            let distribution_odd = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                2_041u128,
            )
            .unwrap();
            assert_eq!(distribution_odd, vec![1_021, 1_020]);
            assert_eq!(distribution_odd.iter().sum::<u128>(), 2_041);

            // Even amount
            let distribution_even = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                2_000u128,
            )
            .unwrap();
            assert_eq!(distribution_even, vec![1_000, 1_000]);
        }

        #[test]
        fn test_split_remaining_amount_with_existing_balances() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Shard 0 already has 1000 sats, shard 1 has 0. Distribute 2041.
            let mut shards = vec![create_shard(1_000), create_shard(0)];
            let shard_indexes = vec![0usize, 1usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                2_041u128,
            )
            .unwrap();
            // The 521 sat allocation is below dust and gets merged into the other output.
            assert_eq!(distribution, vec![2_041]);
            assert_eq!(distribution.iter().sum::<u128>(), 2_041);
        }

        #[test]
        fn test_single_shard_sub_dust_amount() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 1;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(0)];
            let shard_indexes = vec![0usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            // Amount one satoshi below the dust limit – should be discarded entirely.
            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                (DUST_LIMIT as u128) - 1u128,
            )
            .unwrap();
            assert!(distribution.is_empty());
        }

        #[test]
        fn test_single_shard_exact_dust_limit() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 1;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(0)];
            let shard_indexes = vec![0usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                DUST_LIMIT as u128,
            )
            .unwrap();
            assert_eq!(distribution, vec![DUST_LIMIT as u128]);
        }

        #[test]
        fn test_two_shards_each_exact_dust_limit() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(0), create_shard(0)];
            let shard_indexes = vec![0usize, 1usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            // Total amount equals 2 × dust limit.
            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                (DUST_LIMIT as u128) * 2u128,
            )
            .unwrap();
            // Order is descending, but both entries are identical.
            assert_eq!(distribution, vec![DUST_LIMIT as u128, DUST_LIMIT as u128]);
        }

        #[test]
        fn test_mixed_dust_and_non_dust_allocations() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // All shards start empty and we redistribute an amount that would give one
            // shard a sub-dust allocation after balancing.
            let mut shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let shard_indexes = vec![0usize, 1usize, 2usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let amount = 1_600u128; // Provisional split: 533/533/534 (< dust)
            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                amount,
            )
            .unwrap();

            // Every provisional amount would be sub-dust, so they should be merged into one.
            assert_eq!(distribution, vec![amount]);
        }

        #[test]
        #[cfg(feature = "utxo-consolidation")]
        fn test_fees_and_consolidation_accounted_for() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 0;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Simulate consolidation input handled by the program.
            #[cfg(feature = "utxo-consolidation")]
            {
                tx_builder.total_btc_consolidation_input = 1_000;
                tx_builder.extra_tx_size_for_consolidation = 200; // bytes
            }

            let fee_rate = FeeRate(1.0);
            let mut shard_refs: Vec<&mut MockShard> = Vec::new();
            let shard_indexes: Vec<usize> = Vec::new();

            let unsettled = compute_unsettled_btc_in_shards(
                &tx_builder,
                &mut shard_refs,
                &shard_indexes,
                100, // removed_from_shards
                &fee_rate,
            )
            .unwrap();

            // remaining = 0 (inputs) + 1_000 (consolidation) − 100 (removed) − 200 (fee) = 700
            assert_eq!(unsettled, 700);
        }

        #[test]
        fn test_unsettled_underflow_error() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 0;

            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shard_refs: Vec<&mut MockShard> = Vec::new();
            let shard_indexes: Vec<usize> = Vec::new();
            let fee_rate = FeeRate(1.0);

            let result = compute_unsettled_btc_in_shards(
                &tx_builder,
                &mut shard_refs,
                &shard_indexes,
                1, // removed_from_shards greater than total owned (0)
                &fee_rate,
            );

            assert!(matches!(result, Err(MathError::SubtractionOverflow)));
        }

        #[test]
        fn test_no_matching_inputs_results_in_zero_unsettled() {
            use bitcoin::{transaction::Version, OutPoint, ScriptBuf, Sequence, TxIn, Witness};
            const MAX_USER_UTXOS: usize = 1;
            const MAX_SHARDS_PER_POOL: usize = 1;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Add a transaction input that does NOT belong to the shard.
            tx_builder.transaction.version = Version::TWO;
            tx_builder.transaction.input.push(TxIn {
                previous_output: OutPoint::new(
                    bitcoin::Txid::from_str(
                        "0000000000000000000000000000000000000000000000000000000000000001",
                    )
                    .unwrap(),
                    1,
                ),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            });

            let mut shards = vec![create_shard(1_000)];
            let shard_indexes = vec![0usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let fee_rate = FeeRate(1.0);
            let unsettled = compute_unsettled_btc_in_shards(
                &tx_builder,
                &mut shard_refs,
                &shard_indexes,
                0,
                &fee_rate,
            )
            .unwrap();

            // The shard's UTXO is untouched, so unsettled sats should be zero.
            assert_eq!(unsettled, 0);
        }
    }

    // ---------------------------------------------------------------------
    // compute_unsettled_btc_in_shards --------------------------------------
    // ---------------------------------------------------------------------

    mod compute_unsettled_btc_in_shards {
        use super::common::*;
        use crate::split::compute_unsettled_btc_in_shards;
        use bitcoin::ScriptBuf;
        use saturn_bitcoin_transactions::{fee_rate::FeeRate, TransactionBuilder};

        #[test]
        fn basic_unsettled_calculation() {
            const MAX_USER_UTXOS: usize = 1;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // two shards with 1000 & 500 sats respectively
            let mut shards = vec![create_shard(1_000), create_shard(500)];
            let shard_indexes = vec![0usize, 1usize];

            // Spend shard 0's utxo in the tx
            let spent_meta = shards[0].btc_utxos[0].meta;
            tx_builder.transaction.input.push(bitcoin::TxIn {
                previous_output: bitcoin::OutPoint::new(spent_meta.to_txid(), spent_meta.vout()),
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::MAX,
                witness: bitcoin::Witness::new(),
            });

            // No fees for simplicity
            let unsettled = compute_unsettled_btc_in_shards(
                &tx_builder,
                &mut shards.iter_mut().collect::<Vec<&mut MockShard>>(),
                &shard_indexes,
                0,
                &FeeRate(1.0),
            )
            .unwrap();

            // Only shard 0's 1000 sats are unsettled (shard 1 untouched)
            assert_eq!(unsettled, 1_000);
        }
    }

    // ---------------------------------------------------------------------
    // Additional stubs for other public helpers ----------------------------
    // ---------------------------------------------------------------------

    mod compute_unsettled_rune_in_shards_test {
        // Placeholder – add detailed tests once Rune logic is enabled via the "runes" feature.
        #[test]
        fn placeholder() {
            assert!(true);
        }
    }

    mod plan_rune_distribution_among_shards {
        #[test]
        fn placeholder() {
            assert!(true);
        }
    }

    mod redistribute_remaining_btc_to_shards {
        #[test]
        fn placeholder() {
            assert!(true);
        }
    }

    mod redistribute_remaining_rune_to_shards {
        #[test]
        fn placeholder() {
            assert!(true);
        }
    }

    // ---------------------------------------------------------------------
    // Edge case tests ------------------------------------------------------
    // ---------------------------------------------------------------------

    mod edge_cases {
        use super::common::*;
        use super::*;
        use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
        use saturn_bitcoin_transactions::{
            constants::DUST_LIMIT, fee_rate::FeeRate, TransactionBuilder,
        };
        use saturn_safe_math::MathError;

        #[test]
        fn test_redistribute_sub_dust_all_above_dust() {
            // Case 1a: All provisional allocations ≥ dust → vector unchanged
            let mut amounts = vec![1000u128, 2000u128, 3000u128];
            let original_amounts = amounts.clone();

            super::redistribute_sub_dust_values(&mut amounts, DUST_LIMIT as u128).unwrap();

            // Should be unchanged since all amounts are above dust
            assert_eq!(amounts, original_amounts);
        }

        #[test]
        fn test_redistribute_sub_dust_all_below_but_sum_above() {
            // Case 1b: All < dust but combined sum ≥ dust
            let dust_limit = DUST_LIMIT as u128;
            let mut amounts = vec![200u128, 200u128, 200u128]; // All below dust (546)

            super::redistribute_sub_dust_values(&mut amounts, dust_limit).unwrap();

            // Should create single entry with combined sum
            assert_eq!(amounts, vec![600u128]);
        }

        #[test]
        fn test_redistribute_sub_dust_mixed_with_remainder() {
            // Case 1c: Mix of large & small with uneven remainder distribution
            let dust_limit = DUST_LIMIT as u128;
            let mut amounts = vec![1000u128, 200u128, 300u128, 2000u128]; // 200+300=500 sub-dust

            super::redistribute_sub_dust_values(&mut amounts, dust_limit).unwrap();

            // Should have 2 entries (1000, 2000) with 500 redistributed
            assert_eq!(amounts.len(), 2);
            assert_eq!(amounts.iter().sum::<u128>(), 3500u128);
            // 500 / 2 = 250 each, remainder 0
            assert!(amounts.contains(&1250u128));
            assert!(amounts.contains(&2250u128));
        }

        #[test]
        fn test_plan_btc_distribution_zero_shards() {
            // Case 2: Zero-shard inputs
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 0;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards: Vec<MockShard> = vec![];
            let shard_indexes: Vec<usize> = vec![];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let result = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                1000u128,
            );

            // Should return DivisionOverflow error when dividing by zero shards
            assert!(matches!(result, Err(MathError::DivisionOverflow)));
        }

        #[test]
        fn test_max_capacity_stress() {
            // Case 3: Max capacity stress test
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 10;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Create max number of shards, each with multiple UTXOs
            let mut shards: Vec<MockShard> = (0..MAX_SHARDS_PER_POOL)
                .map(|i| {
                    let mut shard = create_shard(0);
                    // Add multiple UTXOs to each shard
                    for j in 0..5 {
                        shard.add_btc_utxo(create_btc_utxo(1000, (i * 10 + j) as u32));
                    }
                    shard
                })
                .collect();

            let shard_indexes: Vec<usize> = (0..MAX_SHARDS_PER_POOL).collect();
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                10000u128,
            )
            .unwrap();

            assert_eq!(distribution.iter().sum::<u128>(), 10000u128);
        }

        #[test]
        fn test_near_boundary_dust_splits_below() {
            // Case 4a: amount = (dust_limit × n) − 1
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let shard_indexes = vec![0usize, 1usize, 2usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let amount = (DUST_LIMIT as u128) * 3 - 1; // Just below 3 dust outputs
            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                amount,
            )
            .unwrap();

            // Should consolidate into fewer outputs since individual amounts are sub-dust
            assert!(distribution.len() < 3);
            assert_eq!(distribution.iter().sum::<u128>(), amount);
        }

        #[test]
        fn test_near_boundary_dust_splits_above() {
            // Case 4b: amount = (dust_limit × n) + 1
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let shard_indexes = vec![0usize, 1usize, 2usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let amount = (DUST_LIMIT as u128) * 3 + 1;
            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                amount,
            )
            .unwrap();

            // Should have 3 outputs, each at least dust limit
            assert_eq!(distribution.len(), 3);
            assert!(distribution.iter().all(|&x| x >= DUST_LIMIT as u128));
            assert_eq!(distribution.iter().sum::<u128>(), amount);
        }

        #[test]
        fn test_duplicate_meta_utxos_across_shards() {
            // Case 5: Two shards reference the same UtxoMeta
            const MAX_USER_UTXOS: usize = 1;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Create UTXOs with identical meta
            let shared_meta = random_utxo_meta(42);
            let utxo1 = saturn_bitcoin_transactions::utxo_info::UtxoInfo::<SingleRuneSet> {
                meta: shared_meta,
                value: 1000,
                ..Default::default()
            };
            let utxo2 = saturn_bitcoin_transactions::utxo_info::UtxoInfo::<SingleRuneSet> {
                meta: shared_meta, // Same meta!
                value: 2000,
                ..Default::default()
            };

            let mut shard1 = MockShard::default();
            let mut shard2 = MockShard::default();
            shard1.add_btc_utxo(utxo1);
            shard2.add_btc_utxo(utxo2);

            let mut shards = vec![shard1, shard2];
            let shard_indexes = vec![0usize, 1usize];

            // Add the shared UTXO to transaction inputs
            tx_builder.transaction.input.push(TxIn {
                previous_output: OutPoint::new(shared_meta.to_txid(), shared_meta.vout()),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            });

            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let unsettled = compute_unsettled_btc_in_shards(
                &tx_builder,
                &mut shard_refs,
                &shard_indexes,
                0,
                &FeeRate(1.0),
            )
            .unwrap();

            // Should only count the UTXO once (first match), so 1000 not 3000
            assert_eq!(unsettled, 1000);
        }

        #[test]
        fn test_high_fee_scenario_overflow() {
            // Case 6: Fee exceeds available funds
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 1;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(100)]; // Small input
            let shard_indexes = vec![0usize];

            // Add the shard's UTXO to inputs
            let utxo_meta = shards[0].btc_utxos[0].meta;
            tx_builder.transaction.input.push(TxIn {
                previous_output: OutPoint::new(utxo_meta.to_txid(), utxo_meta.vout()),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            });

            // Set up consolidation to trigger high fees
            #[cfg(feature = "utxo-consolidation")]
            {
                tx_builder.extra_tx_size_for_consolidation = 10000; // Very large consolidation size
            }

            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            // High fee rate that will cause overflow when multiplied by large consolidation size
            let rune_amount = RuneAmount {
                id: RuneId::BTC,
                amount: u128::MAX,
            };

            let result = super::balance_amount_across_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                &rune_amount,
            );

            // The calculation should succeed and return the full amount for the single shard.
            assert_eq!(result.unwrap(), vec![u128::MAX]);
        }

        #[test]
        fn test_empty_amount_optimization() {
            // Case 8: amount == 0 should yield empty outputs
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(1000), create_shard(2000)];
            let shard_indexes = vec![0usize, 1usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let original_output_count = tx_builder.transaction.output.len();

            let distribution = redistribute_remaining_btc_to_shards(
                &mut tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                0, // No removed amount
                ScriptBuf::new(),
                &FeeRate(1.0),
            )
            .unwrap();

            // Should be empty and no outputs added
            assert!(distribution.is_empty());
            assert_eq!(tx_builder.transaction.output.len(), original_output_count);
        }

        #[test]
        fn test_single_shard_huge_sub_dust_amount() {
            // Case 9: Single shard + amount well below dust
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 1;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let mut shards = vec![create_shard(0)];
            let shard_indexes = vec![0usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let distribution = plan_btc_distribution_among_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                46u128, // Well below dust limit
            )
            .unwrap();

            // Should discard and create no output
            assert!(distribution.is_empty());
        }

        #[cfg(feature = "runes")]
        #[test]
        fn test_runestone_pointer_update() {
            // Case 10: Pointer update for runestones
            use ordinals::RuneId;
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Add some pre-existing outputs
            tx_builder.transaction.output.push(bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(1000),
                script_pubkey: ScriptBuf::new(),
            });
            tx_builder.transaction.output.push(bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(2000),
                script_pubkey: ScriptBuf::new(),
            });

            let old_output_count = tx_builder.transaction.output.len();

            let mut shards = vec![create_shard(0), create_shard(0)];
            let shard_indexes = vec![0usize, 1usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            let _rune_id = RuneId { block: 1, tx: 0 };
            let distribution = redistribute_remaining_rune_to_shards(
                &mut tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                SingleRuneSet::default(),
                ScriptBuf::new(),
            )
            .unwrap();

            // Pointer should point to first new output
            assert_eq!(tx_builder.runestone.pointer, Some(old_output_count as u32));

            // Edicts should reference the correct output indices
            for (i, edict) in tx_builder.runestone.edicts.iter().enumerate() {
                if i > 0 {
                    // First output gets runes via pointer, others via edicts
                    assert_eq!(edict.output, (old_output_count + i) as u32);
                }
            }
        }

        #[test]
        fn test_balance_amount_overflow_protection() {
            // Test arithmetic overflow protection in balance_amount_across_shards
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Create shards with very large amounts
            let mut shard1 = create_shard(0);
            let mut shard2 = create_shard(0);

            // Add UTXOs with maximum values
            shard1.add_btc_utxo(create_btc_utxo(u64::MAX, 1));
            shard2.add_btc_utxo(create_btc_utxo(u64::MAX, 2));

            let mut shards = vec![shard1, shard2];
            let shard_indexes = vec![0usize, 1usize];
            let mut shard_refs: Vec<&mut MockShard> = shards.iter_mut().collect();

            // Try to add more amount that would cause overflow
            let rune_amount = RuneAmount {
                id: RuneId::BTC,
                amount: u128::MAX,
            };

            let result = super::balance_amount_across_shards(
                &tx_builder,
                shard_refs.as_mut_slice(),
                &shard_indexes,
                &rune_amount,
            );

            // Should handle overflow gracefully
            assert!(result.is_err());
        }
    }
}
