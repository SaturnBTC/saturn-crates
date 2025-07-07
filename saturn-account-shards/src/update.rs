//! Helpers to **safely mutate** a collection of [`StateShard`]s after a transaction has
//! been constructed and broadcast.
//!
//! The functions in this module are entirely *generic* over the concrete shard implementation –
//! they only need the `StateShard` trait – and can therefore be reused by any on-chain program
//! that follows the Saturn account-sharding pattern.

use arch_program::input_to_sign::InputToSign;
use arch_program::utxo::UtxoMeta;
use bitcoin::{ScriptBuf, Transaction};

#[cfg(feature = "runes")]
use arch_program::rune::{RuneAmount, RuneId};
#[cfg(feature = "runes")]
use ordinals::Runestone;

use saturn_bitcoin_transactions::utxo_info::UtxoInfoTrait;
use saturn_bitcoin_transactions::{fee_rate::FeeRate, TransactionBuilder};

#[cfg(feature = "utxo-consolidation")]
use saturn_bitcoin_transactions::utxo_info::FixedOptionF64;

#[cfg(feature = "runes")]
use saturn_collections::generic::fixed_set::FixedCapacitySet;

use crate::error::{Result, StateShardError};
use crate::shard::StateShard;

/// Removes all `utxos_to_remove` from the shards identified by `shard_indexes`.
///
/// This is an internal helper; it assumes that each entry in
/// `utxos_to_remove` **must** be present in every shard listed in
/// `shard_indexes` (either as a BTC-UTXO or the optional rune-UTXO) and will
/// silently ignore shards where the UTXO is missing – this is fine because the
/// outer logic only passes in shards that are actually affected.
fn remove_utxos_from_shards<
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    shards: &mut [&mut T],
    shard_indexes: &[usize],
    utxos_to_remove: &[UtxoMeta],
) {
    for utxo_to_remove in utxos_to_remove {
        shard_indexes.iter().for_each(|index| {
            let shard = &mut shards[*index];
            shard.btc_utxos_retain(&mut |utxo| utxo.meta() != utxo_to_remove);

            if let Some(rune_utxo) = shard.rune_utxo() {
                if rune_utxo.meta() == utxo_to_remove {
                    shard.clear_rune_utxo();
                }
            }
        });
    }
}

/// Selects the shard (by index into `shard_indexes`) with the **smallest total BTC value**.
fn select_best_shard_to_add_to_btc_to<
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    shards: &mut [&mut T],
    shard_indexes: &[usize],
) -> Option<usize> {
    shard_indexes
        .iter()
        // Consider only shards that still have spare capacity.
        .filter(|&&idx| shards[idx].btc_utxos_len() < shards[idx].btc_utxos_max_len())
        // Among those, pick the one with the smallest aggregate BTC value.
        .min_by_key(|&&idx| {
            shards[idx]
                .btc_utxos()
                .iter()
                .map(|u| u.value())
                .sum::<u64>()
        })
        .map(|&idx| idx)
}

/// Updates the UTXO sets of the provided shards.
fn update_shards_utxos<
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    shards: &mut [&mut T],
    shard_indexes: &[usize],
    utxos_to_remove: &[UtxoMeta],
    new_rune_utxos: Vec<U>,
    mut new_btc_utxos: Vec<U>,
    fee_rate: &FeeRate,
) -> Result<()> {
    // 1. Remove old UTXOs first.
    remove_utxos_from_shards(shards, shard_indexes, utxos_to_remove);

    // 2. Insert rune UTXOs where needed.
    let mut rune_utxo_iter = new_rune_utxos.into_iter();
    for &shard_index in shard_indexes {
        let shard = &mut shards[shard_index];
        if shard.rune_utxo().is_none() {
            if let Some(utxo) = rune_utxo_iter.next() {
                shard.set_rune_utxo(utxo);
            }
        }
    }

    // 3. Distribute BTC UTXOs – **largest first** – to the least funded shard.
    // Sort descending by value so that the first element is the largest one.
    new_btc_utxos.sort_by(|a, b| b.value().cmp(&a.value()));

    for mut utxo in new_btc_utxos.into_iter() {
        // Select a shard that has capacity and currently holds the least BTC.
        let target_idx = select_best_shard_to_add_to_btc_to(shards, shard_indexes)
            .ok_or(StateShardError::ShardsAreFullOfBtcUtxos)?;

        #[cfg(feature = "utxo-consolidation")]
        if shards[target_idx].btc_utxos_len() > 1 {
            *utxo.needs_consolidation_mut() = FixedOptionF64::some(fee_rate.0);
        }

        if shards[target_idx].add_btc_utxo(utxo).is_none() {
            // This should not happen because we checked capacity, but guard regardless.
            return Err(StateShardError::ShardsAreFullOfBtcUtxos);
        }
    }

    Ok(())
}

