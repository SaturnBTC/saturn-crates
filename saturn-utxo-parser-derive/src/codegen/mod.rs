pub mod extractors;
pub mod predicate;

use crate::ir::{DeriveInputIr, RunesPresence};
use quote::quote;
use syn::parse_quote;
use syn::{visit::Visit, Lifetime};

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
    // We need two different generic lists:
    //  * `ty_generics` – the generics used on the *type* (`Struct<T>`)
    //  * `impl_generics` – the generics for the `impl` block which must
    //    include a fresh lifetime `'a` required by the `TryFromUtxos` trait
    //    *in addition* to whatever generics the struct already has.

    // (a) Keep the original type generics untouched.
    let (_, ty_generics, _) = ir.generics.split_for_impl();

    // (b) Build a new `impl_generics` *by cloning* the struct generics and
    //     prepending the `'a` lifetime parameter.
    let mut impl_generics_mut = ir.generics.clone();
    // Ensure we only add a single `'a` lifetime. If the user already declared one,
    // reuse it; otherwise inject a fresh parameter at the front.
    let has_a_lifetime = impl_generics_mut.params.iter().any(|param| match param {
        syn::GenericParam::Lifetime(lt) => lt.lifetime.ident == "a",
        _ => false,
    });

    if !has_a_lifetime {
        impl_generics_mut.params.insert(0, parse_quote!('a));
    }
    let (impl_generics, _phantom, where_clause) = impl_generics_mut.split_for_impl();

    // ---------------------------------------------------------------
    // Generate compile-time checks that every `anchor = ident` actually
    // exists on the Accounts struct specified via `#[utxo_accounts(..)]`.
    // ----------------------------------------------------------------
    // Helper to collect distinct lifetimes appearing inside `accounts_ty` so
    // we can declare them in a generic position of the assertion functions.
    fn collect_lifetimes(ty: &syn::Type) -> Vec<syn::Lifetime> {
        struct V<'a> {
            lts: Vec<Lifetime>,
            _marker: std::marker::PhantomData<&'a ()>,
        }
        impl<'ast> Visit<'ast> for V<'_> {
            fn visit_lifetime(&mut self, lt: &'ast Lifetime) {
                // Ignore the special implicit/anonymous lifetimes like `'_'`
                if lt.ident != "_" {
                    // Deduplicate while preserving order (small N)
                    if !self.lts.iter().any(|existing| existing.ident == lt.ident) {
                        self.lts.push(lt.clone());
                    }
                }
            }
        }

        let mut v = V {
            lts: Vec::new(),
            _marker: std::marker::PhantomData,
        };
        v.visit_type(ty);
        v.lts
    }

    // Build one tiny helper fn per unique anchor identifier so the Rust
    // compiler will emit a clean error if the field is missing, *before* we
    // enter the larger impl generated further below.
    let mut anchor_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seen_anchors = std::collections::HashSet::new();
    for field in &ir.fields {
        if let Some(anchor_ident) = &field.attr.anchor_ident {
            // Only one check per unique identifier
            if seen_anchors.insert(anchor_ident.to_string()) {
                let fn_ident = syn::Ident::new(
                    &format!("__saturn_utxo_parser_anchor_check_{}", anchor_ident),
                    anchor_ident.span(),
                );

                let lifetimes = collect_lifetimes(accounts_ty);
                let lt_defs: Vec<_> = lifetimes.iter().map(|lt| quote!(#lt)).collect();

                // Generate a function that takes a reference to the Accounts
                // struct and reads the target field. If the field does not
                // exist the compiler produces an easy-to-understand error
                // that pin-points the missing anchor.
                anchor_checks.push(quote! {
                    #[allow(dead_code)]
                    const _: () = {
                        fn #fn_ident<#(#lt_defs),*>(accs: &#accounts_ty) {
                            let _ = &accs.#anchor_ident;
                        }
                    };
                });
            }
        }
    }

    // ---------------------------------------------------------------
    // Build extraction snippets in declaration order.
    // ---------------------------------------------------------------
    let mut init_snippets: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut field_idents: Vec<&syn::Ident> = Vec::new();

    // ---------------------------------------------------------------
    // Initialise index-based traversal variables and duplicate check.
    // ---------------------------------------------------------------
    init_snippets.push(quote! {
        // Strict-order parsing state
        let mut idx: usize = 0;
        let total: usize = utxos.len();

        // Optional pre-flight duplicate meta detection (cheap O(n^2) because N is small)
        for i in 0..total {
            for j in (i + 1)..total {
                if utxos[i] == utxos[j] {
                    return Err(ProgramError::Custom(ErrorCode::DuplicateUtxoMeta.into()));
                }
            }
        }
    });

    for field in &ir.fields {
        field_idents.push(&field.ident);
        let predicate_ts = build_predicate_with_anchor_logic(field);
        let extractor_ts = crate::codegen::extractors::build_extractor(field, &predicate_ts);
        init_snippets.push(extractor_ts);
    }

    // Check for leftover inputs after all fields have extracted theirs.
    init_snippets.push(quote! {
        if idx < total {
            return Err(ProgramError::Custom(ErrorCode::UnexpectedExtraUtxos.into()));
        }
    });

    // ---------------------------------------------------------------
    // Compose the final impl block.
    // ---------------------------------------------------------------
    quote! {
        // Anchor field existence assertions ----------------------------------------------------
        #( #anchor_checks )*

        impl #impl_generics saturn_utxo_parser::TryFromUtxos<'a> for #struct_ident #ty_generics #where_clause {
            type Accs<'any> = #accounts_ty<'any>;

            fn try_utxos<'accs, 'info2>(
                accounts: &'accs Self::Accs<'info2>,
                utxos: &'a [arch_program::utxo::UtxoMeta],
            ) -> core::result::Result<Self, arch_program::program_error::ProgramError> {
                use arch_program::program_error::ProgramError;
                use saturn_utxo_parser::ErrorCode;

                // Shadow the accounts reference with the correct lifetime for convenience.
                let accounts: &Self::Accs<'info2> = accounts;

                #(#init_snippets)*

                Ok(Self { #(#field_idents),* })
            }
        }
    }
}
