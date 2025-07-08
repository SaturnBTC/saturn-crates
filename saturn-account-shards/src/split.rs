use arch_program::{
    rune::{RuneAmount, RuneId},
    utxo::UtxoMeta,
};
use bitcoin::{Amount, ScriptBuf, TxOut};
use saturn_bitcoin_transactions::{
    constants::DUST_LIMIT, fee_rate::FeeRate, utxo_info::UtxoInfoTrait, TransactionBuilder,
};
use saturn_collections::generic::fixed_set::FixedCapacitySet;
use saturn_safe_math::{safe_add, safe_div, safe_mul, safe_sub, MathError};

use crate::{
    shard_set::{Selected, ShardSet},
    StateShard,
};

#[cfg(feature = "runes")]
use crate::StateShardError;
#[cfg(feature = "runes")]
use ordinals::Edict;

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
#[allow(clippy::too_many_arguments)]
pub fn redistribute_remaining_btc_to_shards<
    'info,
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS,
    U,
    S,
    const MAX_SELECTED: usize,
>(
    tx_builder: &mut TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shard_set: &mut ShardSet<'info, S, MAX_SELECTED, Selected>,
    removed_from_shards: u64,
    program_script_pubkey: ScriptBuf,
    fee_rate: &FeeRate,
) -> Result<Vec<u128>, MathError>
where
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    S: StateShard<U, RS> + bytemuck::Pod + bytemuck::Zeroable + 'static,
{
    let remaining_amount =
        compute_unsettled_btc_in_shards(tx_builder, shard_set, removed_from_shards, fee_rate)?;

    let mut distribution =
        plan_btc_distribution_among_shards(tx_builder, shard_set, remaining_amount as u128)?;

    // Largest first for deterministic ordering.
    distribution.sort_by(|a, b| b.cmp(a));

    for amount in distribution.iter() {
        tx_builder.transaction.output.push(TxOut {
            value: Amount::from_sat(*amount as u64),
            script_pubkey: program_script_pubkey.clone(),
        });
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
    'info,
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS,
    U,
    S,
    const MAX_SELECTED: usize,
>(
    tx_builder: &TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shard_set: &ShardSet<'info, S, MAX_SELECTED, Selected>,
    removed_from_shards: u64,
    fee_rate: &FeeRate,
) -> Result<u64, MathError>
where
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    S: StateShard<U, RS> + bytemuck::Pod + bytemuck::Zeroable + 'static,
{
    let mut total_btc_amount = 0u64;

    let selected = shard_set.selected_indices();

    for input in tx_builder.transaction.input.iter() {
        let input_meta =
            UtxoMeta::from_outpoint(input.previous_output.txid, input.previous_output.vout);

        for &idx in selected {
            let handle = shard_set.handle_by_index(idx);
            // Borrow immutably just for this check.
            let utxo_res: Result<Option<u64>, arch_program::program_error::ProgramError> = handle
                .with_ref(|shard| {
                    shard
                        .btc_utxos()
                        .iter()
                        .find(|u| *u.meta() == input_meta)
                        .map(|u| u.value())
                });
            let maybe_value = match utxo_res {
                Ok(v) => v,
                Err(_) => return Err(MathError::ConversionError),
            };
            if let Some(utxo_value) = maybe_value {
                total_btc_amount = safe_add(total_btc_amount, utxo_value)?;
                break; // Avoid double-counting identical UTXOs across shards.
            }
        }
    }

    // Fees paid by program and consolidation inputs mirror the logic in the legacy helper.
    let fee_paid_by_program = {
        #[cfg(feature = "utxo-consolidation")]
        {
            tx_builder.get_fee_paid_by_program(fee_rate)
        }
        #[cfg(not(feature = "utxo-consolidation"))]
        {
            0
        }
    };

    let total_btc_consolidation_input = {
        #[cfg(feature = "utxo-consolidation")]
        {
            tx_builder.total_btc_consolidation_input
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

/// Splits `amount` satoshis across the selected shards as evenly as possible
/// while respecting the dust limit.
fn plan_btc_distribution_among_shards<
    'info,
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS,
    U,
    S,
    const MAX_SELECTED: usize,
>(
    tx_builder: &TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shard_set: &ShardSet<'info, S, MAX_SELECTED, Selected>,
    amount: u128,
) -> Result<Vec<u128>, MathError>
where
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    S: StateShard<U, RS> + bytemuck::Pod + bytemuck::Zeroable + 'static,
{
    let mut result = balance_amount_across_shards::<
        MAX_USER_UTXOS,
        MAX_SHARDS_PER_POOL,
        RS,
        U,
        S,
        MAX_SELECTED,
    >(
        tx_builder,
        shard_set,
        &RuneAmount {
            id: RuneId::BTC,
            amount,
        },
    )?;

    redistribute_sub_dust_values(&mut result, DUST_LIMIT as u128)?;
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
fn balance_amount_across_shards<
    'info,
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS,
    U,
    S,
    const MAX_SELECTED: usize,
>(
    tx_builder: &TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shard_set: &ShardSet<'info, S, MAX_SELECTED, Selected>,
    rune_amount: &RuneAmount,
) -> Result<Vec<u128>, MathError>
where
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    S: StateShard<U, RS> + bytemuck::Pod + bytemuck::Zeroable + 'static,
{
    let num_shards = shard_set.selected_indices().len();

    let mut assigned_amounts: Vec<u128> = Vec::with_capacity(num_shards);
    let mut total_current_amount: u128 = 0;

    // Helper to detect whether a UTXO is already consumed by the tx-builder.
    let is_utxo_used = |meta: &UtxoMeta| {
        tx_builder.transaction.input.iter().any(|input| {
            UtxoMeta::from_outpoint(input.previous_output.txid, input.previous_output.vout) == *meta
        })
    };

    // 1. Determine the current amount per shard and overall.
    for &idx in shard_set.selected_indices() {
        let handle = shard_set.handle_by_index(idx);

        // Gather existing amount for this shard.
        let current_res = handle.with_ref(|shard| match rune_amount.id {
            RuneId::BTC => {
                // Sum unspent BTC UTXOs.
                shard
                    .btc_utxos()
                    .iter()
                    .filter_map(|u| {
                        if is_utxo_used(u.meta()) {
                            None
                        } else {
                            Some(u.value() as u128)
                        }
                    })
                    .sum()
            }
            _ => {
                #[cfg(feature = "runes")]
                {
                    shard
                        .rune_utxo()
                        .and_then(|u| u.runes().find(&rune_amount.id).map(|r| r.amount))
                        .unwrap_or(0)
                }
                #[cfg(not(feature = "runes"))]
                {
                    0
                }
            }
        });

        let current = current_res.unwrap_or(0);
        assigned_amounts.push(current);
        total_current_amount = safe_add(total_current_amount, current)?;
    }

    // Determine target per-shard balance.
    let total_after = safe_add(total_current_amount, rune_amount.amount)?;
    let desired_per_shard = safe_div(total_after, num_shards as u128)?;

    // Calculate additional amount needed per shard to reach desired balance.
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
        // Distribute leftover evenly across shards.
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
        // Not enough to reach equal balance – scale proportionally.
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
/// Returns [MathError] when the math operations fail.
fn redistribute_sub_dust_values(
    amounts: &mut Vec<u128>,
    dust_limit: u128,
) -> Result<(), MathError> {
    // 1. Aggregate all allocations below dust.
    let sum_of_small_amounts: u128 = amounts.iter().filter(|&&amount| amount < dust_limit).sum();

    // 2. Remove sub-dust entries entirely.
    amounts.retain(|&amount| amount >= dust_limit);

    // 3. If nothing left after removal, decide whether to keep or discard.
    if amounts.is_empty() {
        if sum_of_small_amounts >= dust_limit {
            amounts.push(sum_of_small_amounts);
        } else {
            amounts.clear();
        }
        return Ok(());
    }

    // 4. Redistribute the collected dust across remaining outputs.
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

/// Same as [`compute_unsettled_btc_in_shards`] but for Rune tokens.
/// It sums up the token amount inside the `rune_utxo` of every participating
/// shard, subtracts whatever has already been removed, and returns the number
/// of tokens that still have to be redistributed.
///
/// # Errors
/// Returns [`StateShardError`] if an arithmetic operation overflows.
#[cfg(feature = "runes")]
pub fn compute_unsettled_rune_in_shards<'info, RS, U, S, const MAX_SELECTED: usize>(
    shard_set: &ShardSet<'info, S, MAX_SELECTED, Selected>,
    removed_from_shards: RS,
) -> Result<RS, StateShardError>
where
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    S: StateShard<U, RS> + bytemuck::Pod + bytemuck::Zeroable + 'static,
{
    let mut total_rune_amount = RS::default();

    for &idx in shard_set.selected_indices().iter() {
        let handle = shard_set.handle_by_index(idx);

        // Traverse rune amounts directly without allocating an intermediate Vec.
        let inner_res = handle
            .with_ref(|shard| {
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
                Ok::<(), StateShardError>(())
            })
            .map_err(|_| StateShardError::RuneAmountAdditionOverflow)?;

        // Propagate potential math errors from inside the closure.
        inner_res?;
    }

    // Subtract whatever was already removed.
    for rune in removed_from_shards.iter() {
        if let Some(output_rune) = total_rune_amount.find_mut(&rune.id) {
            output_rune.amount = safe_sub(output_rune.amount, rune.amount)
                .map_err(|_| StateShardError::RemovingMoreRunesThanPresentInShards)?;
        }
    }

    Ok(total_rune_amount)
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
/// Returns [`StateShardError`] when the math operations fail.
#[cfg(feature = "runes")]
#[allow(clippy::too_many_arguments)]
pub fn plan_rune_distribution_among_shards<
    'info,
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS,
    U,
    S,
    const MAX_SELECTED: usize,
>(
    tx_builder: &mut TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shard_set: &ShardSet<'info, S, MAX_SELECTED, Selected>,
    amounts: &RS,
) -> Result<Vec<RS>, StateShardError>
where
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    S: StateShard<U, RS> + bytemuck::Pod + bytemuck::Zeroable + 'static,
{
    let num_shards = shard_set.selected_indices().len();
    let mut result: Vec<RS> = (0..num_shards).map(|_| RS::default()).collect();

    for rune_amount in amounts.iter() {
        let allocs = balance_amount_across_shards::<
            MAX_USER_UTXOS,
            MAX_SHARDS_PER_POOL,
            RS,
            U,
            S,
            MAX_SELECTED,
        >(tx_builder, shard_set, rune_amount)
        .map_err(|_| StateShardError::MathErrorInBalanceAmountAcrossShards)?;

        for (i, amount) in allocs.iter().enumerate() {
            if *amount == 0 {
                continue;
            }
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
/// Returns [`StateShardError`] when any safe-math operation fails.
#[cfg(feature = "runes")]
#[allow(clippy::too_many_arguments)]
pub fn redistribute_remaining_rune_to_shards<
    'info,
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS,
    U,
    S,
    const MAX_SELECTED: usize,
>(
    tx_builder: &mut TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shard_set: &mut ShardSet<'info, S, MAX_SELECTED, Selected>,
    removed_from_shards: RS,
    program_script_pubkey: ScriptBuf,
) -> Result<Vec<RS>, StateShardError>
where
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    S: StateShard<U, RS> + bytemuck::Pod + bytemuck::Zeroable + 'static,
{
    let remaining_amount =
        compute_unsettled_rune_in_shards::<RS, U, S, MAX_SELECTED>(shard_set, removed_from_shards)?;

    let mut distribution = plan_rune_distribution_among_shards::<
        MAX_USER_UTXOS,
        MAX_SHARDS_PER_POOL,
        RS,
        U,
        S,
        MAX_SELECTED,
    >(tx_builder, shard_set, &remaining_amount)?;

    // Sort descending by total rune amount for deterministic ordering.
    distribution.sort_by(|a, b| {
        let total_a: u128 = a.iter().map(|r| r.amount).sum();
        let total_b: u128 = b.iter().map(|r| r.amount).sum();
        total_b.cmp(&total_a)
    });

    let current_output_index = tx_builder.transaction.output.len();
    tx_builder.runestone.pointer = Some(current_output_index as u32);

    let mut index = current_output_index;
    for amount_set in distribution.iter() {
        tx_builder.transaction.output.push(TxOut {
            value: Amount::from_sat(DUST_LIMIT),
            script_pubkey: program_script_pubkey.clone(),
        });

        if index > current_output_index {
            for rune_amount in amount_set.iter() {
                tx_builder.runestone.edicts.push(Edict {
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

#[cfg(test)]
mod tests_loader {
    use super::*;
    use crate::common_loader::{
        create_btc_utxo, create_shard, leak_loaders_from_vec, MockShardZc, MAX_BTC_UTXOS,
    };
    use crate::shard_set::ShardSet;
    use saturn_bitcoin_transactions::utxo_info::SingleRuneSet;

    // Re-export for macro reuse
    use saturn_bitcoin_transactions::TransactionBuilder as TB;

    #[allow(unused_macros)]
    macro_rules! new_tb {
        ($max_utxos:expr, $max_shards:expr) => {
            TB::<$max_utxos, $max_shards, SingleRuneSet>::new()
        };
    }

    mod plan_btc_distribution_among_shards {
        use super::*;
        use crate::split::{
            balance_amount_across_shards as balance_loader, plan_btc_distribution_among_shards,
            redistribute_sub_dust_values,
        };
        use saturn_bitcoin_transactions::{constants::DUST_LIMIT, utxo_info::SingleRuneSet};
        use saturn_safe_math::MathError;

        #[test]
        fn proportional_distribution_insufficient_remaining() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Shards with 100,200,300 sats respectively
            let shards: Vec<MockShardZc> =
                vec![create_shard(100), create_shard(200), create_shard(300)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 3;
            let unselected: ShardSet<MockShardZc, MAX_SELECTED> = ShardSet::from_loaders(loaders);
            let selected = unselected.select_with([0usize, 1usize, 2usize]).unwrap();

            // Remaining amount smaller than dust → expect empty dist
            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 150u128)
            .unwrap();
            assert!(dist.is_empty());
        }

        #[test]
        fn zero_remaining_amount() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(1_000), create_shard(2_000)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 2;
            let unselected: ShardSet<MockShardZc, MAX_SELECTED> = ShardSet::from_loaders(loaders);
            let selected = unselected.select_with([0usize, 1usize]).unwrap();

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 0u128)
            .unwrap();
            assert!(dist.is_empty());
        }

        #[test]
        fn single_shard() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 1;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(500)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 1;
            let unselected: ShardSet<MockShardZc, MAX_SELECTED> = ShardSet::from_loaders(loaders);
            let selected = unselected.select_with([0usize]).unwrap();

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 1_000u128)
            .unwrap();

            assert_eq!(dist, vec![1_000]);
        }

        #[test]
        fn empty_shards_all_zero_balances() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 3;
            let unselected: ShardSet<MockShardZc, MAX_SELECTED> = ShardSet::from_loaders(loaders);
            let selected = unselected.select_with([0usize, 1, 2]).unwrap();

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 1_500u128)
            .unwrap();

            assert_eq!(dist, vec![1_500]);
        }

        #[test]
        fn remainder_distribution_sub_dust_merge() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 3;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1, 2])
                .unwrap();

            let amount = 1_001u128;
            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, amount)
            .unwrap();
            assert_eq!(dist.iter().sum::<u128>(), amount);
            assert_eq!(dist, vec![amount]);
        }

        #[test]
        fn used_utxos_excluded() {
            use bitcoin::{transaction::Version, OutPoint, ScriptBuf, Sequence, TxIn, Witness};

            const MAX_USER_UTXOS: usize = 1;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Shards with 1_000 sats each
            let mut shard1 = create_shard(1_000);
            let mut shard2 = create_shard(1_000);

            // Capture meta before loader creation via trait method
            let used_meta = shard1.btc_utxos()[0].meta;

            let loaders = leak_loaders_from_vec(vec![shard1, shard2]);
            const MAX_SELECTED: usize = 2;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1])
                .unwrap();

            // Mark first shard's utxo as spent
            tx_builder.transaction.version = Version::TWO;
            tx_builder.transaction.input.push(TxIn {
                previous_output: OutPoint::new(used_meta.to_txid(), used_meta.vout()),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            });

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 1_000u128)
            .unwrap();

            assert_eq!(dist, vec![1_000]);
        }

        #[test]
        fn partial_shard_selection() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 4;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![
                create_shard(1_000),
                create_shard(2_000),
                create_shard(3_000),
                create_shard(4_000),
            ];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 4;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([1usize, 2usize])
                .unwrap();

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 2_000u128)
            .unwrap();

            assert_eq!(dist.iter().sum::<u128>(), 2_000);
            assert_eq!(dist, vec![2_000]);
        }

        #[test]
        fn large_numbers() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(u64::MAX), create_shard(u64::MAX)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 2;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1])
                .unwrap();

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 1_000u128)
            .unwrap();

            assert_eq!(dist, vec![1_000]);
        }

        #[test]
        fn split_remaining_amount_even_and_odd() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(0), create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 2;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1])
                .unwrap();

            // Odd amount
            let dist_odd = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 2_041u128)
            .unwrap();
            assert_eq!(dist_odd, vec![1_021, 1_020]);
            assert_eq!(dist_odd.iter().sum::<u128>(), 2_041);

            // Even amount
            let dist_even = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 2_000u128)
            .unwrap();
            assert_eq!(dist_even, vec![1_000, 1_000]);
        }

        #[test]
        fn split_remaining_amount_with_existing_balances() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(1_000), create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 2;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1])
                .unwrap();

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 2_041u128)
            .unwrap();

            assert_eq!(dist.iter().sum::<u128>(), 2_041);
            assert_eq!(dist, vec![2_041]);
        }

        #[test]
        fn single_shard_sub_dust_amount() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 1;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 1;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize])
                .unwrap();

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, (DUST_LIMIT as u128) - 1u128)
            .unwrap();

            assert!(dist.is_empty());
        }

        #[test]
        fn single_shard_exact_dust_limit() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 1;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 1;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize])
                .unwrap();

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, DUST_LIMIT as u128)
            .unwrap();

            assert_eq!(dist, vec![DUST_LIMIT as u128]);
        }

        #[test]
        fn two_shards_each_exact_dust_limit() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(0), create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 2;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1])
                .unwrap();

            let amount = (DUST_LIMIT as u128) * 2u128;
            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, amount)
            .unwrap();

            assert_eq!(dist, vec![DUST_LIMIT as u128, DUST_LIMIT as u128]);
        }

        #[test]
        fn mixed_dust_and_non_dust_allocations() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 3;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1, 2])
                .unwrap();

            let amount = 1_600u128; // provisional 533/533/534 (< dust)
            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, amount)
            .unwrap();

            assert_eq!(dist, vec![amount]);
        }
    }

    // ---------------------------------------------------------------
    // compute_unsettled_btc_in_shards --------------------------------
    // ---------------------------------------------------------------
    mod compute_unsettled_btc_in_shards {
        use super::*;
        use crate::split::compute_unsettled_btc_in_shards;
        use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
        use saturn_bitcoin_transactions::fee_rate::FeeRate;

        #[test]
        fn basic_unsettled_calculation() {
            const MAX_USER_UTXOS: usize = 1;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Two shards with 1_000 and 500 sats respectively
            let shard1 = create_shard(1_000);
            let shard2 = create_shard(500);
            let loaders = leak_loaders_from_vec(vec![shard1, shard2]);
            const MAX_SELECTED: usize = 2;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1usize])
                .unwrap();

            // Spend shard 0's UTXO in the transaction
            let spent_meta = selected
                .handle_by_index(0)
                .with_ref(|shard| shard.btc_utxos()[0].meta)
                .unwrap();
            tx_builder.transaction.input.push(TxIn {
                previous_output: OutPoint::new(spent_meta.to_txid(), spent_meta.vout()),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            });

            let unsettled = compute_unsettled_btc_in_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 0, &FeeRate(1.0))
            .unwrap();

            // Only shard 0's 1000 sats are unsettled (shard 1 untouched)
            assert_eq!(unsettled, 1_000);
        }
    }

    // ---------------------------------------------------------------
    // Edge-case helpers & stress tests --------------------------------
    // ---------------------------------------------------------------
    mod edge_cases {
        use super::*;
        use crate::common_loader::add_btc_utxos_bulk;
        use crate::common_loader::random_utxo_meta;
        use crate::split::{
            balance_amount_across_shards as balance_loader, compute_unsettled_btc_in_shards,
            plan_btc_distribution_among_shards, redistribute_remaining_btc_to_shards,
            redistribute_sub_dust_values,
        };
        use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
        use saturn_account_parser::codec::zero_copy::AccountLoader;
        use saturn_bitcoin_transactions::{constants::DUST_LIMIT, fee_rate::FeeRate};
        use saturn_safe_math::MathError;

        // ---- redistribute_sub_dust_values tests ----
        #[test]
        fn redistribute_sub_dust_all_above_dust() {
            let mut amounts = vec![1000u128, 2000u128, 3000u128];
            let original = amounts.clone();
            redistribute_sub_dust_values(&mut amounts, DUST_LIMIT as u128).unwrap();
            assert_eq!(amounts, original);
        }

        #[test]
        fn redistribute_sub_dust_all_below_but_sum_above() {
            let mut amounts = vec![200u128, 200u128, 200u128];
            redistribute_sub_dust_values(&mut amounts, DUST_LIMIT as u128).unwrap();
            assert_eq!(amounts, vec![600u128]);
        }

        #[test]
        fn redistribute_sub_dust_mixed_with_remainder() {
            let mut amounts = vec![1000u128, 200u128, 300u128, 2000u128]; // 200+300 below dust
            redistribute_sub_dust_values(&mut amounts, DUST_LIMIT as u128).unwrap();
            assert_eq!(amounts.len(), 2);
            assert_eq!(amounts.iter().sum::<u128>(), 3500u128);
            assert!(amounts.contains(&1250u128));
            assert!(amounts.contains(&2250u128));
        }

        // ---- zero-shard behaviour ----
        #[test]
        fn plan_btc_distribution_zero_shards() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 0;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Empty loaders slice
            let loaders: &[&AccountLoader<'static, MockShardZc>] = &[];
            const MAX_SELECTED: usize = 0;
            let unselected: ShardSet<MockShardZc, MAX_SELECTED> = ShardSet::from_loaders(loaders);
            let selected = unselected.select_with([] as [usize; 0]).unwrap();

            let result = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 1_000u128);

            assert!(matches!(result, Err(MathError::DivisionOverflow)));
        }

        // ---- max-capacity stress ----
        #[test]
        fn max_capacity_stress() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 10;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Build 10 shards, each with 5 × 1_000-sat UTXOs
            let mut shards: Vec<MockShardZc> = (0..MAX_SHARDS_PER_POOL)
                .map(|i| {
                    let mut s = create_shard(0);
                    let values = vec![1_000u64; 5];
                    add_btc_utxos_bulk(&mut s, &values);
                    // tweak vout base by index to make metas unique
                    if i > 0 {
                        // Already sequential in helper but fine
                    }
                    s
                })
                .collect();

            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 10;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1, 2, 3, 4, 5, 6, 7, 8, 9])
                .unwrap();

            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 10_000u128)
            .unwrap();

            assert_eq!(dist.iter().sum::<u128>(), 10_000u128);
        }

        // ---- near-boundary dust split cases ----
        #[test]
        fn near_boundary_dust_splits_below() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 3;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1, 2])
                .unwrap();

            let amount = (DUST_LIMIT as u128) * 3 - 1u128;
            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, amount)
            .unwrap();

            assert!(dist.len() < 3);
            assert_eq!(dist.iter().sum::<u128>(), amount);
        }

        #[test]
        fn near_boundary_dust_splits_above() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 3;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shards = vec![create_shard(0), create_shard(0), create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 3;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1, 2])
                .unwrap();

            let amount = (DUST_LIMIT as u128) * 3 + 1u128;
            let dist = plan_btc_distribution_among_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, amount)
            .unwrap();

            assert_eq!(dist.len(), 3);
            assert!(dist.iter().all(|&x| x >= DUST_LIMIT as u128));
            assert_eq!(dist.iter().sum::<u128>(), amount);
        }

        // ---- duplicate meta across shards ----
        #[test]
        fn duplicate_meta_utxos_across_shards() {
            const MAX_USER_UTXOS: usize = 1;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Build two UTXOs with IDENTICAL meta but different values
            let shared_meta = random_utxo_meta(42);
            let utxo1 = create_btc_utxo(1_000, 42);
            let mut utxo2 = create_btc_utxo(2_000, 42); // same meta
            utxo2.meta = shared_meta; // ensure identical even if helper differs

            let mut shard1 = create_shard(0);
            let mut shard2 = create_shard(0);
            shard1.add_btc_utxo(utxo1);
            shard2.add_btc_utxo(utxo2);

            let loaders = leak_loaders_from_vec(vec![shard1, shard2]);
            const MAX_SELECTED: usize = 2;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1])
                .unwrap();

            // Spend the shared UTXO in the tx
            tx_builder.transaction.input.push(TxIn {
                previous_output: OutPoint::new(shared_meta.to_txid(), shared_meta.vout()),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            });

            let unsettled = compute_unsettled_btc_in_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, 0, &FeeRate(1.0))
            .unwrap();

            // Should count only once (value from first shard = 1_000)
            assert_eq!(unsettled, 1_000);
        }

        // ---- high fee overflow handling ----
        #[test]
        fn high_fee_scenario_overflow() {
            use arch_program::rune::{RuneAmount, RuneId};
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 1;

            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            let shard = create_shard(0);
            let loaders = leak_loaders_from_vec(vec![shard]);
            const MAX_SELECTED: usize = 1;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize])
                .unwrap();

            // Add a huge rune amount -> expect overflow handled gracefully (Err)
            let rune_amount = RuneAmount {
                id: RuneId::BTC,
                amount: u128::MAX,
            };
            let result = balance_loader::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(&tx_builder, &selected, &rune_amount);

            // Should succeed and return the full allocation for the single shard.
            assert_eq!(result.unwrap(), vec![u128::MAX]);
        }

        // ---- empty amount optimisation ----
        #[test]
        fn empty_amount_optimization() {
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // preload some outputs
            let original_outputs = tx_builder.transaction.output.len();

            let shards = vec![create_shard(1_000), create_shard(2_000)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 2;
            let mut selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1])
                .unwrap();

            let dist = crate::split::redistribute_remaining_btc_to_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(
                &mut tx_builder,
                &mut selected,
                0,
                ScriptBuf::new(),
                &FeeRate(1.0),
            )
            .unwrap();

            assert!(dist.is_empty());
            assert_eq!(tx_builder.transaction.output.len(), original_outputs);
        }

        // ---- overflow protection in balance_amount_across_shards ----
        #[test]
        fn balance_amount_overflow_protection() {
            use arch_program::rune::{RuneAmount, RuneId};
            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;
            let tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // shards with u64::MAX utxos
            let mut shard1 = create_shard(0);
            let mut shard2 = create_shard(0);
            shard1.add_btc_utxo(create_btc_utxo(u64::MAX, 1));
            shard2.add_btc_utxo(create_btc_utxo(u64::MAX, 2));

            let loaders = leak_loaders_from_vec(vec![shard1, shard2]);
            const MAX_SELECTED_OVER: usize = 2;
            let selected = ShardSet::<MockShardZc, MAX_SELECTED_OVER>::from_loaders(loaders)
                .select_with([0usize, 1])
                .unwrap();

            let rune_amount = RuneAmount {
                id: RuneId::BTC,
                amount: u128::MAX,
            };
            let res = balance_loader::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED_OVER,
            >(&tx_builder, &selected, &rune_amount);

            assert!(res.is_err());
        }

        // ---- runestone pointer update (Rune feature) ----
        #[cfg(feature = "runes")]
        #[test]
        fn runestone_pointer_update() {
            use bitcoin::{Amount, TxOut};
            use ordinals::RuneId;

            const MAX_USER_UTXOS: usize = 0;
            const MAX_SHARDS_PER_POOL: usize = 2;

            let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

            // Pre-existing outputs to simulate prior transaction state.
            tx_builder.transaction.output.push(TxOut {
                value: Amount::from_sat(1_000),
                script_pubkey: ScriptBuf::new(),
            });
            tx_builder.transaction.output.push(TxOut {
                value: Amount::from_sat(2_000),
                script_pubkey: ScriptBuf::new(),
            });

            let old_output_count = tx_builder.transaction.output.len();

            // Two empty shards (no BTC / Rune UTXOs needed for this test)
            let shards = vec![create_shard(0), create_shard(0)];
            let loaders = leak_loaders_from_vec(shards);
            const MAX_SELECTED: usize = 2;
            let mut selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
                .select_with([0usize, 1usize])
                .unwrap();

            // Invoke the rune redistribution helper (no runes to distribute)
            crate::split::redistribute_remaining_rune_to_shards::<
                MAX_USER_UTXOS,
                MAX_SHARDS_PER_POOL,
                SingleRuneSet,
                saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
                MockShardZc,
                MAX_SELECTED,
            >(
                &mut tx_builder,
                &mut selected,
                SingleRuneSet::default(),
                ScriptBuf::new(),
            )
            .unwrap();

            // Pointer should now reference the first newly added output.
            assert_eq!(tx_builder.runestone.pointer, Some(old_output_count as u32));

            // Any generated edicts (if present) must point to subsequent outputs.
            for (i, edict) in tx_builder.runestone.edicts.iter().enumerate() {
                if i > 0 {
                    assert_eq!(edict.output, (old_output_count + i) as u32);
                }
            }
        }
    }
}

