#![allow(dead_code)]
//! Snippets that pull matching UTXOs from the `remaining` vector for each `FieldKind`.
//!
//! This file now contains **full** generator routines that emit exactly the same
//! extraction semantics that the original `derive_utxo_parser_old` macro
//! provided, but working from the crate-internal IR.  The implementation is
//! intentionally verbose so that the generated source mirrors the proven logic
//! one-to-one.

use crate::codegen::predicate;
use crate::ir::{Field, FieldKind, RunesPresence};
use quote::{format_ident, quote};

/// Helper: choose the `ErrorCode` variant that should be used when the field
/// fails to match **without** needing the specialised RuneId/RuneAmount logic.
fn base_error_variant(attr: &crate::ir::UtxoAttr) -> proc_macro2::TokenStream {
    // Anchored fields implicitly require `runes == none` even if the user did
    // not specify the `runes` flag.  Therefore their failure mode should be
    // `InvalidRunesPresence` when the predicate does not match.
    if attr.anchor_ident.is_some() && attr.runes.is_none() {
        return quote! { ErrorCode::InvalidRunesPresence };
    }
    if attr.rune_id_expr.is_some() {
        quote! { ErrorCode::InvalidRuneId }
    } else if attr.rune_amount_expr.is_some() {
        quote! { ErrorCode::InvalidRuneAmount }
    } else if attr.runes.is_some() {
        quote! { ErrorCode::InvalidRunesPresence }
    } else if attr.value.is_some() {
        quote! { ErrorCode::InvalidUtxoValue }
    } else {
        quote! { ErrorCode::MissingRequiredUtxo }
    }
}