#[cfg(feature = "runes")]
fn update_modified_program_utxos_with_rune_amount<
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
>(
    new_program_outputs: &mut Vec<U>,
    runestone: &Runestone,
    prev_rune_amount: &mut RS,
) -> Result<Vec<U>> {
    let remaining_rune_amount = prev_rune_amount;
    let mut rune_utxos = vec![];

    for edict in &runestone.edicts {
        let rune_amount = edict.amount;
        let index = edict.output;
        let pos = new_program_outputs
            .iter()
            .position(|u| u.meta().vout() == index)
            .ok_or(StateShardError::OutputEdictIsNotInTransaction)?;

        let output = new_program_outputs
            .get_mut(pos)
            .ok_or(StateShardError::OutputEdictIsNotInTransaction)?;

        let rune_id = RuneId::new(edict.id.block, edict.id.tx);

        output.runes_mut().insert_or_modify::<StateShardError, _>(
            RuneAmount {
                id: rune_id,
                amount: rune_amount,
            },
            |rune_input| {
                rune_input.amount = rune_input
                    .amount
                    .checked_add(rune_amount)
                    .ok_or(StateShardError::RuneAmountAdditionOverflow)?;
                Ok(())
            },
        )?;

        if let Some(remaining_rune_amount) = remaining_rune_amount.find_mut(&rune_id) {
            remaining_rune_amount.amount = remaining_rune_amount
                .amount
                .checked_sub(rune_amount)
                .ok_or(StateShardError::NotEnoughRuneInShards)?;
        }
    }

    // Handle pointer logic for remainder
    if let Some(pointer_index) = runestone.pointer {
        for rune_amount in remaining_rune_amount.iter() {
            if rune_amount.amount > 0 {
                if let Some(output) = new_program_outputs
                    .iter_mut()
                    .find(|u| u.meta().vout() == pointer_index)
                {
                    output.runes_mut().insert_or_modify::<StateShardError, _>(
                        RuneAmount {
                            id: rune_amount.id,
                            amount: rune_amount.amount,
                        },
                        |rune_input| {
                            rune_input.amount =
                                rune_input
                                    .amount
                                    .checked_add(rune_amount.amount)
                                    .ok_or(StateShardError::RuneAmountAdditionOverflow)?;

                            Ok(())
                        },
                    )?;
                } else {
                    return Err(StateShardError::RunestonePointerIsNotInTransaction);
                }
            }
        }
    } else {
        // if any of the prev_rune amounts contain more than 0, return an error
        for rune_amount in remaining_rune_amount.iter() {
            if rune_amount.amount > 0 {
                return Err(StateShardError::RunestonePointerIsNotInTransaction);
            }
        }
    }

    // Extract outputs with runes by iterating backwards to avoid index shifting
    let mut i = new_program_outputs.len();
    while i > 0 {
        i -= 1;
        if new_program_outputs[i].runes().len() > 0 {
            // Move the UTXO out of the vector (zero-copy)
            let rune_utxo = new_program_outputs.swap_remove(i);
            rune_utxos.push(rune_utxo);
        }
    }

    // reverse the vector to maintain the original order of the rune utxos
    rune_utxos.reverse();

    Ok(rune_utxos)
}

/// Updates the provided `shards` to reflect the effects of a transaction that
/// has just been **broadcast and accepted**.
///
/// The function performs three high-level steps:
/// 1. Determine which program-owned UTXOs were **spent** and which new ones were
///    **created** by looking at the `TransactionBuilder` and the final
///    `transaction` that was signed.
/// 2. Split the newly created outputs into *plain BTC* vs *rune carrying*
///    outputs (the latter is only compiled in when the `runes` feature is
///    enabled).
/// 3. Call an internal balancing helper so that the new UTXOs are evenly
///    distributed across the shards involved in the call.
///
/// Only shards listed in `used_shard_indexes` are mutated which means callers
/// can safely pass in references to the *entire* shards array without having to
/// allocate a temporary slice.
///
/// # Errors
/// Returns `StateShardError::VecOverflow` when any shard's fixed-size UTXO
/// collection runs out of capacity while trying to insert a new BTC UTXO.
#[allow(clippy::too_many_arguments)]
pub fn update_shards_after_transaction<
    const MAX_USER_UTXOS: usize,
    const MAX_SHARDS_PER_POOL: usize,
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
    T: StateShard<U, RS>,
>(
    transaction_builder: &mut TransactionBuilder<MAX_USER_UTXOS, MAX_SHARDS_PER_POOL, RS>,
    shards: &mut [&mut T],
    used_shard_indexes: &[usize],
    program_script_pubkey: &ScriptBuf,
    fee_rate: &FeeRate,
) -> Result<()> {
    // 1. Determine which pool UTXOs have been spent and which new ones were created.
    let (utxos_to_remove, mut new_program_outputs) = get_modified_program_utxos_in_transaction(
        program_script_pubkey,
        &transaction_builder.transaction,
        transaction_builder.inputs_to_sign.as_slice(),
    );

    // 2. Extract rune-carrying outputs.
    let new_rune_utxos: Vec<U> = {
        #[cfg(feature = "runes")]
        {
            update_modified_program_utxos_with_rune_amount(
                &mut new_program_outputs,
                &transaction_builder.runestone,
                &mut transaction_builder.total_rune_inputs,
            )?
        }

        #[cfg(not(feature = "runes"))]
        {
            Vec::<U>::with_capacity(0)
        }
    };

    // 3. Finally mutate the shards.
    update_shards_utxos(
        shards,
        used_shard_indexes,
        &utxos_to_remove,
        new_rune_utxos,
        new_program_outputs,
        fee_rate,
    )
}

/// Helper used by `update_shards_after_transaction` to extract modified program UTXOs from a transaction.
fn get_modified_program_utxos_in_transaction<
    RS: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RS>,
