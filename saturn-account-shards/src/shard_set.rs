#[cfg(feature = "runes")]
use crate::StateShardError;
use crate::{shard_indices::IntoShardIndices, StateShard};
use arch_program::rune::RuneAmount;
use core::marker::PhantomData;
#[cfg(feature = "runes")]
use saturn_bitcoin_transactions::error::BitcoinTxError;
use saturn_bitcoin_transactions::utxo_info::UtxoInfoTrait;
use saturn_collections::generic::fixed_list::{FixedList, FixedListError};
use saturn_collections::generic::fixed_set::FixedCapacitySet;
use std::cmp::{Ordering, Reverse};

/// Marker type representing a `ShardSet` that has **not** yet been filtered
/// via [`ShardSet::select`].  Only a few safe helper methods are available in
/// this state (e.g. [`ShardSet::len`]).
pub struct Unselected;

/// Marker type representing a `ShardSet` that **has** an active shard
/// selection.  Almost all high-level helpers are implemented for this state
/// only, ensuring at compile time that callers remembered to call
/// [`ShardSet::select`] first.
pub struct Selected;

/// A thin wrapper around a mutable slice of shards that tracks, at the type
/// level, whether the caller has already narrowed down which shards they are
/// currently operating on.
///
/// ## Typestate Pattern
///
/// `ShardSet` uses the typestate pattern to enforce correct usage at compile time:
/// - [`Unselected`] state: Only basic operations like [`len`](Self::len) are available
/// - [`Selected`] state: Full API is available after calling a selection method
///
/// ## Generic Parameters
///
/// - `'a` - Lifetime of the borrowed shard slice
/// - `RuneSet` - Type representing a set of rune amounts with fixed capacity
/// - `U` - UTXO info type that must implement [`UtxoInfoTrait`]
/// - `S` - Shard type that must implement [`StateShard`]
/// - `MAX_SELECTED_SHARDS` - Maximum number of shards that can be selected
/// - `State` - Typestate marker, either [`Unselected`] (default) or [`Selected`]
///
/// ## Example
///
/// ```rust
/// # use saturn_account_shards::{ShardSet, StateShard};
/// # use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};
/// # use bitcoin::Amount;
/// #
/// # // Minimal stub types for the example
/// # #[derive(Default)]
/// # struct DummyShard;
/// #
/// # impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for DummyShard {
/// #     fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] { &[] }
/// #     fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] { &mut [] }
/// #     fn btc_utxos_retain(&mut self, _: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {}
/// #     fn add_btc_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) -> Option<usize> { None }
/// #     fn btc_utxos_len(&self) -> usize { 0 }
/// #     fn btc_utxos_max_len(&self) -> usize { 0 }
/// #     fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> { None }
/// #     fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> { None }
/// #     fn clear_rune_utxo(&mut self) {}
/// #     fn set_rune_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) {}
/// # }
/// #
/// # let mut shard1 = DummyShard::default();
/// # let mut shard2 = DummyShard::default();
/// # let mut shards = [&mut shard1, &mut shard2];
/// #
/// // Create an unselected ShardSet
/// let shard_set = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DummyShard, 2>::new(&mut shards);
///
/// // Select specific shards to work with
/// let selected = shard_set.select_with([0, 1]).unwrap();
///
/// // Now all high-level operations are available
/// // selected.redistribute_remaining_btc_to_shards(...);
/// ```
pub struct ShardSet<
    'a,
    RuneSet: FixedCapacitySet<Item = RuneAmount> + Default,
    U: UtxoInfoTrait<RuneSet>,
    S: crate::shard::StateShard<U, RuneSet>,
    const MAX_SELECTED_SHARDS: usize,
    State = Unselected,
> {
    /// Mutable slice of all shards involved in this call.
    ///
    /// The lifetime `'a` is tied to the caller – `ShardSet` never allocates
    /// new shards, it only provides ergonomic access to the ones the caller
    /// already borrowed mutably.
    shards: &'a mut [&'a mut S],

    /// The subset of shards the caller is operating on (may be empty while
    /// the set is in the [`Unselected`] state).
    selected: FixedList<usize, MAX_SELECTED_SHARDS>,

    /// Zero-sized field that encodes the current typestate.
    #[doc(hidden)]
    _state: PhantomData<State>,

    /// Phantom data to ensure U is used
    #[doc(hidden)]
    _utxo_info: PhantomData<U>,

    /// Phantom data to ensure RuneSet is used
    #[doc(hidden)]
    _rune_set: PhantomData<RuneSet>,
}

