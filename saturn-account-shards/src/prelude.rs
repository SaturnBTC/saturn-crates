//! Public aliases and helper macro for `ShardSet` ergonomics.
//!
//! Import this as `use saturn_account_shards::prelude::*` to gain the
//! user-friendly façade without pulling in the full type machinery.

use crate::{
    shard_set::{Selected, Unselected},
    ShardSet,
};
use saturn_bitcoin_transactions::utxo_info::UtxoInfoTrait;
use saturn_collections::generic::fixed_set::FixedCapacitySet;

/// Convenience alias for an **unselected** `ShardSet` in which the typestate
/// parameter is hidden.
pub type Shards<'a, RuneSet, U, S, const MAX_SELECTED_SHARDS: usize> =
    ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, Unselected>;

/// Convenience alias for a `ShardSet` that already carries an active shard
/// selection.
pub type SelectedShards<'a, RuneSet, U, S, const MAX_SELECTED_SHARDS: usize> =
    ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, Selected>;

/// Allows constructing a `ShardSet` via `slice.into()` instead of calling
/// `ShardSet::new(slice)` explicitly.
impl<
        'a,
        RuneSet: FixedCapacitySet<Item = arch_program::rune::RuneAmount> + Default,
        U: UtxoInfoTrait<RuneSet>,
        S: crate::shard::StateShard<U, RuneSet>,
        const MAX_SELECTED_SHARDS: usize,
    > From<&'a mut [&'a mut S]> for ShardSet<'a, RuneSet, U, S, MAX_SELECTED_SHARDS, Unselected>
{
    #[inline]
    fn from(slice: &'a mut [&'a mut S]) -> Self {
        ShardSet::new(slice)
    }
}

// === Helper macro ===================================================================== //
/// Executes `$body` with `set` bound to a [`SelectedShards`] instance that
/// represents the shards specified in `$spec`.
///
/// Example:
/// ```rust,ignore
/// use saturn_account_shards::with_selected_shards;
/// # // `shards` would be a mutable slice of mutable shard references in real code.
/// # let mut shards: Vec<&mut ()> = Vec::new();
/// with_selected_shards!(32, &mut shards[..], [0, 1] => {
///     set.len();
/// });
/// ```
#[macro_export]
macro_rules! with_selected_shards {
    ($max:expr, $shards:expr, $spec:expr => $body:block) => {{
        #[allow(unused_mut)]
        let mut set = $crate::ShardSet::<_, _, $max>::new($shards).select_with($spec);
        let result = { $body };
        result
    }};
}
