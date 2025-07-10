use core::marker::PhantomData;

use bytemuck::{Pod, Zeroable};
use saturn_account_parser::codec::zero_copy::AccountLoader;
use saturn_account_parser::codec::zero_copy::Discriminator;
use saturn_collections::generic::fixed_list::{FixedList, FixedListError};

use crate::shard_handle::ShardHandle;
use crate::shard_indices::IntoShardIndices;
use arch_program::program_error::ProgramError;

/// Marker type representing an **unselected** set of shards.
pub struct Unselected;

/// Marker type representing a **selected** subset of shards.
pub struct Selected;

/// A type-safe wrapper around a slice of [`AccountLoader`]s representing the
/// shards that belong to the currently executing instruction.
///
/// # Typestate Pattern
///
/// Like its in-memory predecessor (`shard_set.rs`) this variant follows the
/// **typestate** pattern to ensure that callers always remember to narrow down
/// the set of shards they want to work with:
///
/// * [`Unselected`] – newly created `ShardSet` that exposes only a very small
///   API (`len`, `is_empty`, [`select_with`]) and therefore prevents accidental
///   mutations across *all* shards.
/// * [`Selected`] – returned by one of the selection helpers and unlocks the
///   full, high-level API (`selected_indices`, [`handle_by_index`],
///   [`for_each_mut`], …).
///
/// The compiler will make it impossible to call "selected-only" functions
/// unless `.select_with(…)` (or a future convenience wrapper) has been invoked
/// first.
///
/// # Generic Parameters
///
/// * `'info` – lifetime that ties this wrapper to the surrounding Anchor
///   context.
/// * `S` – zero-copy shard type that must implement [`Pod`] + [`Zeroable`].
/// * `MAX_SELECTED_SHARDS` – upper bound enforced at **compile time** on how
///   many shards can participate in a single operation.
/// * `State` – either [`Unselected`] (default) or [`Selected`]; manipulated by
///   the public API and **never** supplied by callers.
///
/// # Example
///
/// ```rust,ignore
/// use saturn_account_shards::shard_set_loader::{ShardSet, Unselected};
/// # use saturn_account_shards::shard_handle::ShardHandle;
/// # use saturn_account_parser::codec::zero_copy::AccountLoader;
/// # use bytemuck::{Pod, Zeroable};
///
/// # #[derive(Default, Clone, Copy)]
/// # #[repr(C)]
/// # struct DummyShard; // implements Pod + Zeroable
/// # unsafe impl Pod for DummyShard {}
/// # unsafe impl Zeroable for DummyShard {}
///
/// // Imagine these loaders coming from the instruction's account context.
/// fn example<'info>(loaders: &'info [&'info AccountLoader<'info, DummyShard>]) {
///     // Create an *unselected* ShardSet
///     let shards = ShardSet::<DummyShard, 4>::from_loaders(loaders);
///
///     // Pick the shards we actually want to touch – everything else stays immutable
///     let selected = shards.select_with([0, 2]).expect("invalid selection");
///
///     // Do something with the selected shards
///     selected
///         .for_each_mut(|shard| {
///             // mutate shard...
///         })
///         .unwrap();
/// }
/// ```
///
/// Internally **no long-lived `Ref` or `RefMut` is held**. Every call borrows a
/// shard only for the exact duration of the closure passed to
/// [`ShardHandle::with_ref`] / [`ShardHandle::with_mut`], making it impossible
/// to accidentally lock up accounts for longer than necessary.
///
/// The `shard_set_loader` module supersedes the older, in-memory
/// `shard_set.rs` implementation and will receive new functionality first.
#[allow(dead_code)]
pub struct ShardSet<'info, S, const MAX_SELECTED_SHARDS: usize, State = Unselected>
where
    S: Pod + Zeroable + Discriminator + 'static,
{
    /// All shard loaders supplied by the caller.
    loaders: &'info [&'info AccountLoader<'info, S>],

    /// Indexes of the shards that are currently *selected* (may be empty while
    /// the set is in the [`Unselected`] state).
    selected: FixedList<usize, MAX_SELECTED_SHARDS>,

    /// Typestate marker.
    _state: PhantomData<State>,
}

