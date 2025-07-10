#![allow(dead_code)]

use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Lit, Type};

use crate::{args::ShardArgs, struct_info::StructInfo};

pub type Result<T> = syn::Result<T>;

/// Stand-alone code-generation entry that produces the `impl StateShard<…>`.
/// This was extracted from the monolithic implementation in `lib.rs`.
pub fn derive_state_shard(input: proc_macro2::TokenStream) -> Result<proc_macro2::TokenStream> {
    // Parse derive input and gather high-level struct information
    let derive_input: DeriveInput = syn::parse2(input.clone())?;
    let info = StructInfo::from_derive_input(derive_input)?;

    // Parse customization attributes
    let shard_args = ShardArgs::from_attrs(&info.attrs)?;

    // Resolve defaults / delegate to old generator
    generate_impl(&info, &shard_args)
}

fn generate_impl(info: &StructInfo, args: &ShardArgs) -> Result<proc_macro2::TokenStream> {
    let struct_ident = &info.ident;

    // Extract customizations or fall back to defaults
    let btc_utxos_field_ident = &args.btc_utxos_ident;
    let rune_utxo_field_ident = &args.rune_utxo_ident;

    // Infer rune_utxo field type if needed
    let rune_utxo_field_type = info.field_type(rune_utxo_field_ident).ok_or_else(|| {
        syn::Error::new_spanned(rune_utxo_field_ident, "Could not find rune_utxo field")
    })?;

    // Provide sensible defaults when types are not overridden
    let (rune_set_type, validation_token): (Type, proc_macro2::TokenStream) =
        if let Some(custom) = &args.rune_set_ty {
            (custom.clone(), quote! {})
        } else {
            // Fall back to the default alias defined by the `#[program]` macro **and**
            // produce a friendly compile-time error if that alias is missing.
            (
                // Look for the alias in the current or any ancestor module rather than requiring
                // it to live in crate root. This matches the new behaviour of the saturn_program
                // macro which emits the alias next to the annotated module.
                syn::parse_quote!(crate::__SaturnDefaultRuneSet),
                quote! {
                    // This constant fails to compile with a descriptive error message if the
                    // `__SaturnDefaultRuneSet` alias is not available in the current crate.
                    const _: () = {
                        trait _SaturnDefaultRuneSetAvailable {}
                        // The impl below will only succeed if the alias can be resolved. If not,
                        // the compiler produces an error pointing here, prompting the developer
                        // to either bring the alias into scope (via the `#[program]` macro) or
                        // specify `#[shard(rune_set_type = "...")]` explicitly.
                        impl _SaturnDefaultRuneSetAvailable for crate::__SaturnDefaultRuneSet {}
                    };
                },
            )
        };

    let utxo_info_type: Type = match &args.utxo_info_ty {
        Some(t) => t.clone(),
        None => {
            let rune_ts = quote! { #rune_set_type };
            let ty_str = format!(
                "saturn_bitcoin_transactions::utxo_info::UtxoInfo<{}>",
                rune_ts.to_string().replace(' ', "")
            );
            syn::parse_str(&ty_str)?
        }
    };

    let fixed_option_type: Type = args
        .fixed_option_ty
        .clone()
        .unwrap_or(rune_utxo_field_type.clone());

    // Generate implementation token stream
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

        #validation_token
    };

    Ok(expanded)
}
