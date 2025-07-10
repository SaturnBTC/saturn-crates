//! Public aliases and helper macro for `ShardSet` ergonomics.
//!
//! Import this as `use saturn_account_shards::prelude::*` to gain the
//! user-friendly façade without pulling in the full type machinery.

use crate::shard_set::{Selected, ShardSet, Unselected};
use bytemuck::{Pod, Zeroable};
use saturn_account_parser::codec::zero_copy::AccountLoader;
use saturn_account_parser::codec::zero_copy::Discriminator;

/// Convenience alias for an **unselected** `ShardSet` in which the typestate
/// parameter is hidden.
pub type Shards<'info, S, const MAX_SELECTED_SHARDS: usize> =
    ShardSet<'info, S, MAX_SELECTED_SHARDS, Unselected>;

/// Convenience alias for a `ShardSet` that already carries an active shard
/// selection.
pub type SelectedShards<'info, S, const MAX_SELECTED_SHARDS: usize> =
    ShardSet<'info, S, MAX_SELECTED_SHARDS, Selected>;

/// Allows constructing a `ShardSet` via `slice.into()` instead of calling
/// `ShardSet::new(slice)` explicitly.
impl<'info, S, const MAX_SELECTED_SHARDS: usize> From<&'info [&'info AccountLoader<'info, S>]>
    for ShardSet<'info, S, MAX_SELECTED_SHARDS, Unselected>
where
    S: Pod + Zeroable + Discriminator + 'static,
{
    #[inline]
    fn from(slice: &'info [&'info AccountLoader<'info, S>]) -> Self {
        ShardSet::from_loaders(slice)
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
    ($max:expr, $loaders:expr, $spec:expr => $body:block) => {{
        let unselected = $crate::ShardSet::<_, $max>::from_loaders($loaders);
        let mut set = unselected
            .select_with($spec)
            .expect("invalid shard selection");
        let result = { $body };
        result
    }};
}