impl<
        'a,
        RuneSet: FixedCapacitySet<Item = arch_program::rune::RuneAmount> + Default,
        U: UtxoInfoTrait<RuneSet>,
        S: crate::shard::StateShard<U, RuneSet>,
        const MAX_SELECTED_SHARDS: usize,
        State,
    > ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, State>
{
    /// Returns the number of shards in the set.
    ///
    /// This method is available in both [`Unselected`] and [`Selected`] states.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use saturn_account_shards::{ShardSet, StateShard};
    /// # use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};
    /// # #[derive(Default)]
    /// # struct DummyShard;
    /// # impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for DummyShard {
    /// #     fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] { &[] }
    /// #     fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] { &mut [] }
    /// #     fn btc_utxos_retain(&mut self, _: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {}
    /// #     fn add_btc_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) -> Option<usize> { None }
    /// #     fn btc_utxos_len(&self) -> usize { 0 }
    /// #     fn btc_utxos_max_len(&self) -> usize { 0 }
    /// #     fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> { None }
    /// #     fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> { None }
    /// #     fn clear_rune_utxo(&mut self) {}
    /// #     fn set_rune_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) {}
    /// # }
    /// # let mut shard1 = DummyShard::default();
    /// # let mut shard2 = DummyShard::default();
    /// # let mut shards = [&mut shard1, &mut shard2];
    /// let shard_set = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DummyShard, 2>::new(&mut shards);
    /// assert_eq!(shard_set.len(), 2);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.shards.len()
    }

    /// Returns `true` if the shard set contains no shards.
    ///
    /// This method is available in both [`Unselected`] and [`Selected`] states.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use saturn_account_shards::{ShardSet, StateShard};
    /// # use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};
    /// # #[derive(Default)]
    /// # struct DummyShard;
    /// # impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for DummyShard {
    /// #     fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] { &[] }
    /// #     fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] { &mut [] }
    /// #     fn btc_utxos_retain(&mut self, _: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {}
    /// #     fn add_btc_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) -> Option<usize> { None }
    /// #     fn btc_utxos_len(&self) -> usize { 0 }
    /// #     fn btc_utxos_max_len(&self) -> usize { 0 }
    /// #     fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> { None }
    /// #     fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> { None }
    /// #     fn clear_rune_utxo(&mut self) {}
    /// #     fn set_rune_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) {}
    /// # }
    /// # let mut shards: [&mut DummyShard; 0] = [];
    /// let shard_set = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DummyShard, 0>::new(&mut shards);
    /// assert!(shard_set.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.shards.is_empty()
    }

    /// Mutable access to the underlying slice – sometimes necessary to call
    /// legacy helpers that still expect `&mut [&mut S]` instead of `ShardSet`.
    #[inline]
    pub fn as_mut_slice(&'a mut self) -> &'a mut [&'a mut S] {
        self.shards
    }

    /// Returns a mutable reference to the shard at the specified index.
    ///
    /// # Parameters
    /// - `index` - The zero-based index of the shard to retrieve
    ///
    /// # Panics
    /// Panics if the index is out of bounds (>= [`len`](Self::len)).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use saturn_account_shards::{ShardSet, StateShard};
    /// # use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};
    /// # #[derive(Default)]
    /// # struct DummyShard;
    /// # impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for DummyShard {
    /// #     fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] { &[] }
    /// #     fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] { &mut [] }
    /// #     fn btc_utxos_retain(&mut self, _: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {}
    /// #     fn add_btc_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) -> Option<usize> { None }
    /// #     fn btc_utxos_len(&self) -> usize { 0 }
    /// #     fn btc_utxos_max_len(&self) -> usize { 0 }
    /// #     fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> { None }
    /// #     fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> { None }
    /// #     fn clear_rune_utxo(&mut self) {}
    /// #     fn set_rune_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) {}
    /// # }
    /// # let mut shard1 = DummyShard::default();
    /// # let mut shard2 = DummyShard::default();
    /// # let mut shards = [&mut shard1, &mut shard2];
    /// let mut shard_set = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DummyShard, 2>::new(&mut shards);
    /// let first_shard = shard_set.get_shard_by_index_mut(0);
    /// // Modify the first shard...
    /// ```
    pub fn get_shard_by_index_mut(&'a mut self, index: usize) -> &'a mut S {
        &mut self.shards[index]
    }

    /// Returns an immutable reference to the shard at the specified index.
    ///
    /// # Parameters
    /// - `index` - The zero-based index of the shard to retrieve
    ///
    /// # Panics
    /// Panics if the index is out of bounds (>= [`len`](Self::len)).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use saturn_account_shards::{ShardSet, StateShard};
    /// # use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};
    /// # #[derive(Default)]
    /// # struct DummyShard;
    /// # impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for DummyShard {
    /// #     fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] { &[] }
    /// #     fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] { &mut [] }
    /// #     fn btc_utxos_retain(&mut self, _: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {}
    /// #     fn add_btc_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) -> Option<usize> { None }
    /// #     fn btc_utxos_len(&self) -> usize { 0 }
    /// #     fn btc_utxos_max_len(&self) -> usize { 0 }
    /// #     fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> { None }
    /// #     fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> { None }
    /// #     fn clear_rune_utxo(&mut self) {}
    /// #     fn set_rune_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) {}
    /// # }
    /// # let mut shard1 = DummyShard::default();
    /// # let mut shard2 = DummyShard::default();
    /// # let mut shards = [&mut shard1, &mut shard2];
    /// let shard_set = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DummyShard, 2>::new(&mut shards);
    /// let first_shard = shard_set.get_shard_by_index(0);
    /// // Read from the first shard...
    /// ```
    pub fn get_shard_by_index(&'a self, index: usize) -> &'a S {
        &self.shards[index]
    }
}

// === Constructors & state transitions ====================================================== //

impl<
        'a,
        RuneSet: FixedCapacitySet<Item = arch_program::rune::RuneAmount> + Default,
        U: UtxoInfoTrait<RuneSet>,
        S: crate::shard::StateShard<U, RuneSet>,
        const MAX_SELECTED_SHARDS: usize,
    > ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, Unselected>
{
    /// Creates a new wrapper around an existing mutable slice of shards.
    ///
    /// This is a **zero-cost** operation – no copy of the slice is made, we
    /// merely store the reference so that subsequent helper methods are
    /// discoverable via auto-complete.
    pub fn new(shards: &'a mut [&'a mut S]) -> Self {
        Self {
            shards,
            selected: FixedList::new(),
            _state: PhantomData,
            _utxo_info: PhantomData,
            _rune_set: PhantomData,
        }
    }

    /// Consumes `self`, records the provided `spec` of shard indices and
    /// transitions into the [`Selected`] state.
    ///
    /// Accepts any type that implements [`IntoShardIndices`], removing the
    /// need for callers to fiddle with slices or const generics – **just pass
    /// a single index, a `Vec`, an array, …**.
    #[inline]
    pub fn select_with<T>(
        self,
        spec: T,
    ) -> Result<ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, Selected>, FixedListError>
    where
        T: IntoShardIndices<MAX_SELECTED_SHARDS>,
    {
        let indices = spec.into_indices()?;

        Ok(ShardSet {
            shards: self.shards,
            selected: indices,
            _state: PhantomData,
            _utxo_info: PhantomData,
            _rune_set: PhantomData,
        })
    }

    /// Selects a single shard by applying the provided `key_fn` to all shards,
    /// and choosing the lowest value.
    ///
    /// *Example*:
    /// ```rust
    /// # use saturn_account_shards::{ShardSet, StateShard};
    /// # use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};
    /// # use bitcoin::Amount;
    /// #
    /// # // -----------------------------------------------------------------
    /// # // Minimal stub types so the example compiles. In real code you would
    /// # // use your actual shard implementation.
    /// # // -----------------------------------------------------------------
    /// # #[derive(Default)]
    /// # struct DummyShard;
    /// #
    /// # impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for DummyShard {
    /// #     fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] { &[] }
    /// #     fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] { &mut [] }
    /// #     fn btc_utxos_retain(&mut self, _: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {}
    /// #     fn add_btc_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) -> Option<usize> { None }
    /// #     fn btc_utxos_len(&self) -> usize { 0 }
    /// #     fn btc_utxos_max_len(&self) -> usize { 0 }
    /// #     fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> { None }
    /// #     fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> { None }
    /// #     fn clear_rune_utxo(&mut self) {}
    /// #     fn set_rune_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) {}
    /// # }
    /// #
    /// # // One dummy shard is enough for this illustration.
    /// # let mut shard = DummyShard::default();
    /// # let mut slice: [&mut DummyShard; 1] = [&mut shard];
    /// #
    /// # // Create an *unselected* ShardSet.
    /// # let shards = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DummyShard, 1>::new(&mut slice);
    /// #
    /// // Select the shard with the smallest total BTC.
    /// let selected = shards.select_min_by(|shard| shard.total_btc().to_sat()).unwrap();
    /// ```
    pub fn select_min_by<F>(
        self,
        key_fn: F,
    ) -> Result<ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, Selected>, FixedListError>
    where
        F: Fn(&dyn StateShard<U, RuneSet>) -> u64,
    {
        let index = self
            .shards
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| key_fn(**s))
            .map(|(i, _)| i);

        match index {
            Some(i) => self.select_with([i]),
            None => self.select_with([]),
        }
    }

    /// Selects multiple shards that meet some condition, defined by a predicate.
    ///
    /// This method filters all shards using the provided predicate function and
    /// selects those that return `true`. The resulting `ShardSet` transitions to
    /// the [`Selected`] state.
    ///
    /// # Parameters
    /// - `predicate` - A function that takes a shard reference and returns `true`
    ///   if the shard should be selected
    ///
    /// # Returns
    /// A `Result` containing either:
    /// - `Ok(ShardSet<Selected>)` - A `ShardSet` in the [`Selected`] state with
    ///   the filtered shards
    /// - `Err(FixedListError)` - If more shards match the predicate than
    ///   `MAX_SELECTED_SHARDS`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use saturn_account_shards::{ShardSet, StateShard};
    /// # use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};
    /// # use bitcoin::Amount;
    /// # #[derive(Default)]
    /// # struct DummyShard { btc_amount: u64 }
    /// # impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for DummyShard {
    /// #     fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] { &[] }
    /// #     fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] { &mut [] }
    /// #     fn btc_utxos_retain(&mut self, _: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {}
    /// #     fn add_btc_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) -> Option<usize> { None }
    /// #     fn btc_utxos_len(&self) -> usize { 0 }
    /// #     fn btc_utxos_max_len(&self) -> usize { 0 }
    /// #     fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> { None }
    /// #     fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> { None }
    /// #     fn clear_rune_utxo(&mut self) {}
    /// #     fn set_rune_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) {}
    /// # }
    /// # let mut shard1 = DummyShard { btc_amount: 1000 };
    /// # let mut shard2 = DummyShard { btc_amount: 500 };
    /// # let mut shard3 = DummyShard { btc_amount: 2000 };
    /// # let mut shards = [&mut shard1, &mut shard2, &mut shard3];
    /// let shard_set = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DummyShard, 3>::new(&mut shards);
    ///
    /// // Select all shards with more than 750 satoshis
    /// let selected = shard_set.select_multiple_by(|shard| {
    ///     shard.total_btc().to_sat() > 750
    /// }).unwrap();
    ///
    /// // selected now contains shards with 1000 and 2000 satoshis
    /// ```
    pub fn select_multiple_by<P>(
        self,
        predicate: P,
    ) -> Result<ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, Selected>, FixedListError>
    where
        P: Fn(&dyn StateShard<U, RuneSet>) -> bool,
    {
        let indices: Vec<_> = self
            .shards
            .iter()
            .enumerate()
            .filter(|(_, shard)| predicate(**shard))
            .map(|(i, _)| i)
            .collect();

        self.select_with(indices)
    }

    /// Selects multiple shards that together fulfill a predicate, by sorting
    /// them first using the provided function.
    ///
    /// This method sorts all shards by the provided key function (in descending order),
    /// then iteratively tests increasing subsets of shards until the predicate is
    /// satisfied. This is useful for selecting the minimum number of shards needed
    /// to meet some criteria (e.g., total liquidity requirements).
    ///
    /// # Parameters
    /// - `key_fn` - Function that returns a sorting key for each shard. Higher numbers
    ///   result in being closer to the start, lower numbers go towards the end
    /// - `predicate` - Function that tests whether a subset of shards meets the criteria
    ///
    /// # Returns
    /// A `Result` containing either:
    /// - `Ok(ShardSet<Selected>)` - A `ShardSet` with the minimum number of shards
    ///   that satisfy the predicate
    /// - `Err(FixedListError)` - If the selection exceeds `MAX_SELECTED_SHARDS`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use saturn_account_shards::{ShardSet, StateShard};
    /// # use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};
    /// # use bitcoin::Amount;
    /// # #[derive(Default)]
    /// # struct DummyShard { btc_amount: u64 }
    /// # impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for DummyShard {
    /// #     fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] { &[] }
    /// #     fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] { &mut [] }
    /// #     fn btc_utxos_retain(&mut self, _: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {}
    /// #     fn add_btc_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) -> Option<usize> { None }
    /// #     fn btc_utxos_len(&self) -> usize { 0 }
    /// #     fn btc_utxos_max_len(&self) -> usize { 0 }
    /// #     fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> { None }
    /// #     fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> { None }
    /// #     fn clear_rune_utxo(&mut self) {}
    /// #     fn set_rune_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) {}
    /// # }
    /// # let mut shard1 = DummyShard { btc_amount: 1000 };
    /// # let mut shard2 = DummyShard { btc_amount: 500 };
    /// # let mut shard3 = DummyShard { btc_amount: 2000 };
    /// # let mut shards = [&mut shard1, &mut shard2, &mut shard3];
    /// let shard_set = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DummyShard, 3>::new(&mut shards);
    ///
    /// // Select the minimum shards needed to have at least 1500 satoshis total
    /// let selected = shard_set.select_multiple_sorted(
    ///     |shard| shard.total_btc().to_sat(),  // Sort by BTC amount (descending)
    ///     |shards| {
    ///         let total: u64 = shards.iter().map(|s| s.total_btc().to_sat()).sum();
    ///         total >= 1500
    ///     }
    /// ).unwrap();
    ///
    /// // selected will contain the shard with 2000 satoshis (sufficient alone)
    /// ```
    pub fn select_multiple_sorted<K, P>(
        self,

        key_fn: K,
        predicate: P,
    ) -> Result<ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, Selected>, FixedListError>
    where
        K: Fn(&dyn StateShard<U, RuneSet>) -> u64,
        P: Fn(&[&mut dyn StateShard<U, RuneSet>]) -> bool,
    {
        let mut indices: Vec<_> = (0..self.len()).collect();

        indices.sort_by_key(|i| {
            let shard = &self.shards[*i];
            let key = key_fn(*shard);

            Reverse(key)
        });

        for n in 1..=self.len() {
            let indices: Vec<_> = indices.iter().take(n).copied().collect();
            let shards: Vec<_> = self
                .shards
                .iter_mut()
                .enumerate()
                .filter(|(i, _)| indices.iter().find(|j| i == *j).is_some())
                .map(|(_, s)| *s as &mut dyn StateShard<U, RuneSet>)
                .collect();

            if predicate(shards.as_slice()) {
                return self.select_with(indices);
            } else {
                continue;
            }
        }

        self.select_with(indices)
    }
}

