#![allow(dead_code)]
//! Code generation for the `UtxoParser` IR.

pub mod extractors;
pub mod predicate;

use crate::ir::{DeriveInputIr, RunesPresence};
use quote::quote;

/// Build the predicate TokenStream for a field, applying the implicit rule
/// that `anchor = ...` implies `runes == none` when the user did not provide a
/// runes constraint.  This preserves legacy semantics without modifying the
/// parsing stage.
fn build_predicate_with_anchor_logic(field: &crate::ir::Field) -> proc_macro2::TokenStream {
    let mut attr = field.attr.clone();
    if attr.anchor_ident.is_some() && attr.runes.is_none() {
        attr.runes = Some(RunesPresence::None);
    }
    crate::codegen::predicate::build(&attr)
}

/// Assemble the final `TokenStream` implementing `TryFromUtxos` for the target
/// struct.  The generated code mirrors the behaviour of the original
/// `derive_utxo_parser_old` implementation while being driven by the new IR /
/// modular design.
pub fn expand(ir: &DeriveInputIr) -> proc_macro2::TokenStream {
    let struct_ident = &ir.struct_ident;
    let accounts_ty = &ir.accounts_ty;

    // Extract the type-level generics (`<T, 'a, ..>` → used as #ty_generics).
    let (_, ty_generics, _) = ir.generics.split_for_impl();

    // ---------------------------------------------------------------
    // Build extraction snippets in declaration order.
    // ---------------------------------------------------------------
    let mut init_snippets: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut field_idents: Vec<&syn::Ident> = Vec::new();

    // Initialise the `remaining` vector.
    init_snippets.push(quote! {
        let mut remaining: Vec<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo> =
            utxos.iter().collect();
    });

    for field in &ir.fields {
        field_idents.push(&field.ident);
        let predicate_ts = build_predicate_with_anchor_logic(field);
        let extractor_ts = crate::codegen::extractors::build_extractor(field, &predicate_ts);
        init_snippets.push(extractor_ts);
    }

    // Check for leftover inputs after all fields have extracted theirs.
    init_snippets.push(quote! {
        if !remaining.is_empty() {
            return Err(ProgramError::Custom(ErrorCode::UnexpectedExtraUtxos.into()));
        }
    });

    // ---------------------------------------------------------------
    // Compose the final impl block.
    // ---------------------------------------------------------------
    quote! {
        impl<'a> saturn_utxo_parser::TryFromUtxos<'a> for #struct_ident #ty_generics {
            type Accs = #accounts_ty;

            fn try_utxos(
                accounts: &'a Self::Accs,
                utxos: &'a [saturn_bitcoin_transactions::utxo_info::UtxoInfo],
            ) -> core::result::Result<Self, arch_program::program_error::ProgramError> {
                use arch_program::program_error::ProgramError;
                use saturn_utxo_parser::ErrorCode;

                #(#init_snippets)*

                Ok(Self { #(#field_idents),* })
            }
        }
    }
}