>(
    program_script_pubkey: &ScriptBuf,
    transaction: &Transaction,
    inputs_to_sign: &[InputToSign],
) -> (Vec<UtxoMeta>, Vec<U>) {
    let mut utxos_to_remove = Vec::with_capacity(inputs_to_sign.len());
    let mut program_outputs = Vec::with_capacity(transaction.output.len() / 2);

    let txid_bytes =
        saturn_bitcoin_transactions::bytes::txid_to_bytes_big_endian(&transaction.compute_txid());

    // Process inputs
    for input in inputs_to_sign {
        let outpoint = transaction.input[input.index as usize].previous_output;
        utxos_to_remove.push(UtxoMeta::from(
            saturn_bitcoin_transactions::bytes::txid_to_bytes_big_endian(&outpoint.txid),
            outpoint.vout,
        ));
    }

    // Process outputs
    for (index, output) in transaction.output.iter().enumerate() {
        if output.script_pubkey == *program_script_pubkey {
            program_outputs.push(U::new(
                UtxoMeta::from(txid_bytes, index as u32),
                output.value.to_sat(),
            ));
        }
    }

    (utxos_to_remove, program_outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    use bitcoin::absolute::LockTime;
    use bitcoin::hashes::Hash;
    use bitcoin::transaction::Version;
    use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness};
    use ordinals::{Edict, Runestone};
    use saturn_bitcoin_transactions::utxo_info::UtxoInfo;

    #[allow(unused_macros)]
    macro_rules! new_tb {
        ($max_mod:expr, $max_inputs:expr) => {{
            #[cfg(feature = "runes")]
            {
                TransactionBuilder::<
                    $max_mod,
                    $max_inputs,
                    saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
                >::new()
            }
            #[cfg(not(feature = "runes"))]
            {
                TransactionBuilder::<
                    $max_mod,
                    $max_inputs,
                    saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
                >::new()
            }
        }};
    }

    /// Simple in-memory implementation of [`StateShard`] for unit testing.
    #[derive(Default, Clone, Debug)]
    struct TestShard {
        btc_utxos: Vec<UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>>,
        rune_utxo: Option<UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>>,
    }

    impl
        StateShard<
            UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>,
            saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
        > for TestShard
    {
        fn btc_utxos(&self) -> &[UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>] {
            &self.btc_utxos
        }

        fn btc_utxos_mut(
            &mut self,
        ) -> &mut [UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>] {
            &mut self.btc_utxos
        }

        fn btc_utxos_max_len(&self) -> usize {
            16 // match the cap used in `add_btc_utxo`
        }

        fn btc_utxos_retain(
            &mut self,
            f: &mut dyn FnMut(
                &UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>,
            ) -> bool,
        ) {
            self.btc_utxos.retain(|u| f(u));
        }

        fn add_btc_utxo(
            &mut self,
            utxo: UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>,
        ) -> Option<usize> {
            const MAX: usize = 16; // arbitrary bound for testing
            if self.btc_utxos.len() >= MAX {
                return None;
            }
            self.btc_utxos.push(utxo);
            Some(self.btc_utxos.len() - 1)
        }

        fn btc_utxos_len(&self) -> usize {
            self.btc_utxos.len()
        }

        fn rune_utxo(
            &self,
        ) -> Option<&UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>> {
            self.rune_utxo.as_ref()
        }

        fn rune_utxo_mut(
            &mut self,
        ) -> Option<&mut UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>> {
            self.rune_utxo.as_mut()
        }

        fn clear_rune_utxo(&mut self) {
            self.rune_utxo = None;
        }

        fn set_rune_utxo(
            &mut self,
            utxo: UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet>,
        ) {
            self.rune_utxo = Some(utxo);
        }
    }

    // === Shared helpers ====================================================
    fn create_utxo(
        value: u64,
        txid_byte: u8,
        vout: u32,
    ) -> UtxoInfo<saturn_bitcoin_transactions::utxo_info::SingleRuneSet> {
        let txid = [txid_byte; 32];
        UtxoInfo {
            meta: UtxoMeta::from(txid, vout),
            value,
            ..Default::default()
        }
    }

    fn fee_rate() -> FeeRate {
        FeeRate(1.0)
    }

    // ---------------------------------------------------------------------
    mod select_best_shard_to_add_to_btc_to {
        use super::*;

        #[test]
        fn selects_shard_with_smallest_total_btc() {
            let shard_low = TestShard {
                btc_utxos: vec![create_utxo(50, 1, 0)],
                rune_utxo: None,
            };
            let shard_medium = TestShard {
                btc_utxos: vec![create_utxo(100, 2, 0)],
                rune_utxo: None,
            };
            let shard_high = TestShard {
                btc_utxos: vec![create_utxo(200, 3, 0)],
                rune_utxo: None,
            };

            let mut shards_storage = vec![shard_medium, shard_low, shard_high];
            let mut shards: Vec<&mut TestShard> = shards_storage.iter_mut().collect();

            // indexes 0,1,2 correspond to medium, low, high
            let shard_indexes = [0_usize, 1_usize, 2_usize];
            let best = super::select_best_shard_to_add_to_btc_to(&mut shards, &shard_indexes);
            assert_eq!(best, Some(1)); // shard_low

            // tie-breaker: two equal totals – selects the first in order (idx 0)
            let mut shards_storage = vec![
                TestShard {
                    btc_utxos: vec![create_utxo(100, 4, 0)],
                    rune_utxo: None,
                },
                TestShard {
                    btc_utxos: vec![create_utxo(100, 5, 0)],
                    rune_utxo: None,
                },
            ];
            let mut shards: Vec<&mut TestShard> = shards_storage.iter_mut().collect();
            let idxs = [0_usize, 1_usize];
            assert_eq!(
                super::select_best_shard_to_add_to_btc_to(&mut shards, &idxs),
                Some(0)
            );
        }

        #[test]
        fn returns_none_when_all_shards_are_full() {
            // Fill both shards to capacity
            let mut shard0 = TestShard::default();
            let mut shard1 = TestShard::default();
            for i in 0..16 {
                shard0.btc_utxos.push(create_utxo(1, 100, i));
                shard1.btc_utxos.push(create_utxo(1, 101, i));
            }

            let mut shards_storage = vec![shard0, shard1];
            let mut shards: Vec<&mut TestShard> = shards_storage.iter_mut().collect();
            let shard_indexes = [0_usize, 1_usize];

            let result = super::select_best_shard_to_add_to_btc_to(&mut shards, &shard_indexes);
            assert_eq!(result, None);
        }

        #[test]
        fn skips_full_shards_and_selects_available_one() {
            // shard0 is full, shard1 has capacity
            let mut shard0 = TestShard::default();
            for i in 0..16 {
                shard0.btc_utxos.push(create_utxo(1, 110, i));
            }
            let shard1 = TestShard {
                btc_utxos: vec![create_utxo(500, 111, 0)],
                rune_utxo: None,
            };

            let mut shards_storage = vec![shard0, shard1];
            let mut shards: Vec<&mut TestShard> = shards_storage.iter_mut().collect();
            let shard_indexes = [0_usize, 1_usize];

            let result = super::select_best_shard_to_add_to_btc_to(&mut shards, &shard_indexes);
            assert_eq!(result, Some(1)); // Only shard1 has capacity
        }
    }

    // ---------------------------------------------------------------------
    mod update_shards_utxos {
        use super::*;

        #[test]
        fn distributes_new_utxos_and_handles_runes() {
            // Initial shards (empty)
            let mut shard0 = TestShard::default();
            let mut shard1 = TestShard::default();
            let shard_indexes = [0_usize, 1_usize];

            // New rune UTXO – only one, should go to shard0
            let new_rune_utxo = create_utxo(546, 10, 0);
            // New BTC UTXOs – 200 and 100 sats
            let new_btc_utxo_big = create_utxo(200, 11, 0);
            let new_btc_utxo_small = create_utxo(100, 12, 0);

            let mut shards: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];

            let result = super::update_shards_utxos(
                &mut shards,
                &shard_indexes,
                &[], // nothing to remove
                vec![new_rune_utxo.clone()],
                vec![new_btc_utxo_big.clone(), new_btc_utxo_small.clone()],
                &fee_rate(),
            );
            assert!(result.is_ok());

            // shard0 gets rune utxo and the larger btc utxo (picked first)
            assert!(shard0.rune_utxo().is_some());
            assert_eq!(shard0.btc_utxos.len(), 1);
            assert_eq!(shard0.btc_utxos[0], new_btc_utxo_big);

            // shard1 remains without rune utxo and receives the smaller btc
            assert!(shard1.rune_utxo().is_none());
            assert_eq!(shard1.btc_utxos.len(), 1);
            assert_eq!(shard1.btc_utxos[0], new_btc_utxo_small);
        }

        #[test]
        fn skips_inserting_rune_when_already_present() {
            let existing_rune_utxo = create_utxo(546, 30, 0);
            let mut shard0 = TestShard {
                btc_utxos: vec![],
                rune_utxo: Some(existing_rune_utxo.clone()),
            };
            let mut shard1 = TestShard::default();
            let mut shards: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];

            let res = super::update_shards_utxos(
                &mut shards,
                &[0_usize, 1_usize],
                &[],
                vec![create_utxo(546, 31, 0)], // new rune UTXO
                vec![],
                &fee_rate(),
            );
            assert!(res.is_ok());

            // shard0 keeps its original rune; shard1 receives the new one
            assert_eq!(shard0.rune_utxo().unwrap(), &existing_rune_utxo);
            assert!(shard1.rune_utxo().is_some());
        }

        #[test]
        fn errors_when_btc_utxo_vector_overflows() {
            // Fill **both** shards to capacity so the insertion has no room to succeed.
            let mut shard0 = TestShard::default();
            for i in 0..16 {
                shard0.btc_utxos.push(create_utxo(1, 70, i));
            }

            let mut shard1 = TestShard::default();
            for i in 0..16 {
                shard1.btc_utxos.push(create_utxo(1, 72, i));
            }

            let mut shards: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];

            let err = super::update_shards_utxos(
                &mut shards,
                &[0_usize, 1_usize],
                &[],
                vec![],
                vec![create_utxo(1, 71, 0)], // additional BTC UTXO triggers overflow
                &fee_rate(),
            )
            .unwrap_err();

            assert_eq!(err, StateShardError::ShardsAreFullOfBtcUtxos);
        }

        #[test]
        fn succeeds_after_removal_creates_capacity() {
            // Start with shard0 at capacity
            let utxo_to_remove = create_utxo(100, 120, 0);
            let mut shard0 = TestShard::default();
            for i in 0..15 {
                shard0.btc_utxos.push(create_utxo(1, 121, i));
            }
            shard0.btc_utxos.push(utxo_to_remove.clone());
            assert_eq!(shard0.btc_utxos.len(), 16); // at capacity

            let mut shard1 = TestShard::default();
            let mut shards: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];

            let new_utxo = create_utxo(200, 122, 0);

            let result = super::update_shards_utxos(
                &mut shards,
                &[0_usize, 1_usize],
                &[utxo_to_remove.meta], // remove one from shard0
                vec![],
                vec![new_utxo.clone()],
                &fee_rate(),
            );

            assert!(result.is_ok());
            // shard0 should now have 15 old UTXOs (after removal)
            assert_eq!(shard0.btc_utxos.len(), 15);
            assert!(!shard0.btc_utxos.iter().any(|u| u == &utxo_to_remove));
            // The new UTXO should go to shard1 since it has smaller total value (0) after removal
            assert_eq!(shard1.btc_utxos.len(), 1);
            assert!(shard1.btc_utxos.iter().any(|u| u == &new_utxo));
        }

        #[test]
        fn replaces_rune_utxo_correctly() {
            let old_rune = create_utxo(546, 130, 0);
            let new_rune = create_utxo(546, 131, 0);

            let mut shard0 = TestShard {
                btc_utxos: vec![],
                rune_utxo: Some(old_rune.clone()),
            };
            let mut shard1 = TestShard::default();
            let mut shards: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];

            let result = super::update_shards_utxos(
                &mut shards,
                &[0_usize, 1_usize],
                &[old_rune.meta],       // remove old rune
                vec![new_rune.clone()], // add new rune
                vec![],
                &fee_rate(),
            );

            assert!(result.is_ok());
            // shard0 should have the new rune (it was cleared then refilled)
            assert_eq!(shard0.rune_utxo().unwrap(), &new_rune);
            assert!(shard1.rune_utxo().is_none());
        }

        #[test]
        fn handles_no_new_runes_when_shards_have_none() {
            let mut shard0 = TestShard::default();
            let mut shard1 = TestShard::default();
            let mut shards: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];

            let result = super::update_shards_utxos(
                &mut shards,
                &[0_usize, 1_usize],
                &[],
                vec![], // no new runes
                vec![create_utxo(1000, 140, 0)],
                &fee_rate(),
            );

            assert!(result.is_ok());
            assert!(shard0.rune_utxo().is_none());
            assert!(shard1.rune_utxo().is_none());
        }

        #[cfg(feature = "utxo-consolidation")]
        #[test]
        fn sets_needs_consolidation_flag_when_applicable() {
            // shard0 has >1 UTXO but tiny total so it will receive the new one.
            let mut shard0 = TestShard {
                btc_utxos: vec![create_utxo(1, 80, 0), create_utxo(1, 81, 0)],
                rune_utxo: None,
            };
            let mut shard1 = TestShard {
                btc_utxos: vec![create_utxo(100, 82, 0)],
                rune_utxo: None,
            };

            let new_utxo = create_utxo(5, 83, 0);

            let mut shards: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];

            let _ = super::update_shards_utxos(
                &mut shards,
                &[0_usize, 1_usize],
                &[],
                vec![],
                vec![new_utxo],
                &fee_rate(),
            )
            .unwrap();

            // The last element in shard0 should be the inserted utxo with consolidation flag.
            let inserted = shard0.btc_utxos.last().unwrap();
            assert!(inserted.needs_consolidation.is_some());
            assert_eq!(inserted.needs_consolidation.get().unwrap(), fee_rate().0);
        }

        #[cfg(feature = "utxo-consolidation")]
        #[test]
        fn does_not_set_consolidation_flag_when_shard_has_one_or_zero_utxos() {
            // shard0 has only 1 UTXO, shard1 is empty
            let mut shard0 = TestShard {
                btc_utxos: vec![create_utxo(50, 150, 0)],
                rune_utxo: None,
            };
            let mut shard1 = TestShard::default();

            let new_utxo = create_utxo(10, 151, 0);
            let mut shards: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];

            let _ = super::update_shards_utxos(
                &mut shards,
                &[0_usize, 1_usize],
                &[],
                vec![],
                vec![new_utxo],
                &fee_rate(),
            )
            .unwrap();

            // The new UTXO should go to shard1 (empty, smaller total)
            assert_eq!(shard1.btc_utxos.len(), 1);
            let inserted = &shard1.btc_utxos[0];
            // Should NOT have consolidation flag since shard1 had ≤1 UTXO before insertion
            assert!(inserted.needs_consolidation.is_none());
        }
    }

    // ---------------------------------------------------------------------
    mod remove_utxos_from_shards {
        use super::*;

        #[test]
        fn removes_btc_and_rune_utxos_across_shards() {
            let utxo_to_remove = create_utxo(1_000, 42, 0);
            let other_utxo = create_utxo(2_000, 43, 1);

            let mut shard_a = TestShard {
                btc_utxos: vec![utxo_to_remove.clone(), other_utxo.clone()],
                rune_utxo: Some(utxo_to_remove.clone()),
            };
            let mut shard_b = TestShard {
                btc_utxos: vec![other_utxo.clone(), utxo_to_remove.clone()],
                rune_utxo: Some(utxo_to_remove.clone()),
            };

            let mut shards: Vec<&mut TestShard> = vec![&mut shard_a, &mut shard_b];
            let shard_indexes = [0_usize, 1_usize];
            let metas = vec![utxo_to_remove.meta];

            super::remove_utxos_from_shards(&mut shards, &shard_indexes, &metas);

            for shard in shards {
                assert!(!shard.btc_utxos.iter().any(|u| u == &utxo_to_remove));
                assert!(shard.rune_utxo.is_none());
            }
        }

        #[test]
        fn ignores_utxo_missing_in_some_shards() {
            let utxo_to_remove = create_utxo(1_000, 50, 0);
            let other_utxo = create_utxo(500, 51, 0);

            // shard_a contains the UTXO, shard_b does not.
            let mut shard_a = TestShard {
                btc_utxos: vec![utxo_to_remove.clone(), other_utxo.clone()],
                rune_utxo: None,
            };
            let mut shard_b = TestShard {
                btc_utxos: vec![other_utxo.clone()],
                rune_utxo: None,
            };

            let mut shards: Vec<&mut TestShard> = vec![&mut shard_a, &mut shard_b];
            let shard_indexes = [0_usize, 1_usize];

            super::remove_utxos_from_shards(&mut shards, &shard_indexes, &[utxo_to_remove.meta]);

            // Only shard_a should have removed the UTXO.
            assert_eq!(shard_a.btc_utxos.len(), 1);
            assert_eq!(shard_b.btc_utxos.len(), 1);
        }

        #[test]
        fn works_when_shard_has_no_rune_utxo() {
            let utxo_to_remove = create_utxo(1_000, 60, 0);

            let mut shard = TestShard {
                btc_utxos: vec![utxo_to_remove.clone()],
                rune_utxo: None, // No rune UTXO present.
            };

            let mut shards: Vec<&mut TestShard> = vec![&mut shard];
            let shard_indexes = [0_usize];

            super::remove_utxos_from_shards(&mut shards, &shard_indexes, &[utxo_to_remove.meta]);

            assert!(shard.btc_utxos.is_empty());
            assert!(shard.rune_utxo.is_none());
        }

        #[test]
        fn removes_multiple_utxos_from_multiple_shards() {
            let utxo1 = create_utxo(1000, 160, 0);
            let utxo2 = create_utxo(2000, 161, 0);
            let keeper = create_utxo(500, 162, 0);

            // shard0 has both UTXOs to remove + keeper
            let mut shard0 = TestShard {
                btc_utxos: vec![utxo1.clone(), keeper.clone(), utxo2.clone()],
                rune_utxo: Some(utxo1.clone()),
            };
            // shard1 has only utxo2 + keeper
            let mut shard1 = TestShard {
                btc_utxos: vec![keeper.clone(), utxo2.clone()],
                rune_utxo: Some(utxo2.clone()),
            };

            let mut shards: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];
            let shard_indexes = [0_usize, 1_usize];

            super::remove_utxos_from_shards(&mut shards, &shard_indexes, &[utxo1.meta, utxo2.meta]);

            // shard0 should only have keeper left, no rune
            assert_eq!(shard0.btc_utxos.len(), 1);
            assert_eq!(shard0.btc_utxos[0], keeper);
            assert!(shard0.rune_utxo.is_none());

            // shard1 should only have keeper left, no rune
            assert_eq!(shard1.btc_utxos.len(), 1);
            assert_eq!(shard1.btc_utxos[0], keeper);
            assert!(shard1.rune_utxo.is_none());
        }

        #[test]
        fn handles_empty_utxos_to_remove() {
            let keeper = create_utxo(1000, 170, 0);
            let mut shard = TestShard {
                btc_utxos: vec![keeper.clone()],
                rune_utxo: Some(keeper.clone()),
            };

            let mut shards: Vec<&mut TestShard> = vec![&mut shard];
            let shard_indexes = [0_usize];

            super::remove_utxos_from_shards(&mut shards, &shard_indexes, &[]);

            // Nothing should change
            assert_eq!(shard.btc_utxos.len(), 1);
            assert_eq!(shard.btc_utxos[0], keeper);
            assert_eq!(shard.rune_utxo.as_ref().unwrap(), &keeper);
        }
    }

    // ---------------------------------------------------------------------
    mod get_modified_program_utxos_in_transaction {
        use super::*;

        #[test]
        fn identifies_program_outputs_correctly() {
            let script = ScriptBuf::new(); // empty script used for program

            // Build a simple transaction with one output matching the program script
            let tx = Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: OutPoint::null(),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::default(),
                }],
                output: vec![TxOut {
                    value: Amount::from_sat(1_000),
                    script_pubkey: script.clone(),
                }],
            };

            // One InputToSign at index 0
            let inputs_to_sign = vec![InputToSign {
                index: 0,
                signer: arch_program::pubkey::Pubkey::default(),
            }];

            let (removed, added): (
                Vec<UtxoMeta>,
                Vec<
                    saturn_bitcoin_transactions::utxo_info::UtxoInfo<
                        saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
                    >,
                >,
            ) = super::get_modified_program_utxos_in_transaction(&script, &tx, &inputs_to_sign);

            assert_eq!(removed.len(), 1);
            assert_eq!(added.len(), 1);
            assert_eq!(added[0].value, 1_000);

            // Provide a second non-program output and ensure still only 1 program UTXO detected.
            let tx2 = Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: OutPoint::null(),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::default(),
                }],
                output: vec![
                    TxOut {
                        value: Amount::from_sat(1_000),
                        script_pubkey: script.clone(),
                    },
                    TxOut {
                        value: Amount::from_sat(2_000),
                        script_pubkey: ScriptBuf::from_bytes(vec![0x51]), // OP_TRUE
                    },
                ],
            };

            let (rm, add): (
                Vec<UtxoMeta>,
                Vec<
                    saturn_bitcoin_transactions::utxo_info::UtxoInfo<
                        saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
                    >,
                >,
            ) = super::get_modified_program_utxos_in_transaction(&script, &tx2, &inputs_to_sign);

            assert_eq!(rm.len(), 1);
            assert_eq!(add.len(), 1);
        }

        #[test]
        fn handles_multiple_inputs_to_sign() {
            let script = ScriptBuf::new();

            let outpoint1 = OutPoint {
                txid: bitcoin::Txid::from_slice(&[1; 32]).unwrap(),
                vout: 0,
            };
            let outpoint2 = OutPoint {
                txid: bitcoin::Txid::from_slice(&[2; 32]).unwrap(),
                vout: 1,
            };

            let tx = Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: vec![
                    TxIn {
                        previous_output: outpoint1,
                        script_sig: ScriptBuf::new(),
                        sequence: Sequence::MAX,
                        witness: Witness::default(),
                    },
                    TxIn {
                        previous_output: outpoint2,
                        script_sig: ScriptBuf::new(),
                        sequence: Sequence::MAX,
                        witness: Witness::default(),
                    },
                ],
                output: vec![],
            };

            let inputs_to_sign = vec![
                InputToSign {
                    index: 0,
                    signer: arch_program::pubkey::Pubkey::default(),
                },
                InputToSign {
                    index: 1,
                    signer: arch_program::pubkey::Pubkey::default(),
                },
            ];

            let (removed, _added): (
                Vec<UtxoMeta>,
                Vec<
                    saturn_bitcoin_transactions::utxo_info::UtxoInfo<
                        saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
                    >,
                >,
            ) = super::get_modified_program_utxos_in_transaction(&script, &tx, &inputs_to_sign);

            assert_eq!(removed.len(), 2);
            // Verify both outpoints are captured
            assert!(removed.iter().any(|meta| meta.vout() == 0));
            assert!(removed.iter().any(|meta| meta.vout() == 1));
        }

        #[test]
        fn handles_multiple_program_outputs() {
            let script = ScriptBuf::new();

            let tx = Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: vec![],
                output: vec![
                    TxOut {
                        value: Amount::from_sat(1_000),
                        script_pubkey: script.clone(),
                    },
                    TxOut {
                        value: Amount::from_sat(2_000),
                        script_pubkey: ScriptBuf::from_bytes(vec![0x51]), // non-program
                    },
                    TxOut {
                        value: Amount::from_sat(3_000),
                        script_pubkey: script.clone(),
                    },
                ],
            };

            let (_removed, added): (
                Vec<UtxoMeta>,
                Vec<
                    saturn_bitcoin_transactions::utxo_info::UtxoInfo<
                        saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
                    >,
                >,
            ) = super::get_modified_program_utxos_in_transaction(&script, &tx, &[]);

            assert_eq!(added.len(), 2);
            assert_eq!(added[0].value, 1_000);
            assert_eq!(added[0].meta.vout(), 0);
            assert_eq!(added[1].value, 3_000);
            assert_eq!(added[1].meta.vout(), 2);
        }
    }

    // ---------------------------------------------------------------------
    mod update_shards_after_transaction {
        use super::*;

        #[test]
        fn integrates_all_helpers() {
            // Prepare builder with single program output
            let mut builder: TransactionBuilder<
                4,
                4,
                saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
            > = new_tb!(4, 4);
            let program_script = ScriptBuf::new();
            builder.transaction = Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: vec![],
                output: vec![TxOut {
                    value: Amount::from_sat(10_000),
                    script_pubkey: program_script.clone(),
                }],
            };

            // Prepare shards
            let mut shard0 = TestShard::default();
            let mut shard1 = TestShard::default();
            let mut shards_vec: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];
            let used_indexes = [0_usize, 1_usize];

            let res = super::update_shards_after_transaction(
                &mut builder,
                &mut shards_vec,
                &used_indexes,
                &program_script,
                &fee_rate(),
            );
            assert!(res.is_ok());

            // One BTC utxo should have been assigned to shard0 (smallest total)
            assert_eq!(shard0.btc_utxos.len(), 1);
            assert_eq!(shard0.btc_utxos[0].value, 10_000);
            assert_eq!(shard1.btc_utxos.len(), 0);
        }

        #[test]
        fn handles_spending_and_creating_utxos() {
            let program_script = ScriptBuf::new();
            let existing_utxo = create_utxo(5_000, 200, 0);

            // Set up initial state: shard0 has an existing UTXO
            let mut shard0 = TestShard {
                btc_utxos: vec![existing_utxo.clone()],
                rune_utxo: None,
            };
            let mut shard1 = TestShard::default();

            // Create a transaction that spends the existing UTXO and creates a new one
            let mut builder = new_tb!(4, 4);

            let input_outpoint = OutPoint {
                txid: bitcoin::Txid::from_slice(&[200; 32]).unwrap(),
                vout: 0,
            };

            builder.transaction = Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: input_outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::default(),
                }],
                output: vec![TxOut {
                    value: Amount::from_sat(4_500), // less due to fees
                    script_pubkey: program_script.clone(),
                }],
            };

            builder
                .inputs_to_sign
                .push(InputToSign {
                    index: 0,
                    signer: arch_program::pubkey::Pubkey::default(),
                })
                .unwrap();

            let mut shards_vec: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];
            let used_indexes = [0_usize, 1_usize];

            let res = super::update_shards_after_transaction(
                &mut builder,
                &mut shards_vec,
                &used_indexes,
                &program_script,
                &fee_rate(),
            );
            assert!(res.is_ok());

            // The old UTXO should be removed and new one added
            assert!(!shard0.btc_utxos.iter().any(|u| u == &existing_utxo));

            // One of the shards should have the new UTXO
            let total_utxos = shard0.btc_utxos.len() + shard1.btc_utxos.len();
            assert_eq!(total_utxos, 1);

            let new_utxo = if !shard0.btc_utxos.is_empty() {
                &shard0.btc_utxos[0]
            } else {
                &shard1.btc_utxos[0]
            };
            assert_eq!(new_utxo.value, 4_500);
        }

        #[cfg(feature = "runes")]
        #[test]
        fn handles_rune_utxo_spending_and_creation() {
            let program_script = ScriptBuf::new();
            let existing_rune_utxo = create_utxo(546, 210, 0);

            // Set up initial state: shard0 has a rune UTXO
            let mut shard0 = TestShard {
                btc_utxos: vec![],
                rune_utxo: Some(existing_rune_utxo.clone()),
            };
            let mut shard1 = TestShard::default();

            // Create a transaction that spends the rune UTXO and creates new ones
            let mut builder: TransactionBuilder<
                4,
                4,
                saturn_bitcoin_transactions::utxo_info::SingleRuneSet,
            > = new_tb!(4, 4);

            builder
                .total_rune_inputs
                .insert(arch_program::rune::RuneAmount {
                    id: arch_program::rune::RuneId::new(1, 0),
                    amount: 100,
                })
                .unwrap();

            let input_outpoint = OutPoint {
                txid: bitcoin::Txid::from_slice(&[210; 32]).unwrap(),
                vout: 0,
            };

            builder.transaction = Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: input_outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::default(),
                }],
                output: vec![
                    TxOut {
                        value: Amount::from_sat(546),
                        script_pubkey: program_script.clone(),
                    },
                    TxOut {
                        value: Amount::from_sat(546),
                        script_pubkey: program_script.clone(),
                    },
                ],
            };

            builder
                .inputs_to_sign
                .push(InputToSign {
                    index: 0,
                    signer: arch_program::pubkey::Pubkey::default(),
                })
                .unwrap();

            // Set up runestone to distribute runes
            builder.runestone = Runestone {
                pointer: Some(1),
                edicts: vec![Edict {
                    id: ordinals::RuneId { block: 1, tx: 0 },
                    amount: 60,
                    output: 0,
                }],
                ..Default::default()
            };

            let mut shards_vec: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];
            let used_indexes = [0_usize, 1_usize];

            let res = super::update_shards_after_transaction(
                &mut builder,
                &mut shards_vec,
                &used_indexes,
                &program_script,
                &fee_rate(),
            );
            assert!(res.is_ok());

            // The old rune UTXO should be removed
            assert!(
                shard0.rune_utxo().is_none() || shard0.rune_utxo().unwrap() != &existing_rune_utxo
            );

            // At least one shard should have a new rune UTXO
            assert!(shard0.rune_utxo().is_some() || shard1.rune_utxo().is_some());
        }

        #[test]
        fn propagates_overflow_error_when_all_shards_full() {
            let program_script = ScriptBuf::new();

            // Fill both shards to capacity
            let mut shard0 = TestShard::default();
            let mut shard1 = TestShard::default();
            for i in 0..16 {
                shard0.btc_utxos.push(create_utxo(1, 220, i));
                shard1.btc_utxos.push(create_utxo(1, 221, i));
            }

            let mut builder = new_tb!(4, 4);
            builder.transaction = Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: vec![],
                output: vec![TxOut {
                    value: Amount::from_sat(1_000),
                    script_pubkey: program_script.clone(),
                }],
            };

            let mut shards_vec: Vec<&mut TestShard> = vec![&mut shard0, &mut shard1];
            let used_indexes = [0_usize, 1_usize];

            let err = super::update_shards_after_transaction(
                &mut builder,
                &mut shards_vec,
                &used_indexes,
                &program_script,
                &fee_rate(),
            )
            .unwrap_err();

            assert_eq!(err, StateShardError::ShardsAreFullOfBtcUtxos);
        }
    }

    // ---------------------------------------------------------------------
    #[cfg(feature = "runes")]
    mod update_modified_program_utxos_with_rune_amount {
        use saturn_collections::generic::fixed_set::FixedSet;

        use super::*;

        // Helper type & function for tests that require more than one rune per UTXO.
        type RuneSet3 = FixedSet<arch_program::rune::RuneAmount, 3>;
        fn create_utxo_rs3(
            value: u64,
            txid_byte: u8,
            vout: u32,
        ) -> saturn_bitcoin_transactions::utxo_info::UtxoInfo<RuneSet3> {
            let txid = [txid_byte; 32];
            saturn_bitcoin_transactions::utxo_info::UtxoInfo::<RuneSet3>::new(
                arch_program::utxo::UtxoMeta::from(txid, vout),
                value,
            )
        }

        #[test]
        fn splits_rune_outputs_and_assigns_remainder() {
            // Prepare three program outputs with distinct vout indices
            let mut outputs = vec![
                create_utxo_rs3(546, 20, 0),
                create_utxo_rs3(546, 20, 1),
                create_utxo_rs3(546, 20, 2),
            ];

            // Create a runestone with one explicit edict for vout 1 and pointer to vout 2
            let edict_amount: u128 = 50;
            let remaining: u128 = 50;
            let runestone = Runestone {
                pointer: Some(2),
                edicts: vec![Edict {
                    id: ordinals::RuneId { block: 1, tx: 1 },
                    amount: edict_amount,
                    output: 1,
                }],
                ..Default::default()
            };

            let mut prev_runes: RuneSet3 = RuneSet3::default();
            prev_runes
                .insert(arch_program::rune::RuneAmount {
                    id: arch_program::rune::RuneId::new(1, 1),
                    amount: edict_amount + remaining,
                })
                .unwrap();

            let rune_utxos = super::update_modified_program_utxos_with_rune_amount(
                &mut outputs,
                &runestone,
                &mut prev_runes,
            )
            .expect("update should succeed");

            // Should have extracted 2 rune UTXOs (one for edict, one remainder)
            assert_eq!(rune_utxos.len(), 2);
            // outputs vector should now only contain the untouched vout 0
            assert_eq!(outputs.len(), 1);

            // Validate amounts
            let total_extracted: u128 = rune_utxos
                .iter()
                .map(|u| u.runes.get().unwrap().amount)
                .sum();
            assert_eq!(total_extracted, edict_amount + remaining);
        }

        #[test]
        fn handles_zero_remainder() {
            let mut outputs = vec![create_utxo_rs3(546, 180, 0), create_utxo_rs3(546, 180, 1)];

            let edict_amount: u128 = 100;
            let runestone = Runestone {
                pointer: Some(1), // pointer exists but won't be used
                edicts: vec![Edict {
                    id: ordinals::RuneId { block: 1, tx: 0 },
                    amount: edict_amount,
                    output: 0,
                }],
                ..Default::default()
            };

            let mut prev_runes = RuneSet3::default();
            prev_runes
                .insert(arch_program::rune::RuneAmount {
                    id: arch_program::rune::RuneId::new(1, 0),
                    amount: edict_amount,
                })
                .unwrap();

            let rune_utxos = super::update_modified_program_utxos_with_rune_amount(
                &mut outputs,
                &runestone,
                &mut prev_runes,
            )
            .expect("update should succeed");

            // Should have extracted only 1 rune UTXO (no remainder)
            assert_eq!(rune_utxos.len(), 1);
            assert_eq!(rune_utxos[0].runes.get().unwrap().amount, edict_amount);
            // outputs vector should contain the untouched vout 1
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].meta.vout(), 1);
        }

        #[test]
        fn handles_multiple_edicts() {
            let mut outputs = vec![
                create_utxo_rs3(546, 190, 0),
                create_utxo_rs3(546, 190, 1),
                create_utxo_rs3(546, 190, 2),
                create_utxo_rs3(546, 190, 3),
            ];

            let edict1_amount: u128 = 30;
            let edict2_amount: u128 = 20;
            let remainder: u128 = 50;
            let total = edict1_amount + edict2_amount + remainder;

            let runestone = Runestone {
                pointer: Some(3),
                edicts: vec![
                    Edict {
                        id: ordinals::RuneId { block: 1, tx: 0 },
                        amount: edict1_amount,
                        output: 0,
                    },
                    Edict {
                        id: ordinals::RuneId { block: 2, tx: 0 },
                        amount: edict2_amount,
                        output: 1,
                    },
                ],
                ..Default::default()
            };

            let mut prev_runes: RuneSet3 = RuneSet3::default();

            prev_runes
                .insert(arch_program::rune::RuneAmount {
                    id: arch_program::rune::RuneId::new(1, 0),
                    amount: edict1_amount + remainder,
                })
                .unwrap();
            prev_runes
                .insert(arch_program::rune::RuneAmount {
                    id: arch_program::rune::RuneId::new(2, 0),
                    amount: edict2_amount,
                })
                .unwrap();

            let rune_utxos = super::update_modified_program_utxos_with_rune_amount(
                &mut outputs,
                &runestone,
                &mut prev_runes,
            )
            .expect("update should succeed");

            // Should have extracted 3 rune UTXOs (2 edicts + 1 remainder)
            assert_eq!(rune_utxos.len(), 3);

            let total_extracted: u128 = rune_utxos
                .iter()
                .map(|u| u.runes.get().unwrap().amount)
                .sum();
            assert_eq!(total_extracted, total);

            // outputs vector should contain only the untouched vout 2
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].meta.vout(), 2);
        }

        #[test]
        fn error_when_edict_output_missing() {
            let mut outputs = vec![create_utxo_rs3(546, 40, 0)];

            let runestone = Runestone {
                pointer: Some(0),
                edicts: vec![Edict {
                    id: ordinals::RuneId { block: 1, tx: 0 },
                    amount: 10,
                    output: 1, // non-existent
                }],
                ..Default::default()
            };

            let mut prev_runes = RuneSet3::default();
            prev_runes
                .insert(arch_program::rune::RuneAmount {
                    id: arch_program::rune::RuneId::new(0, 0),
                    amount: 10,
                })
                .unwrap();

            let err = super::update_modified_program_utxos_with_rune_amount(
                &mut outputs,
                &runestone,
                &mut prev_runes,
            )
            .unwrap_err();
            assert_eq!(err, StateShardError::OutputEdictIsNotInTransaction);
        }

        #[test]
        fn error_when_pointer_missing() {
            let mut outputs = vec![create_utxo_rs3(546, 41, 0)];

            let runestone = Runestone {
                pointer: None,
                edicts: vec![],
                ..Default::default()
            };

            let mut prev_runes = RuneSet3::default();
            prev_runes
                .insert(arch_program::rune::RuneAmount {
                    id: arch_program::rune::RuneId::new(0, 0),
                    amount: 10,
                })
                .unwrap();

            let err = super::update_modified_program_utxos_with_rune_amount(
                &mut outputs,
                &runestone,
                &mut prev_runes,
            )
            .unwrap_err();
            assert_eq!(err, StateShardError::RunestonePointerIsNotInTransaction);
        }

        #[test]
        fn error_when_pointer_not_in_tx() {
            let mut outputs = vec![create_utxo_rs3(546, 42, 0)];

            let runestone = Runestone {
                pointer: Some(5), // non-existent
                edicts: vec![],
                ..Default::default()
            };

            let mut prev_runes = RuneSet3::default();
            prev_runes
                .insert(arch_program::rune::RuneAmount {
                    id: arch_program::rune::RuneId::new(0, 0),
                    amount: 10,
                })
                .unwrap();

            let err = super::update_modified_program_utxos_with_rune_amount(
                &mut outputs,
                &runestone,
                &mut prev_runes,
            )
            .unwrap_err();
            assert_eq!(err, StateShardError::RunestonePointerIsNotInTransaction);
        }

        #[test]
        fn error_when_not_enough_rune() {
            let mut outputs = vec![create_utxo_rs3(546, 43, 0)];

            let runestone = Runestone {
                pointer: Some(0),
                edicts: vec![Edict {
                    id: ordinals::RuneId { block: 1, tx: 0 },
                    amount: 20,
                    output: 0,
                }],
                ..Default::default()
            };

            let mut prev_runes = RuneSet3::default();
            prev_runes
                .insert(arch_program::rune::RuneAmount {
                    id: arch_program::rune::RuneId::new(1, 0),
                    amount: 10,
                })
                .unwrap();

            let err = super::update_modified_program_utxos_with_rune_amount(
                &mut outputs,
                &runestone,
                &mut prev_runes,
            )
            .unwrap_err();
            assert_eq!(err, StateShardError::NotEnoughRuneInShards);
        }
    }
}