// === Helpers that require an *active* selection ============================================ //

impl<
        'a,
        RuneSet: FixedCapacitySet<Item = arch_program::rune::RuneAmount> + Default,
        U: UtxoInfoTrait<RuneSet>,
        S: crate::shard::StateShard<U, RuneSet>,
        const MAX_SELECTED_SHARDS: usize,
    > ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, Selected>
{
    /// Returns an iterator over the indices of all shards that were part of the selection.
    ///
    /// This method is only available when the `ShardSet` is in the [`Selected`] state.
    /// The iterator yields the zero-based indices of the selected shards in the order
    /// they were selected.
    ///
    /// # Returns
    /// An iterator yielding `usize` indices of the selected shards.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use saturn_account_shards::{ShardSet, StateShard};
    /// # use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};
    /// # #[derive(Default)]
    /// # struct DummyShard;
    /// # impl StateShard<UtxoInfo<SingleRuneSet>, SingleRuneSet> for DummyShard {
    /// #     fn btc_utxos(&self) -> &[UtxoInfo<SingleRuneSet>] { &[] }
    /// #     fn btc_utxos_mut(&mut self) -> &mut [UtxoInfo<SingleRuneSet>] { &mut [] }
    /// #     fn btc_utxos_retain(&mut self, _: &mut dyn FnMut(&UtxoInfo<SingleRuneSet>) -> bool) {}
    /// #     fn add_btc_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) -> Option<usize> { None }
    /// #     fn btc_utxos_len(&self) -> usize { 0 }
    /// #     fn btc_utxos_max_len(&self) -> usize { 0 }
    /// #     fn rune_utxo(&self) -> Option<&UtxoInfo<SingleRuneSet>> { None }
    /// #     fn rune_utxo_mut(&mut self) -> Option<&mut UtxoInfo<SingleRuneSet>> { None }
    /// #     fn clear_rune_utxo(&mut self) {}
    /// #     fn set_rune_utxo(&mut self, _: UtxoInfo<SingleRuneSet>) {}
    /// # }
    /// # let mut shard1 = DummyShard::default();
    /// # let mut shard2 = DummyShard::default();
    /// # let mut shard3 = DummyShard::default();
    /// # let mut shards = [&mut shard1, &mut shard2, &mut shard3];
    /// let shard_set = ShardSet::<SingleRuneSet, UtxoInfo<SingleRuneSet>, DummyShard, 3>::new(&mut shards);
    /// let selected = shard_set.select_with([0, 2]).unwrap();
    ///
    /// let indices: Vec<usize> = selected.selected_indices().collect();
    /// assert_eq!(indices, vec![0, 2]);
    /// ```
    pub fn selected_indices(&self) -> &[usize] {
        self.selected.as_slice()
    }

    /// Redistributes the *remaining satoshis* that still belong to the selected shards
    /// back into brand-new outputs of `tx_builder` so that liquidity across the
    /// shards ends up **as even as possible**.
    ///
    /// This is a thin convenience wrapper around
    /// [`crate::split::redistribute_remaining_btc_to_shards`].  Refer to that
    /// function for the complete algorithmic details.
    ///
    /// The length and ordering of the returned `Vec` mirrors the current
    /// selection and can therefore be zipped with
    /// [`ShardSet::selected_indices`] for post-processing.
    ///
    /// # Parameters
    /// * `tx_builder` – The transaction that is currently being assembled.
    /// * `removed_from_shards` – Total satoshis the caller has already taken
    ///   out of the selected shards in the scope of the current instruction.
    /// * `program_script_pubkey` – The program's address – every newly created
    ///   change output will be locked to this script.
    /// * `fee_rate` – Fee rate that the program uses to offset its own fee
    ///   inputs.
    ///
    /// # Errors
    /// Forwards any [`saturn_safe_math::MathError`] returned by the underlying
    /// helper.
    #[allow(clippy::type_complexity)]
    pub fn redistribute_remaining_btc_to_shards<
        const MAX_USER_UTXOS: usize,
        const MAX_SHARDS_PER_POOL: usize,
    >(
        &'a mut self,
        tx_builder: &mut saturn_bitcoin_transactions::TransactionBuilder<
            MAX_USER_UTXOS,
            MAX_SHARDS_PER_POOL,
            RuneSet,
        >,
        removed_from_shards: u64,
        program_script_pubkey: bitcoin::ScriptBuf,
        fee_rate: &saturn_bitcoin_transactions::fee_rate::FeeRate,
    ) -> core::result::Result<Vec<u128>, saturn_safe_math::MathError> {
        let indices: FixedList<usize, MAX_SELECTED_SHARDS> =
            FixedList::from_slice(self.selected.as_slice());

        crate::split::redistribute_remaining_btc_to_shards(
            tx_builder,
            self.as_mut_slice(),
            indices.as_slice(),
            removed_from_shards,
            program_script_pubkey,
            fee_rate,
        )
    }

    /// Same as [`ShardSet::redistribute_remaining_btc_to_shards`] but for Rune
    /// tokens instead of satoshis.
    ///
    /// The helper updates the embedded *runestone* inside `tx_builder` so that
    /// the redistributed Rune balances are reflected on-chain.  See
    /// [`crate::split::redistribute_remaining_rune_to_shards`] for full
    /// details.
    ///
    /// The returned vector obeys the same invariants as the BTC variant – one
    /// entry per selected shard, ordered identically.
    ///
    /// # Parameters
    /// * `tx_builder` – Mutable reference to the in-flight transaction builder.
    /// * `rune_id` – The identifier of the Rune that is being redistributed.
    /// * `removed_from_shards` – Total number of Rune tokens already taken out
    ///   of the shards during the current instruction.
    /// * `program_script_pubkey` – Script locking the newly created change
    ///   outputs.
    ///
    /// # Errors
    /// Propagates [`saturn_safe_math::MathError`] on arithmetic overflow or
    /// underflow.
    #[allow(clippy::type_complexity)]
    #[cfg(feature = "runes")]
    pub fn redistribute_remaining_rune_to_shards<
        const MAX_USER_UTXOS: usize,
        const MAX_SHARDS_PER_POOL: usize,
    >(
        &'a mut self,
        tx_builder: &mut saturn_bitcoin_transactions::TransactionBuilder<
            MAX_USER_UTXOS,
            MAX_SHARDS_PER_POOL,
            RuneSet,
        >,
        removed_from_shards: RuneSet,
        program_script_pubkey: bitcoin::ScriptBuf,
    ) -> core::result::Result<Vec<RuneSet>, StateShardError> {
        let indices: FixedList<usize, MAX_SELECTED_SHARDS> =
            FixedList::from_slice(self.selected.as_slice());

        crate::split::redistribute_remaining_rune_to_shards(
            tx_builder,
            self.as_mut_slice(),
            indices.as_slice(),
            removed_from_shards,
            program_script_pubkey,
        )
    }

    /// Calculates the number of satoshis that are **still pending to be
    /// returned** to the selected shards after accounting for already removed
    /// liquidity and the program-paid fees.
    ///
    /// This is a convenience wrapper around
    /// [`crate::split::compute_unsettled_btc_in_shards`].  The semantics and
    /// invariants are identical to the free function – the only purpose of
    /// this method is to relieve callers from having to manually assemble the
    /// `shards` slice and `shard_indexes` array.
    ///
    /// # Parameters
    /// * `tx_builder` – Reference to the in-flight transaction.
    /// * `removed_from_shards` – Total satoshis that have already been taken
    ///   out of the selected shards in the context of the current
    ///   instruction.
    /// * `fee_rate` – Fee rate used when calculating how many satoshis the
    ///   program itself has already paid.
    ///
    /// # Errors
    /// Propagates [`saturn_safe_math::MathError`] on arithmetic overflow or
    /// underflow.
    pub fn compute_unsettled_btc_in_shards<
        const MAX_USER_UTXOS: usize,
        const MAX_SHARDS_PER_POOL: usize,
    >(
        &'a mut self,
        tx_builder: &saturn_bitcoin_transactions::TransactionBuilder<
            MAX_USER_UTXOS,
            MAX_SHARDS_PER_POOL,
            RuneSet,
        >,
        removed_from_shards: u64,
        fee_rate: &saturn_bitcoin_transactions::fee_rate::FeeRate,
    ) -> core::result::Result<u64, saturn_safe_math::MathError> {
        let indices: FixedList<usize, MAX_SELECTED_SHARDS> =
            FixedList::from_slice(self.selected.as_slice());

        crate::split::compute_unsettled_btc_in_shards(
            tx_builder,
            self.as_mut_slice(),
            indices.as_slice(),
            removed_from_shards,
            fee_rate,
        )
    }

    /// Same as [`ShardSet::compute_unsettled_btc_in_shards`] but for Rune
    /// tokens instead of satoshis.
    ///
    /// This is a thin convenience wrapper around
    /// [`crate::split::compute_unsettled_rune_in_shards`].  The returned
    /// amount represents the number of Rune tokens that still need to be sent
    /// back to the selected shards so that no tokens are lost.
    #[cfg(feature = "runes")]
    pub fn compute_unsettled_rune_in_shards(
        &'a mut self,
        removed_from_shards: RuneSet,
    ) -> core::result::Result<RuneSet, StateShardError> {
        let indices: FixedList<usize, MAX_SELECTED_SHARDS> =
            FixedList::from_slice(self.selected.as_slice());

        crate::split::compute_unsettled_rune_in_shards(
            self.as_mut_slice(),
            indices.as_slice(),
            removed_from_shards,
        )
    }

    /// Plans an **as-balanced-as-possible** redistribution of the provided
    /// `amount` of satoshis across the currently selected shards **without**
    /// mutating either the shards or the in-flight transaction.
    ///
    /// Internally this forwards to
    /// [`crate::split::plan_btc_distribution_among_shards`].  Refer to that
    /// function for the full algorithmic details and invariants.
    ///
    /// The returned vector obeys the following properties:
    /// * Its length matches the current selection (one entry per shard).
    /// * The i-th value corresponds to the i-th index returned by
    ///   [`ShardSet::selected_indices`].
    /// * The sum of all entries equals the supplied `amount` (subject to
    ///   integer-division rounding).
    #[allow(clippy::type_complexity)]
    pub fn plan_btc_distribution_among_shards<
        const MAX_USER_UTXOS: usize,
        const MAX_SHARDS_PER_POOL: usize,
    >(
        &'a mut self,
        tx_builder: &saturn_bitcoin_transactions::TransactionBuilder<
            MAX_USER_UTXOS,
            MAX_SHARDS_PER_POOL,
            RuneSet,
        >,
        amount: u128,
    ) -> core::result::Result<Vec<u128>, saturn_safe_math::MathError> {
        let indices: FixedList<usize, MAX_SELECTED_SHARDS> =
            FixedList::from_slice(self.selected.as_slice());

        crate::split::plan_btc_distribution_among_shards(
            tx_builder,
            self.as_mut_slice(),
            indices.as_slice(),
            amount,
        )
    }

    /// Same as [`ShardSet::plan_btc_distribution_among_shards`] but for Rune
    /// tokens instead of satoshis.
    ///
    /// The helper merely computes the optimal allocation and **does not**
    /// mutate any state – callers are responsible for turning the resulting
    /// plan into concrete outputs/edicts.
    #[allow(clippy::type_complexity)]
    #[cfg(feature = "runes")]
    pub fn plan_rune_distribution_among_shards<
        const MAX_USER_UTXOS: usize,
        const MAX_SHARDS_PER_POOL: usize,
    >(
        &'a mut self,
        tx_builder: &mut saturn_bitcoin_transactions::TransactionBuilder<
            MAX_USER_UTXOS,
            MAX_SHARDS_PER_POOL,
            RuneSet,
        >,
        amounts: &RuneSet,
    ) -> core::result::Result<Vec<RuneSet>, StateShardError> {
        let indices: FixedList<usize, MAX_SELECTED_SHARDS> =
            FixedList::from_slice(self.selected.as_slice());

        crate::split::plan_rune_distribution_among_shards(
            tx_builder,
            self.as_mut_slice(),
            indices.as_slice(),
            amounts,
        )
    }

    /// Applies the effects of a **broadcast & accepted** transaction back onto
    /// the selected shards so that their in-memory UTXO sets stay in sync with
    /// the chain.
    ///
    /// Behind the scenes this delegates to
    /// [`crate::update::update_shards_after_transaction`] and therefore offers
    /// the exact same semantics – the only difference is that callers no
    /// longer need to plumb through the shard slice and index array
    /// themselves.
    ///
    /// # Parameters
    /// * `transaction_builder` – The builder that was used to assemble & sign
    ///   the transaction.  It is **consumed** by the underlying helper, so a
    ///   mutable reference is required.
    /// * `program_script_pubkey` – The script that identifies outputs owned by
    ///   the program (usually its on-chain address).
    /// * `default_rune_id` – When the `runes` feature is enabled, any Rune
    ///   amount that cannot be matched to an explicit edict will be credited
    ///   to this ID.
    /// * `fee_rate` – Fee rate used to mark new UTXOs that would benefit from
    ///   future consolidation (only relevant when the `utxo-consolidation`
    ///   feature is active).
    ///
    /// # Errors
    /// Bubbles up any [`crate::error::StateShardError`] produced by the inner
    /// implementation, wrapped in the crate-level [`Result`] alias.
    #[allow(clippy::too_many_arguments)]
    pub fn update_shards_after_transaction<
        const MAX_USER_UTXOS: usize,
        const MAX_SHARDS_PER_POOL: usize,
    >(
        &'a mut self,
        transaction_builder: &mut saturn_bitcoin_transactions::TransactionBuilder<
            MAX_USER_UTXOS,
            MAX_SHARDS_PER_POOL,
            RuneSet,
        >,
        program_script_pubkey: &bitcoin::ScriptBuf,
        fee_rate: &saturn_bitcoin_transactions::fee_rate::FeeRate,
    ) -> crate::error::Result<()> {
        let indices: FixedList<usize, MAX_SELECTED_SHARDS> =
            FixedList::from_slice(self.selected.as_slice());

        crate::update::update_shards_after_transaction(
            transaction_builder,
            self.as_mut_slice(),
            indices.as_slice(),
            program_script_pubkey,
            fee_rate,
        )
    }

    /// Applies the provided function to all the selected shards
    pub fn for_each<F>(&mut self, mut fun: F)
    where
        F: FnMut(&mut S) -> (),
    {
        self.selected_indices()
            .to_vec()
            .iter()
            .copied()
            .for_each(|i| fun(&mut self.shards[i]));
    }

    /// Returns all the BTC UTXOs in the selected shards
    pub fn btc_utxos(&self) -> Vec<&U> {
        self.selected_indices()
            .iter()
            .copied()
            .filter_map(|index| self.shards.get(index))
            .flat_map(|shard| shard.btc_utxos().iter())
            .collect()
    }

    /// Returns all the Rune UTXOs in the selected shards
    pub fn rune_utxos(&self) -> Vec<&U> {
        self.selected_indices()
            .iter()
            .copied()
            .filter_map(|index| self.shards.get(index))
            .filter_map(|shard| shard.rune_utxo())
            .collect()
    }
}