// ---------------------------- Unselected ------------------------------------
impl<'info, S, const MAX_SELECTED_SHARDS: usize> ShardSet<'info, S, MAX_SELECTED_SHARDS, Unselected>
where
    S: Pod + Zeroable + Discriminator + 'static,
{
    /// Creates a new `ShardSet` wrapping the provided loaders.
    #[inline]
    pub fn from_loaders(loaders: &'info [&'info AccountLoader<'info, S>]) -> Self {
        Self {
            loaders,
            selected: FixedList::new(),
            _state: PhantomData,
        }
    }

    /// Number of shards (loaders) available.
    #[inline]
    pub fn len(&self) -> usize {
        self.loaders.len()
    }

    /// `true` if no shards are present.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.loaders.is_empty()
    }
}

// ----------------- Unselected -> Selected -------------------------
impl<'info, S, const MAX_SELECTED_SHARDS: usize> ShardSet<'info, S, MAX_SELECTED_SHARDS, Unselected>
where
    S: Pod + Zeroable + Discriminator + 'static,
{
    /// Select shards by index and transition into the [`Selected`] state. Only
    /// available on *writable* shard sets.
    pub fn select_with<T>(
        mut self,
        spec: T,
    ) -> Result<ShardSet<'info, S, MAX_SELECTED_SHARDS, Selected>, FixedListError>
    where
        T: IntoShardIndices<MAX_SELECTED_SHARDS>,
    {
        let indexes = spec.into_indices()?;

        for &idx in indexes.as_slice() {
            debug_assert!(idx < self.loaders.len());
            self.selected.push(idx)?;
        }

        Ok(ShardSet {
            loaders: self.loaders,
            selected: self.selected,
            _state: PhantomData,
        })
    }
}

// ---------------------------- Selected -------------------------------
impl<'info, S, const MAX_SELECTED_SHARDS: usize> ShardSet<'info, S, MAX_SELECTED_SHARDS, Selected>
where
    S: Pod + Zeroable + Discriminator + 'static,
{
    /// Returns the indexes that were selected via [`Self::select_with`].
    #[inline]
    pub fn selected_indices(&self) -> &[usize] {
        self.selected.as_slice()
    }

    /// Returns a [`ShardHandle`] for the shard at **global** `idx`.
    #[inline]
    pub fn handle_by_index(&self, idx: usize) -> ShardHandle<'info, S> {
        debug_assert!(idx < self.loaders.len());
        ShardHandle::new(self.loaders[idx])
    }

    /// Executes `f` for every **selected** shard, borrowing each one exactly
    /// for the duration of the closure call. Only available on *writable*
    /// shard sets.
    pub fn for_each<R>(&self, mut f: impl FnMut(&S) -> R) -> Result<Vec<R>, ProgramError> {
        let mut results = Vec::with_capacity(self.selected.len());
        for &idx in self.selected.iter() {
            let handle = ShardHandle::new(self.loaders[idx]);
            let out = handle.with_ref(|shard| f(shard))?;
            results.push(out);
        }
        Ok(results)
    }
}

// ------------------------ Selected (mutable helper) --------------------------------
impl<'info, S, const MAX_SELECTED_SHARDS: usize> ShardSet<'info, S, MAX_SELECTED_SHARDS, Selected>
where
    S: Pod + Zeroable + Discriminator + 'static,
{
    /// Executes `f` for every **selected** shard mutably.
    pub fn for_each_mut<R>(&self, mut f: impl FnMut(&mut S) -> R) -> Result<Vec<R>, ProgramError> {
        let mut results = Vec::with_capacity(self.selected.len());
        for &idx in self.selected.iter() {
            let handle = ShardHandle::new(self.loaders[idx]);
            let out = handle.with_mut(|shard| f(shard))?;
            results.push(out);
        }
        Ok(results)
    }
}
