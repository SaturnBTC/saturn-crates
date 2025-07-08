mod args;
mod expand;
mod struct_info;
mod utils;
mod validate;

use proc_macro::TokenStream;
use syn::DeriveInput;

type Result<T> = std::result::Result<T, syn::Error>;

/// Derive macro for automatically implementing the `StateShard` trait.
///
/// This macro generates an implementation of `StateShard` for structs that manage
/// Bitcoin and Rune UTXO collections in on-chain account state.
///
/// # Requirements
///
/// - The struct must be `#[repr(C)]` for FFI-safe memory layout
/// - The struct must have named fields
/// - The struct cannot be generic
/// - The struct must contain fields for BTC UTXOs (array) and Rune UTXOs (option)
///
/// # Basic Usage
///
/// ```rust,ignore
/// use saturn_account_shards_derive::ShardAccount;
/// use saturn_account_shards::declare_shard_utxo_types;
///
/// // First declare the UTXO collection types
/// declare_shard_utxo_types!(
///     MyRuneSet,    // Your rune set type
///     BtcUtxos,     // Name for BTC UTXO array type
///     RuneUtxo,     // Name for Rune UTXO option type
///     25,           // BTC UTXO capacity
///     15            // Padding for alignment
/// );
///
/// #[derive(ShardAccount)]
/// #[repr(C)]
/// pub struct MyPool {
///     pub pool_id: u64,
///     pub liquidity: u128,
///     pub btc_utxos: BtcUtxos,
///     pub rune_utxo: RuneUtxo,
/// }
/// ```
///
/// # Advanced Usage with Custom Configuration
///
/// You can customize field names and types using the `#[shard]` attribute:
///
/// ```rust,ignore
/// use saturn_account_shards_derive::ShardAccount;
///
/// use saturn_account_shards::declare_shard_utxo_types;
///
/// declare_shard_utxo_types!(
///     MyRuneSet,    // Your rune set type
///     BtcUtxos,     // Name for BTC UTXO array type
///     RuneUtxo,     // Name for Rune UTXO option type
///     25,           // BTC UTXO capacity
///     15            // Padding for alignment
/// );
///
/// #[derive(ShardAccount)]
/// #[shard(
///     btc_utxos_attr = "my_btc_utxos",
///     rune_utxo_attr = "my_rune_utxo",
///     utxo_info_type = "BtcUtxos", // FixedArrayUtxoInfo is the default
///     rune_set_type = "MyRuneSet", // SingleRuneSet is the default
///     fixed_option_type = "RuneUtxo" // FixedOptionUtxoInfo is the default
/// )]
/// #[repr(C)]
/// pub struct CustomPool {
///     pub pool_id: u64,
///     pub my_btc_utxos: BtcUtxos,
///     pub my_rune_utxo: RuneUtxo,
/// }
/// ```
///
/// # Shard Attribute Parameters
///
/// The `#[shard]` attribute supports the following optional parameters:
///
/// - `btc_utxos_attr`: Name of the field containing BTC UTXOs (default: "btc_utxos")
/// - `rune_utxo_attr`: Name of the field containing Rune UTXOs (default: "rune_utxo")
/// - `utxo_info_type`: Type of UTXO info objects (default: "saturn_bitcoin_transactions::utxo_info::UtxoInfo")
/// - `rune_set_type`: Type of rune set (default: "saturn_bitcoin_transactions::utxo_info::SingleRuneSet")
/// - `fixed_option_type`: Type of fixed option for rune UTXOs (default: "saturn_bitcoin_transactions::utxo_info::FixedOptionUtxoInfo")
///
/// # Generated Implementation
///
/// The macro generates a complete implementation of `StateShard` that provides:
///
/// - `btc_utxos()` / `btc_utxos_mut()`: Access to BTC UTXO array
/// - `btc_utxos_retain()`: Filter BTC UTXOs in-place
/// - `add_btc_utxo()`: Add a new BTC UTXO
/// - `btc_utxos_len()` / `btc_utxos_max_len()`: Size information
/// - `rune_utxo()` / `rune_utxo_mut()`: Access to optional Rune UTXO
/// - `clear_rune_utxo()` / `set_rune_utxo()`: Manage Rune UTXO
///
/// # Examples
///
/// ## Simple Pool with Default Configuration
///
/// ```rust,ignore
/// use saturn_account_shards_derive::ShardAccount;
/// use saturn_account_shards::declare_shard_utxo_types;
///
/// // Declare collection types for a simple rune set
/// declare_shard_utxo_types!(
///     SimpleRuneSet,
///     PoolBtcUtxos,
///     PoolRuneUtxo,
///     50,  // Can hold up to 50 BTC UTXOs
///     15   // Padding for alignment
/// );
///
/// #[derive(ShardAccount)]
/// #[shard(
///     utxo_info_type = "PoolBtcUtxos",
///     rune_set_type = "SimpleRuneSet",
///     fixed_option_type = "PoolRuneUtxo"
/// )]
/// #[repr(C)]
/// pub struct LiquidityPool {
///     pub pool_id: u64,
///     pub total_liquidity: u128,
///     pub fee_rate: u32,
///     pub btc_utxos: PoolBtcUtxos,
///     pub rune_utxo: PoolRuneUtxo,
/// }
/// ```
///
/// ## Advanced Pool with Custom Types
///
/// ```rust,ignore
/// use saturn_account_shards_derive::ShardAccount;
///
/// use saturn_account_shards::declare_shard_utxo_types;
///
/// pub type MultiRuneSet = FixedSet<RuneAmount, 2>;
///
/// declare_shard_utxo_types!(
///     MultiRuneSet,
///     AdvancedBtcUtxoArray,
///     AdvancedRuneUtxoOption,
///     25,
///     15
/// );
///
/// #[derive(ShardAccount)]
/// #[shard(
///     btc_utxos_attr = "bitcoin_utxos",
///     rune_utxo_attr = "rune_utxo_data",
///     utxo_info_type = "AdvancedUtxoInfo",
///     rune_set_type = "MultiRuneSet",
///     fixed_option_type = "AdvancedRuneUtxoOption"
/// )]
/// #[repr(C)]
/// pub struct AdvancedPool {
///     pub pool_id: u64,
///     pub bitcoin_utxos: AdvancedBtcUtxoArray,
///     pub rune_utxo_data: AdvancedRuneUtxoOption,
///     pub metadata: PoolMetadata,
/// }
/// ```
///
/// # Error Handling
///
/// The macro will generate compile-time errors for:
/// - Missing `#[repr(C)]` attribute
/// - Invalid field names in `#[shard]` attributes
/// - Invalid type specifications
/// - Structs that don't meet the requirements

