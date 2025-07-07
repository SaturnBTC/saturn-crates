use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse_quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields, Ident, Lit, Meta, Type};

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
/// use saturn_account_shards_derive::StateShard;
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
/// #[derive(StateShard)]
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
/// use saturn_account_shards_derive::StateShard;
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
/// #[derive(StateShard)]
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
/// use saturn_account_shards_derive::StateShard;
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
/// #[derive(StateShard)]
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
/// use saturn_account_shards_derive::StateShard;
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
/// #[derive(StateShard)]
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
#[proc_macro_derive(StateShard, attributes(shard))]
pub fn derive_state_shard(input: TokenStream) -> TokenStream {
    derive_state_shard_impl(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn assert_repr_c(shard_ident: &Ident, attrs: &[Attribute]) -> Result<()> {
    // Find the #[repr] attribute
    let repr_attr = attrs
        .iter()
        .find(|attr| attr.path().is_ident("repr"))
        .ok_or_else(|| syn::Error::new_spanned(shard_ident, "Shard struct must be #[repr(C)]"))?;

    // Assume it's not C
    let mut has_repr_c = false;

    // Look for the C
    repr_attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("C") {
            has_repr_c = true;
        }

        // Consume any other content in the stream
        if meta.input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in meta.input);
            content.parse::<proc_macro2::TokenStream>()?;
        }

        Ok(())
    })?;

    // Return Ok or fail
    if has_repr_c {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            shard_ident,
            "Shard struct must be #[repr(C)]",
        ))
    }
}

