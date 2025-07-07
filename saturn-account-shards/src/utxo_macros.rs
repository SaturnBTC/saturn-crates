#[macro_export]
macro_rules! declare_shard_utxo_types {
    // Declare both an array type (fixed-capacity) and a fixed-option type for
    // `saturn_bitcoin_transactions::utxo_info::UtxoInfo<RuneSet>`.
    //
    // Usage:
    //     declare_utxo_info_types!(
    //         MyRuneSet,          // The concrete RuneSet type
    //         BtcUtxos,           // Name of the array type to generate
    //         RuneUtxo,           // Name of the option type to generate
    //         25,                 // Capacity of the array
    //         15                  // Padding for the option type (see below)
    //     );
    //
    // This expands to two `#[repr(C)]` structs – `BtcUtxos` and `RuneUtxo` –
    // that can be embedded in on-chain account state and used with
    // `saturn_account_shards::StateShard`.
    //
    // Parameters
    // ----------
    // 1. `$RuneSet:ty` – Concrete RuneSet type implementing `FixedCapacitySet`.
    // 2. `$ArrayName:ident` – Identifier for the generated fixed-array type.
    // 3. `$OptionName:ident` – Identifier for the generated fixed-option type.
    // 4. `$SIZE:expr` – Maximum number of BTC-UTXOs the array can store.
    // 5. `$PADDING:expr` – Padding bytes for the option struct so that
    //    `size_of::<$T>() + 1 + $PADDING` is a multiple of 8/16.  When in
    //    doubt you can pick `15`, which yields 8-byte alignment for the vast
    //    majority of `UtxoInfo` sizes.
    (
        $RuneSet:ty,
        $ArrayName:ident,
        $OptionName:ident,
        $SIZE:expr,
        $PADDING:expr $(,)?
    ) => {
        // Bring helper macros into scope explicitly so users do not have to
        // import them at the call site.
        $crate::declare_fixed_array! {
            $ArrayName,
            saturn_bitcoin_transactions::utxo_info::UtxoInfo<$RuneSet>,
            $SIZE
        }

        $crate::declare_fixed_option! {
            $OptionName,
            saturn_bitcoin_transactions::utxo_info::UtxoInfo<$RuneSet>,
            $PADDING
        }
    };

    // ------------------------------------------------------------------
    // Extended variant: declare a `FixedSet` Rune set inside the same macro
    // invocation.  This lets callers avoid writing an extra `type` or
    // `declare_fixed_set!` statement beforehand.
    //
    // Usage:
    //     declare_shard_utxo_types!(
    //         RuneAmount, 2,         // element type and capacity => generates the RuneSet
    //         MultiRuneSet,          // name for the generated RuneSet type
    //         BtcUtxos, RuneUtxo,    // BTC array and Rune option helper types
    //         25, 15                 // array capacity and option padding
    //     );
    (
        $RuneElem:ty,
        $RUNE_CAP:expr,
        $RuneSetName:ident,
        $ArrayName:ident,
        $OptionName:ident,
        $SIZE:expr,
        $PADDING:expr $(,)?
    ) => {
        // First, generate the fixed-capacity RuneSet struct using the helper
        // macro re-exported by this crate.
        $crate::declare_fixed_set!($RuneSetName, $RuneElem, $RUNE_CAP);

        // Next, invoke the original five-argument rule to create the BTC UTXO
        // array and Rune UTXO option wrappers that use the freshly declared
        // RuneSet type.
        $crate::declare_shard_utxo_types!($RuneSetName, $ArrayName, $OptionName, $SIZE, $PADDING);
    };
}