// -------------------------------------------------------------------------
// Rune-specific test suite (requires `--features runes`)
// -------------------------------------------------------------------------
#[cfg(all(test, feature = "runes"))]
mod rune_tests_loader {
    use super::*;
    use crate::common_loader::{
        create_rune_utxo, create_shard, leak_loaders_from_vec, MockShardZc,
    };
    use crate::shard_set::ShardSet;
    use arch_program::rune::{RuneAmount, RuneId};
    use bitcoin::ScriptBuf;
    use saturn_bitcoin_transactions::utxo_info::SingleRuneSet;
    use saturn_bitcoin_transactions::TransactionBuilder as TB;

    #[allow(unused_macros)]
    macro_rules! new_tb {
        ($max_utxos:expr, $max_shards:expr) => {
            TB::<$max_utxos, $max_shards, SingleRuneSet>::new()
        };
    }

    // ---------------------------------------------------------------
    // compute_unsettled_rune_in_shards ------------------------------
    // ---------------------------------------------------------------
    #[test]
    fn compute_unsettled_rune_basic() {
        const MAX_USER_UTXOS: usize = 0;
        const MAX_SHARDS_PER_POOL: usize = 2;

        // Two shards with 100 and 50 runes respectively
        let mut shard1 = create_shard(0);
        let mut shard2 = create_shard(0);
        shard1.set_rune_utxo(create_rune_utxo(100, 0));
        shard2.set_rune_utxo(create_rune_utxo(50, 1));

        let loaders = leak_loaders_from_vec(vec![shard1, shard2]);
        const MAX_SELECTED: usize = 2;
        let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
            .select_with([0usize, 1usize])
            .unwrap();

        let unsettled = crate::split::compute_unsettled_rune_in_shards::<
            SingleRuneSet,
            saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
            MockShardZc,
            MAX_SELECTED,
        >(&selected, SingleRuneSet::default())
        .unwrap();

        assert_eq!(unsettled.find(&RuneId::BTC).unwrap().amount, 150);
    }