fn derive_state_shard_impl(input: proc_macro2::TokenStream) -> Result<proc_macro2::TokenStream> {
    // Parse the macro input as a regular Rust derive input (struct/enum etc.)
    let input: DeriveInput = syn::parse2(input)?;

    // Only structs are supported for now
    let struct_ident = &input.ident;

    // Ensure the struct is #[repr(C)] for FFI-safe layout
    assert_repr_c(struct_ident, &input.attrs)?;

    // Try to locate a custom #[shard(...)] attribute. It is optional – when absent we fall
    // back to the built-in defaults (btc_utxos, rune_utxo, built-in helper types).
    let shard_attr_opt = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("shard"));

    // Defaults
    let mut btc_utxos_field_ident = Ident::new("btc_utxos", struct_ident.span());
    let mut rune_utxo_field_ident = Ident::new("rune_utxo", struct_ident.span());
    let mut utxo_info_type: Option<Type> = None;
    let mut rune_set_type: Option<Type> = None;
    let mut fixed_option_type: Option<Type> = None;

    if let Some(shard_attr) = shard_attr_opt {
        // Parse the attribute meta list: #[shard(key = "value", ...)]
        shard_attr.parse_nested_meta(|meta| {
            // For each key=value pair
            let key_path = meta.path.clone(); // Path of the key
            if meta.input.peek(syn::Token![=]) {
                // Consume '='
                let _eq_token: syn::Token![=] = meta.input.parse()?;
                // Parse the value as a literal string
                let lit: Lit = meta.input.parse()?;
                let lit_str = if let Lit::Str(s) = lit {
                    s.value()
                } else {
                    return Err(syn::Error::new_spanned(lit, "Expected string literal"));
                };

                if key_path.is_ident("btc_utxos_attr") {
                    btc_utxos_field_ident = format_ident!("{}", lit_str);
                } else if key_path.is_ident("rune_utxo_attr") {
                    rune_utxo_field_ident = format_ident!("{}", lit_str);
                } else if key_path.is_ident("utxo_info_type") {
                    utxo_info_type = Some(syn::parse_str::<Type>(&lit_str)?);
                } else if key_path.is_ident("rune_set_type") {
                    rune_set_type = Some(syn::parse_str::<Type>(&lit_str)?);
                } else if key_path.is_ident("fixed_option_type") {
                    fixed_option_type = Some(syn::parse_str::<Type>(&lit_str)?);
                } else {
                    return Err(syn::Error::new_spanned(
                        key_path,
                        "Unknown key in #[shard] attribute",
                    ));
                }
            }
            Ok(())
        })?;
    }

    // NEW: figure out the concrete type of the `rune_utxo` field so we can
    //       fall back to it when the user does not explicitly specify a
    //       `fixed_option_type`.
    let rune_utxo_field_type = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named.named.iter().find_map(|f| {
                f.ident.as_ref().and_then(|id| {
                    if id == &rune_utxo_field_ident {
                        Some(f.ty.clone())
                    } else {
                        None
                    }
                })
            }),
            _ => None,
        },
        _ => None,
    }
    .ok_or_else(|| {
        syn::Error::new_spanned(
            &rune_utxo_field_ident,
            format!("Could not find field `{}` in struct", rune_utxo_field_ident),
        )
    })?;

    // Provide sensible defaults when the developer omits them. All defaults live in
    // `saturn_bitcoin_transactions::utxo_info` unless we can infer them from the
    // struct itself.
    let utxo_info_type: Type = match utxo_info_type {
        Some(t) => t,
        None => syn::parse_str("saturn_bitcoin_transactions::utxo_info::UtxoInfo")?,
    };

    let rune_set_type: Type = match rune_set_type {
        Some(t) => t,
        None => syn::parse_str("saturn_bitcoin_transactions::utxo_info::SingleRuneSet")?,
    };

    // Default to the concrete option-wrapper type used in the struct if the user
    // did not override it explicitly via the `fixed_option_type` attribute.
    let fixed_option_type: Type = match fixed_option_type {
        Some(t) => t,
        None => rune_utxo_field_type.clone(),
    };

    // Generate the implementation for StateShard
    let expanded = quote! {
        impl saturn_account_shards::StateShard<#utxo_info_type, #rune_set_type> for #struct_ident {
            fn btc_utxos(&self) -> &[#utxo_info_type] {
                self.#btc_utxos_field_ident.as_slice()
            }

            fn btc_utxos_mut(&mut self) -> &mut [#utxo_info_type] {
                self.#btc_utxos_field_ident.as_mut_slice()
            }

            fn btc_utxos_retain(&mut self, f: &mut dyn FnMut(&#utxo_info_type) -> bool) {
                self.#btc_utxos_field_ident.retain(f);
            }

            fn add_btc_utxo(&mut self, utxo: #utxo_info_type) -> Option<usize> {
                self.#btc_utxos_field_ident.add(utxo)
            }

            fn btc_utxos_len(&self) -> usize {
                self.#btc_utxos_field_ident.len()
            }

            fn btc_utxos_max_len(&self) -> usize {
                self.#btc_utxos_field_ident.capacity()
            }

            fn rune_utxo(&self) -> Option<&#utxo_info_type> {
                self.#rune_utxo_field_ident.as_ref()
            }

            fn rune_utxo_mut(&mut self) -> Option<&mut #utxo_info_type> {
                self.#rune_utxo_field_ident.as_mut()
            }

            fn clear_rune_utxo(&mut self) {
                self.#rune_utxo_field_ident = #fixed_option_type::none();
            }

            fn set_rune_utxo(&mut self, utxo: #utxo_info_type) {
                self.#rune_utxo_field_ident = #fixed_option_type::some(utxo);
            }
        }
    };

    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::parse_quote;

    fn test_macro_expansion(input: TokenStream) -> Result<TokenStream> {
        derive_state_shard_impl(input)
    }

    #[test]
    fn test_basic_struct_with_defaults() {
        let input = quote! {
            #[repr(C)]
            struct TestShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Check that the implementation contains the expected method signatures
        assert!(output_str.contains("impl saturn_account_shards :: StateShard"));
        assert!(output_str.contains("fn btc_utxos (& self)"));
        assert!(output_str.contains("fn btc_utxos_mut (& mut self)"));
        assert!(output_str.contains("fn btc_utxos_retain"));
        assert!(output_str.contains("fn add_btc_utxo"));
        assert!(output_str.contains("fn btc_utxos_len"));
        assert!(output_str.contains("fn btc_utxos_max_len"));
        assert!(output_str.contains("fn rune_utxo"));
        assert!(output_str.contains("fn rune_utxo_mut"));
        assert!(output_str.contains("fn clear_rune_utxo"));
        assert!(output_str.contains("fn set_rune_utxo"));
    }

    #[test]
    fn test_custom_field_names() {
        let input = quote! {
            #[shard(btc_utxos_attr = "bitcoin_utxos", rune_utxo_attr = "rune_utxo_data")]
            #[repr(C)]
            struct CustomShard {
                pub pool_id: u64,
                pub bitcoin_utxos: CustomBtcUtxos,
                pub rune_utxo_data: CustomRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Check that custom field names are used
        assert!(output_str.contains("self . bitcoin_utxos"));
        assert!(output_str.contains("self . rune_utxo_data"));
    }

    #[test]
    fn test_custom_types() {
        let input = quote! {
            #[shard(
                utxo_info_type = "CustomUtxoInfo",
                rune_set_type = "CustomRuneSet",
                fixed_option_type = "CustomFixedOption"
            )]
            #[repr(C)]
            struct CustomTypeShard {
                pub pool_id: u64,
                pub btc_utxos: CustomBtcUtxos,
                pub rune_utxo: CustomRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Check that custom types are used
        assert!(output_str.contains("StateShard < CustomUtxoInfo , CustomRuneSet >"));
        assert!(output_str.contains("CustomFixedOption :: none"));
        assert!(output_str.contains("CustomFixedOption :: some"));
    }

    #[test]
    fn test_repr_c_required() {
        let input = quote! {
            struct NoReprShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Shard struct must be #[repr(C)]"));
    }

    #[test]
    fn test_repr_c_with_other_attrs() {
        let input = quote! {
            #[repr(C, align(8))]
            struct AlignedShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_repr() {
        let input = quote! {
            #[repr(packed)]
            struct PackedShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Shard struct must be #[repr(C)]"));
    }

    #[test]
    fn test_unknown_shard_attribute() {
        let input = quote! {
            #[shard(unknown_attr = "value")]
            #[repr(C)]
            struct UnknownAttrShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Unknown key in #[shard] attribute"));
    }

    #[test]
    fn test_non_string_literal_in_attribute() {
        let input = quote! {
            #[shard(btc_utxos_attr = 42)]
            #[repr(C)]
            struct NonStringAttrShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Expected string literal"));
    }

    #[test]
    fn test_all_custom_attributes() {
        let input = quote! {
            #[shard(
                btc_utxos_attr = "my_btc_utxos",
                rune_utxo_attr = "my_rune_utxo",
                utxo_info_type = "MyUtxoInfo",
                rune_set_type = "MyRuneSet",
                fixed_option_type = "MyFixedOption"
            )]
            #[repr(C)]
            struct FullCustomShard {
                pub pool_id: u64,
                pub my_btc_utxos: MyBtcUtxos,
                pub my_rune_utxo: MyRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Check all custom attributes are applied
        assert!(output_str.contains("self . my_btc_utxos"));
        assert!(output_str.contains("self . my_rune_utxo"));
        assert!(output_str.contains("StateShard < MyUtxoInfo , MyRuneSet >"));
        assert!(output_str.contains("MyFixedOption :: none"));
        assert!(output_str.contains("MyFixedOption :: some"));
    }

    #[test]
    fn test_complex_struct_with_additional_fields() {
        let input = quote! {
            #[derive(Debug, Clone)]
            #[repr(C)]
            struct ComplexShard {
                pub pool_id: u64,
                pub liquidity: u128,
                pub fee_rate: u32,
                pub last_update: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
                pub metadata: [u8; 32],
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Verify the implementation is generated correctly even with additional fields
        assert!(output_str.contains("impl saturn_account_shards :: StateShard"));
        assert!(output_str.contains("self . btc_utxos"));
        assert!(output_str.contains("self . rune_utxo"));
    }

    #[test]
    fn test_mixed_custom_and_default_attributes() {
        let input = quote! {
            #[shard(btc_utxos_attr = "custom_btc_utxos")]
            #[repr(C)]
            struct MixedShard {
                pub pool_id: u64,
                pub custom_btc_utxos: CustomBtcUtxos,
                pub rune_utxo: DefaultRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Check that custom field name is used for btc_utxos
        assert!(output_str.contains("self . custom_btc_utxos"));
        // Check that default field name is used for rune_utxo
        assert!(output_str.contains("self . rune_utxo"));
        // Check that default types are used
        assert!(output_str.contains("saturn_bitcoin_transactions :: utxo_info :: UtxoInfo"));
    }

    #[test]
    fn test_default_types_in_output() {
        let input = quote! {
            #[repr(C)]
            struct DefaultTypesShard {
                pub pool_id: u64,
                pub btc_utxos: DefaultBtcUtxos,
                pub rune_utxo: DefaultRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Check that default types are used
        assert!(output_str.contains("saturn_bitcoin_transactions :: utxo_info :: UtxoInfo"));
        assert!(output_str.contains("saturn_bitcoin_transactions :: utxo_info :: SingleRuneSet"));
        // The rune UTXO option type defaults to the concrete field type in the
        // struct, so we no longer expect a hard-coded `FixedOptionUtxoInfo` path
        // here.
    }

    #[test]
    fn test_attribute_parsing_edge_cases() {
        // Test empty shard attribute
        let input = quote! {
            #[shard()]
            #[repr(C)]
            struct EmptyAttrShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_method_generation_correctness() {
        let input = quote! {
            #[repr(C)]
            struct MethodTestShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Verify all required methods are present with correct signatures
        assert!(output_str.contains("fn btc_utxos (& self) -> & ["));
        assert!(output_str.contains("fn btc_utxos_mut (& mut self) -> & mut ["));
        assert!(output_str.contains("fn btc_utxos_retain (& mut self , f : & mut dyn FnMut"));
        assert!(output_str.contains("fn add_btc_utxo (& mut self , utxo :"));
        assert!(output_str.contains("fn btc_utxos_len (& self) -> usize"));
        assert!(output_str.contains("fn btc_utxos_max_len (& self) -> usize"));
        assert!(output_str.contains("fn rune_utxo (& self) -> Option"));
        assert!(output_str.contains("fn rune_utxo_mut (& mut self) -> Option"));
        assert!(output_str.contains("fn clear_rune_utxo (& mut self)"));
        assert!(output_str.contains("fn set_rune_utxo (& mut self , utxo :"));

        // Verify correct method implementations
        assert!(output_str.contains("as_slice ()"));
        assert!(output_str.contains("as_mut_slice ()"));
        assert!(output_str.contains("retain (f)"));
        assert!(output_str.contains("add (utxo)"));
        assert!(output_str.contains("len ()"));
        assert!(output_str.contains("capacity ()"));
        assert!(output_str.contains("as_ref ()"));
        assert!(output_str.contains("as_mut ()"));
        assert!(output_str.contains(":: none ()"));
        assert!(output_str.contains(":: some (utxo)"));
    }

    #[test]
    fn test_generic_type_parsing() {
        let input = quote! {
            #[shard(
                utxo_info_type = "MyUtxoInfo<MyRuneSet>",
                rune_set_type = "MyRuneSet"
            )]
            #[repr(C)]
            struct GenericTypeShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Check that generic types are handled correctly
        assert!(output_str.contains("MyUtxoInfo < MyRuneSet >"));
        assert!(output_str.contains("StateShard < MyUtxoInfo < MyRuneSet > , MyRuneSet >"));
    }

    #[test]
    fn test_qualified_type_paths() {
        let input = quote! {
            #[shard(
                utxo_info_type = "crate::custom::UtxoInfo",
                rune_set_type = "crate::custom::RuneSet",
                fixed_option_type = "crate::custom::FixedOption"
            )]
            #[repr(C)]
            struct QualifiedTypeShard {
                pub pool_id: u64,
                pub btc_utxos: TestBtcUtxos,
                pub rune_utxo: TestRuneUtxo,
            }
        };

        let result = test_macro_expansion(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = output.to_string();

        // Check that qualified type paths are preserved
        assert!(output_str.contains("crate :: custom :: UtxoInfo"));
        assert!(output_str.contains("crate :: custom :: RuneSet"));
        assert!(output_str.contains("crate :: custom :: FixedOption"));
    }
}