/// Build the `TokenStream` that initialises the given field using a variable
/// named `remaining` (`Vec<&UtxoInfo>`) and assuming a variable `accounts` in
/// scope.  `predicate` **must** be an expression that can be evaluated for a
/// `utxo` identifier.
pub fn build_extractor(
    field: &Field,
    predicate: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let ident = &field.ident;
    let attr = &field.attr;
    let err_variant = base_error_variant(attr);

    match field.kind {
        // ------------------------------------------------------------------
        // Single reference `&'a UtxoInfo`
        // ------------------------------------------------------------------
        FieldKind::Single => {
            // Anchor handling (executed after successful extraction).
            let anchor_snippet = if let Some(anchor_ident) = &attr.anchor_ident {
                let anchor_ident_tok = anchor_ident.clone();
                quote! {
                    let _anchor_target = &accounts.#anchor_ident_tok;
                    let _anchor_ix = arch_program::system_instruction::anchor(
                        saturn_account_parser::ToAccountInfo::to_account_info(&_anchor_target).key,
                        #ident.meta.txid_big_endian(),
                        #ident.meta.vout(),
                    );
                }
            } else {
                quote! {}
            };

            // Specialised rune-id vs rune-amount diagnostics.
            let specialised_error_logic =
                if attr.rune_id_expr.is_some() && attr.rune_amount_expr.is_some() {
                    // Build predicate that checks *id only* (drop amount).
                    let mut id_only_attr = attr.clone();
                    id_only_attr.rune_amount_expr = None;
                    let id_only_pred = predicate::build(&id_only_attr);
                    quote! {
                        let has_id_match = remaining.iter().any(|utxo| { #id_only_pred });
                        if has_id_match {
                            return Err(ProgramError::Custom(ErrorCode::InvalidRuneAmount.into()));
                        } else {
                            return Err(ProgramError::Custom(ErrorCode::InvalidRuneId.into()));
                        }
                    }
                } else {
                    quote! { return Err(ProgramError::Custom(#err_variant.into())); }
                };

            quote! {
                let pos_opt = remaining.iter().position(|utxo| { #predicate });
                let #ident = if let Some(idx) = pos_opt {
                    remaining.remove(idx)
                } else {
                    #specialised_error_logic
                };
                #anchor_snippet
            }
        }
        // ------------------------------------------------------------------
        // Optional reference `Option<&'a UtxoInfo>`
        // ------------------------------------------------------------------
        FieldKind::Optional => {
            let anchor_snippet = if let Some(anchor_ident) = &attr.anchor_ident {
                let anchor_ident_tok = anchor_ident.clone();
                quote! {
                    if let Some(__opt_utxo) = #ident {
                        let _anchor_target = &accounts.#anchor_ident_tok;
                        let _anchor_ix = arch_program::system_instruction::anchor(
                            saturn_account_parser::ToAccountInfo::to_account_info(&_anchor_target).key,
                            __opt_utxo.meta.txid_big_endian(),
                            __opt_utxo.meta.vout(),
                        );
                    }
                }
            } else {
                quote! {}
            };

            quote! {
                let pos_opt = remaining.iter().position(|utxo| { #predicate });
                let #ident = if let Some(idx) = pos_opt {
                    Some(remaining.remove(idx))
                } else {
                    None
                };
                #anchor_snippet
            }
        }
        // ------------------------------------------------------------------
        // Fixed-length array `[&'a UtxoInfo; N]`
        // ------------------------------------------------------------------
        FieldKind::Array(len) => {
            let len_lit = len as usize; // usize -> literal
            let tmp_ident = format_ident!("{}_tmp", ident);
            let anchor_loop_snippet = if let Some(anchor_ident) = &attr.anchor_ident {
                let anchor_ident_tok = anchor_ident.clone();
                quote! {
                    let _anchor_target = &accounts.#anchor_ident_tok[i];
                    let _anchor_ix = arch_program::system_instruction::anchor(
                        saturn_account_parser::ToAccountInfo::to_account_info(&_anchor_target).key,
                        utxo_ref.meta.txid_big_endian(),
                        utxo_ref.meta.vout(),
                    );
                }
            } else {
                quote! {}
            };

            quote! {
                let mut #tmp_ident: [Option<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo>; #len_lit] = [None; #len_lit];
                for i in 0..#len_lit {
                    let pos_opt = remaining.iter().position(|utxo| { #predicate });
                    let utxo_ref = if let Some(idx) = pos_opt {
                        remaining.remove(idx)
                    } else {
                        return Err(ProgramError::Custom(#err_variant.into()));
                    };
                    #anchor_loop_snippet
                    #tmp_ident[i] = Some(utxo_ref);
                }
                let #ident: [&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo; #len_lit] =
                    #tmp_ident.map(|o| o.unwrap());
            }
        }
        // ------------------------------------------------------------------
        // Vec<&'a UtxoInfo>
        // ------------------------------------------------------------------
        FieldKind::Vec => {
            if let Some(anchor_ident) = &attr.anchor_ident {
                // Vec + anchor (no rest)
                let anchor_ident_tok = anchor_ident.clone();
                quote! {
                    let target_len = accounts.#anchor_ident_tok.len();
                    let mut #ident: Vec<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo> =
                        Vec::with_capacity(target_len);
                    for i in 0..target_len {
                        let pos_opt = remaining.iter().position(|utxo| { #predicate });
                        let utxo_ref = if let Some(idx) = pos_opt {
                            remaining.remove(idx)
                        } else {
                            return Err(ProgramError::Custom(#err_variant.into()));
                        };
                        let _anchor_target = &accounts.#anchor_ident_tok[i];
                        let _anchor_ix = arch_program::system_instruction::anchor(
                            saturn_account_parser::ToAccountInfo::to_account_info(&_anchor_target).key,
                            utxo_ref.meta.txid_big_endian(),
                            utxo_ref.meta.vout(),
                        );
                        #ident.push(utxo_ref);
                    }
                }
            } else if attr.rest {
                // Catch-all rest Vec.
                quote! {
                    let mut #ident: Vec<&'a saturn_bitcoin_transactions::utxo_info::UtxoInfo> = Vec::new();
                    remaining.retain(|utxo| {
                        if { #predicate } {
                            #ident.push(*utxo);
                            false
                        } else { true }
                    });
                }
            } else {
                // Should be unreachable due to validation, but keep a safe-guard.
                quote! { compile_error!("Vec field must be either `rest` or `anchor`."); }
            }
        }
    }
}