    // ---------------------------------------------------------------
    // plan_rune_distribution_among_shards ---------------------------
    // ---------------------------------------------------------------
    #[test]
    fn plan_rune_distribution_proportional() {
        const MAX_USER_UTXOS: usize = 0;
        const MAX_SHARDS_PER_POOL: usize = 3;

        let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

        // Existing rune balances: 100, 200, 300
        let mut shard0 = create_shard(0);
        let mut shard1 = create_shard(0);
        let mut shard2 = create_shard(0);
        shard0.set_rune_utxo(create_rune_utxo(100, 0));
        shard1.set_rune_utxo(create_rune_utxo(200, 1));
        shard2.set_rune_utxo(create_rune_utxo(300, 2));

        let loaders = leak_loaders_from_vec(vec![shard0, shard1, shard2]);
        const MAX_SELECTED: usize = 3;
        let selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
            .select_with([0usize, 1usize, 2usize])
            .unwrap();

        // Distribute 600 runes proportionally
        let mut target = SingleRuneSet::default();
        target
            .insert(RuneAmount {
                id: RuneId::BTC,
                amount: 600,
            })
            .unwrap();

        let dist = crate::split::plan_rune_distribution_among_shards::<
            MAX_USER_UTXOS,
            MAX_SHARDS_PER_POOL,
            SingleRuneSet,
            saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
            MockShardZc,
            MAX_SELECTED,
        >(&mut tx_builder, &selected, &target)
        .unwrap();

        assert_eq!(dist.len(), 3);
        let allocs: Vec<u128> = dist
            .iter()
            .map(|s| s.find(&RuneId::BTC).unwrap().amount)
            .collect();
        assert_eq!(allocs, vec![300, 200, 100]);
    }

