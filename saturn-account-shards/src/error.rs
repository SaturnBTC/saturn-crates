use saturn_collections::generic::fixed_set::FixedSetError;
use saturn_error::saturn_error;

/// Errors that can occur when manipulating a set of `StateShard` instances.
///
/// These variants intentionally do **not** hold additional data so that the
/// type can be marked `Copy` and embedded into other error types with zero-cost.
#[saturn_error(offset = 300)]
#[derive(Debug, PartialEq)]
pub enum StateShardError {
    /// A rune transfer required more runes than were actually present across
    /// the shards involved in the operation.
    #[error("Not enough rune in shards")]
    NotEnoughRuneInShards,
    /// A runestone edict refers to an output index that is **not** part of the
    /// transaction we are processing.
    #[error("Output edict is not in transaction")]
    OutputEdictIsNotInTransaction,

    #[error("Math error in btc amount")]
    MathErrorInBalanceAmountAcrossShards,

    /// Too many runes in utxo
    ///
    /// This error is returned when the total amount of runes in the utxo is
    /// greater than the maximum allowed amount of runes in a utxo.
    #[error("Too many runes in utxo")]
    TooManyRunesInUtxo,

    #[error("Rune amount addition overflow")]
    RuneAmountAdditionOverflow,

    #[error("Shards are full of btc utxos")]
    ShardsAreFullOfBtcUtxos,

    #[error("Removing more runes than are present in the shards")]
    RemovingMoreRunesThanPresentInShards,

    /// The runestone did not contain the mandatory pointer field.
    #[error("Missing pointer in runestone")]
    MissingPointerInRunestone,

    /// The pointer specified inside the runestone does not correspond to any
    /// output created by the transaction.
    #[error("Runestone pointer is not in transaction")]
    RunestonePointerIsNotInTransaction,
}

impl From<FixedSetError> for StateShardError {
    fn from(error: FixedSetError) -> Self {
        match error {
            FixedSetError::Full => StateShardError::TooManyRunesInUtxo,
            FixedSetError::Duplicate => {
                panic!("unreachable. we couldn't have a duplicate rune input")
            }
        }
    }
}

/// Convenience alias used throughout the crate so functions can simply return
/// `Result<T>` instead of writing out the full type every time.
pub type Result<T> = core::result::Result<T, StateShardError>;
