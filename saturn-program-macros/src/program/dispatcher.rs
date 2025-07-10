use convert_case::{Case, Casing};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::LitInt;
use sha2::{Digest, Sha256};

use crate::program::analysis::{AnalysisResult, FnInfo};
use crate::program::attr::AttrConfig;

/// Generates the dispatcher + entrypoint implementation for a `#[saturn_program]` module.
/// The returned `TokenStream` already includes the original (potentially modified)
/// module item.
pub fn generate(attr_cfg: &AttrConfig, analysis: &AnalysisResult) -> TokenStream {
    let module_ident = &analysis.item_mod.ident;

    // ---------------------------------------------------------------------
    // 0. Optional RuneSet alias (identical logic to previous implementation)
    // ---------------------------------------------------------------------
    let rune_alias_ts: Option<TokenStream> = if attr_cfg.enable_bitcoin_tx {
        if let Some(cap) = attr_cfg.btc_tx_cfg.rune_capacity {
            let cap_lit = LitInt::new(&cap.to_string(), Span::call_site());
            Some(quote! {
                #[allow(non_camel_case_types)]
                #[doc(hidden)]
                pub type __SaturnDefaultRuneSet = saturn_collections::generic::fixed_set::FixedSet<
                    arch_program::rune::RuneAmount,
                    #cap_lit
                >;
            })
        } else if let Some(rune_set_path) = &attr_cfg.btc_tx_cfg.rune_set {
            Some(quote! {
                #[doc(hidden)]
                pub type __SaturnDefaultRuneSet = #rune_set_path ;
            })
        } else {
            None
        }
    } else {
        None
    };

    // Mutable copy of the user's module so we can inject helper items.
    let mut item_mod_mut = analysis.item_mod.clone();

    // ---------------------------------------------------------------------
    // 1. Build structs + discriminators (inside hidden module) and dispatcher
    // ---------------------------------------------------------------------

    // Collect struct definitions and dispatcher match arms
    let mut struct_defs: Vec<TokenStream> = Vec::new();
    let mut match_arms: Vec<TokenStream> = Vec::new();

    for FnInfo {
        fn_ident,
        param_tys,
        param_idents,
        acc_ty,
        mod_path,
    } in &analysis.fn_infos
    {
        // -----------------------------------------
        // 1a. Generate per-instruction struct
        // -----------------------------------------
        let struct_name_str = fn_ident.to_string().to_case(Case::Pascal);
        let struct_ident = syn::Ident::new(&struct_name_str, fn_ident.span());

        let struct_path: TokenStream = quote! { #module_ident :: __private :: #struct_ident };

        let struct_body: TokenStream = if param_tys.is_empty() {
            quote! { {} }
        } else {
            let fields: Vec<TokenStream> = param_idents
                .iter()
                .zip(param_tys.iter())
                .map(|(id, ty)| quote! { pub #id : #ty })
                .collect();
            quote! { { #( #fields ),* } }
        };

        // -----------------------------------------
        // 1b. Compute 8-byte discriminator at macro-time
        //      = sha256("global:<handler>")[0..8]
        // -----------------------------------------
        let hash = {
            let mut hasher = Sha256::new();
            hasher.update(format!("global:{}", fn_ident).as_bytes());
            let result = hasher.finalize();
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&result[..8]);
            arr
        };

        let disc_tokens: Vec<TokenStream> = hash
            .iter()
            .map(|b| {
                let lit = syn::LitInt::new(&b.to_string(), Span::call_site());
                quote! { #lit }
            })
            .collect();

        let struct_def: TokenStream = quote! {
            #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
            pub struct #struct_ident #struct_body

            impl #struct_ident {
                pub const DISCRIMINATOR: [u8; 8] = [ #( #disc_tokens ),* ];
            }
        };
        struct_defs.push(struct_def);

        // -----------------------------------------
        // 1c. Generate dispatcher arm for this instruction
        // -----------------------------------------
        // Build `::mod1::mod2` if the handler lives in nested modules.
        // When `mod_path` is empty this expands to an empty stream and is a no-op.
        let nested_path: TokenStream = quote! { #( :: #mod_path )* };

        let handler_call: TokenStream = if param_tys.is_empty() {
            quote! { #module_ident #nested_path :: #fn_ident(ctx)?; }
        } else {
            let param_access: Vec<TokenStream> = param_idents
                .iter()
                .map(|id| quote! { params.#id })
                .collect();
            quote! { #module_ident #nested_path :: #fn_ident(ctx, #( #param_access ),* )?; }
        };

        let arm_body: TokenStream = if attr_cfg.enable_bitcoin_tx {
            let max_inputs_lit = LitInt::new(
                &attr_cfg.btc_tx_cfg.max_inputs_to_sign.unwrap().to_string(),
                Span::call_site(),
            );
            let max_mod_lit = LitInt::new(
                &attr_cfg
                    .btc_tx_cfg
                    .max_modified_accounts
                    .unwrap()
                    .to_string(),
                Span::call_site(),
            );
            let rune_set_path: syn::Path =
                syn::parse_str("crate::__SaturnDefaultRuneSet").expect("internal path parse");

            quote! {
                let mut accounts_struct = <#acc_ty as saturn_account_parser::Accounts>::try_accounts(accounts)?;

                let btc_tx_builder = saturn_account_parser::TxBuilderWrapper::<'info, #max_mod_lit, #max_inputs_lit, #rune_set_path>::default();

                let ctx = saturn_account_parser::Context::new_with_btc_tx(
                    program_id,
                    &mut accounts_struct,
                    &[],
                    btc_tx_builder,
                );

                #handler_call
            }
        } else {
            quote! {
                let mut accounts_struct = <#acc_ty as saturn_account_parser::Accounts>::try_accounts(accounts)?;

                let ctx = saturn_account_parser::Context::new_simple(
                    program_id,
                    &mut accounts_struct,
                    &[],
                );
                #handler_call
            }
        };

        let arm: TokenStream = quote! {
            d if d == #struct_path :: DISCRIMINATOR => {
                let params: #struct_path = borsh::BorshDeserialize::try_from_slice(data)
                    .map_err(|e| ProgramError::BorshIoError(e.to_string()))?;
                #arm_body
            }
        };
        match_arms.push(arm);
    }

    // Push the hidden module with struct definitions inside the user's module
    let private_mod_ts: TokenStream = quote! {
        #[doc(hidden)]
        pub mod __private {
            use super::*;
            #( #struct_defs )*
        }
    };

    // Inject the private module into the user's module so the structs are in scope.
    if let Some((_brace, ref mut items)) = item_mod_mut.content {
        let private_item: syn::Item = syn::parse2(private_mod_ts.clone()).expect("failed to parse private module");
        items.push(private_item);
    }

    // ---------------------------------------------------------------------
    // 2. Generate dispatcher function (Anchor-style 8-byte discriminator)
    // ---------------------------------------------------------------------
    let process_ident = syn::Ident::new("process_instruction", Span::call_site());

    // The internal dispatcher (not exported as the BPF entrypoint).
    let dispatcher_ts: TokenStream = quote! {
        #[allow(clippy::needless_borrow)]
        pub fn #process_ident<'info>(
            program_id: &arch_program::pubkey::Pubkey,
            accounts: &'info [arch_program::account::AccountInfo<'info>],
            instruction_data: &[u8],
        ) -> Result<(), arch_program::program_error::ProgramError> {
            use arch_program::program_error::ProgramError;

            if instruction_data.len() < 8 {
                return Err(ProgramError::InvalidInstructionData);
            }

            let mut disc = [0u8; 8];
            disc.copy_from_slice(&instruction_data[..8]);
            let data = &instruction_data[8..];

            match disc {
                #( #match_arms ),*
                _ => return Err(ProgramError::InvalidInstructionData),
            }

            Ok(())
        }
    };

    // ------------------------------------------------------------------
    // 3. Generate a root-level (actually parent-module-level) wrapper that
    //    exposes the BPF entrypoint symbol and forwards to the internal
    //    dispatcher. This matches Anchor's behaviour and works even when
    //    the #[saturn_program] macro is applied inside a nested module.
    // ------------------------------------------------------------------

    let wrapper_ident = syn::Ident::new("__saturn_entrypoint", Span::call_site());

    let wrapper_ts: TokenStream = quote! {
        #[cfg(all(not(test), not(feature = "no-entrypoint")))]
        arch_program::entrypoint!(#wrapper_ident);

        #[allow(clippy::needless_borrow)]
        fn #wrapper_ident<'info>(
            program_id: &arch_program::pubkey::Pubkey,
            accounts: &'info [arch_program::account::AccountInfo<'info>],
            instruction_data: &[u8],
        ) -> Result<(), arch_program::program_error::ProgramError> {
            #module_ident::#process_ident(program_id, accounts, instruction_data)
        }
    };

    // Keep ID enforcement (macro requires crate::ID)
    let id_check_ts: TokenStream = quote! {
        #[allow(dead_code)]
        const __SATURN_ENFORCE_ID: () = {
            let _ = &crate::ID;
        };
    };

    quote! {
        #id_check_ts
        #rune_alias_ts
        #item_mod_mut
        #dispatcher_ts
        #wrapper_ts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::{quote, ToTokens};
    use syn::parse_quote;

    fn dummy_attr_cfg(enable_btc: bool) -> AttrConfig {
        let mut cfg = AttrConfig {
            enable_bitcoin_tx: enable_btc,
            btc_tx_cfg: Default::default(),
        };
        if enable_btc {
            cfg.btc_tx_cfg.max_inputs_to_sign = Some(2);
            cfg.btc_tx_cfg.max_modified_accounts = Some(4);
            cfg.btc_tx_cfg.rune_set = Some(syn::parse_str("crate::RuneSet").unwrap());
        }
        cfg
    }

    fn dummy_analysis(module_name: &str) -> AnalysisResult {
        let mod_ident = syn::Ident::new(module_name, proc_macro2::Span::call_site());
        let item_mod: syn::ItemMod = parse_quote! { mod #mod_ident {} };
        let fn_info = FnInfo {
            fn_ident: syn::Ident::new("handle_transfer", proc_macro2::Span::call_site()),
            acc_ty: syn::parse_str::<syn::Path>("crate::Acc").unwrap(),
            mod_path: vec![],
            param_tys: vec![syn::parse_str::<syn::Type>("u8").unwrap()],
            param_idents: vec![syn::Ident::new("val", proc_macro2::Span::call_site())],
        };
        AnalysisResult {
            item_mod,
            fn_infos: vec![fn_info],
        }
    }

    #[test]
    fn generates_simple_dispatcher() {
        let attr_cfg = dummy_attr_cfg(false);
        let analysis = dummy_analysis("my_mod");
        let ts = generate(&attr_cfg, &analysis);
        let ts_str = ts.to_string();
        assert!(
            ts_str.contains("Context :: new_simple"),
            "Should use simple context when BTC disabled"
        );
        assert!(!ts_str.contains("new_with_btc_tx"));
        assert!(ts_str.contains("handle_transfer"));
        assert!(ts_str.contains("my_mod"));
    }

    #[test]
    fn generates_btc_dispatcher() {
        let attr_cfg = dummy_attr_cfg(true);
        let analysis = dummy_analysis("btc_mod");
        let ts = generate(&attr_cfg, &analysis);
        let ts_str = ts.to_string();
        assert!(
            ts_str.contains("new_with_btc_tx"),
            "Should use BTC context when enabled"
        );
        assert!(ts_str.contains("handle_transfer"));
        // Ensure const generics were injected (numbers 2 and 4 from cfg)
        assert!(ts_str.contains("2") && ts_str.contains("4"));
    }

    #[test]
    fn injects_rune_alias_in_btc_dispatcher() {
        let mut cfg = dummy_attr_cfg(true);
        cfg.btc_tx_cfg.rune_capacity = Some(3);
        let analysis = dummy_analysis("alias_mod");
        let ts = generate(&cfg, &analysis);
        let ts_str = ts.to_string();
        assert!(
            ts_str.contains("__SaturnDefaultRuneSet"),
            "Should emit alias type"
        );
    }
}