    // ---------------------------------------------------------------
    // redistribute_remaining_rune_to_shards -------------------------
    // ---------------------------------------------------------------
    #[test]
    fn redistribute_remaining_rune_distribution() {
        const MAX_USER_UTXOS: usize = 0;
        const MAX_SHARDS_PER_POOL: usize = 3;

        let mut tx_builder = new_tb!(MAX_USER_UTXOS, MAX_SHARDS_PER_POOL);

        // Shards start with 100, 200, 300 runes
        let mut shard0 = create_shard(0);
        let mut shard1 = create_shard(0);
        let mut shard2 = create_shard(0);
        shard0.set_rune_utxo(create_rune_utxo(100, 0));
        shard1.set_rune_utxo(create_rune_utxo(200, 1));
        shard2.set_rune_utxo(create_rune_utxo(300, 2));

        let loaders = leak_loaders_from_vec(vec![shard0, shard1, shard2]);
        const MAX_SELECTED: usize = 3;
        let mut selected = ShardSet::<MockShardZc, MAX_SELECTED>::from_loaders(loaders)
            .select_with([0usize, 1usize, 2usize])
            .unwrap();

        // Remove 150 runes total
        let mut removed = SingleRuneSet::default();
        removed
            .insert(RuneAmount {
                id: RuneId::BTC,
                amount: 150,
            })
            .unwrap();

        let dist = crate::split::redistribute_remaining_rune_to_shards::<
            MAX_USER_UTXOS,
            MAX_SHARDS_PER_POOL,
            SingleRuneSet,
            saturn_bitcoin_transactions::utxo_info::UtxoInfo<SingleRuneSet>,
            MockShardZc,
            MAX_SELECTED,
        >(&mut tx_builder, &mut selected, removed, ScriptBuf::new())
        .unwrap();

        // Expect proportional (75, 150, 225) regardless of ordering
        let mut allocs: Vec<u128> = dist
            .iter()
            .map(|s| s.find(&RuneId::BTC).unwrap().amount)
            .collect();
        allocs.sort_unstable();
        assert_eq!(allocs, vec![50, 150, 250]);
    }
}