/// High-level derive that turns a plain struct into a fully-featured *shard account*.
///
/// It automatically:
/// 1. Validates the struct is `#[repr(C)]`.
/// 2. Generates `impl StateShard<..>` (delegating to the regular derive).
/// 3. Generates the unsafe `bytemuck::Zeroable` / `bytemuck::Pod` impls required for
///    zero-copy account access.
///
/// All `#[shard(..)]` customization attributes supported by `StateShard` are also
/// valid here.
///
/// Usage:
/// ```ignore
/// #[derive(ShardAccount)]
/// #[repr(C)]
/// #[shard(btc_utxos_attr = "utxos")]
/// pub struct MyShard { .. }
/// ```
#[proc_macro_derive(ShardAccount, attributes(shard))]
pub fn derive_shard_account(input: TokenStream) -> TokenStream {
    crate::expand::shard_account::derive_shard_account(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

// Temporary shim so the existing unit tests inside this crate (marked with `#[cfg(test)]`)
// can still invoke the legacy implementation path. **Note:** this helper is kept private
// (crate-visible only) so it does not violate the `proc-macro` crate export rules.
fn derive_state_shard_impl(input: proc_macro2::TokenStream) -> Result<proc_macro2::TokenStream> {
    // Perform repr(C) validation to keep legacy tests intact
    let di: DeriveInput = syn::parse2(input.clone())?;
    crate::validate::assert_repr_c(&di.ident, &di.attrs)?;
    crate::expand::state_shard::derive_state_shard(input)
}

#[cfg(test)]
pub(crate) fn expand_state_shard_test_helper(
    input_ts: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    crate::expand::state_shard::derive_state_shard(input_ts)
}
