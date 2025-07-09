use proc_macro2::{Span, TokenTree};
use quote::ToTokens;
use syn::parse_quote;
use syn::{FnArg, GenericParam, Item, ItemMod, LitInt, Type};

use crate::program::attr::AttrConfig;

use super::gather::FnInfo;
use super::helpers::has_cfg_test;

/// Rewrites the first parameter type of each handler function to the fully-qualified
/// `saturn_account_parser::Context<..>` path so users can write simply `Context` in
/// their source code without relying on injected type aliases.
pub fn rewrite_context_params(item_mod: &mut ItemMod, fn_infos: &[FnInfo], attr_cfg: &AttrConfig) {
    // Helper that performs the recursive parameter rewrite.
    fn rewrite_in_mod(
        current_mod: &mut ItemMod,
        mod_path: &mut Vec<syn::Ident>,
        fn_infos: &[FnInfo],
        attr_cfg: &AttrConfig,
    ) {
        if let Some((_brace, ref mut items)) = current_mod.content {
            for inner in items.iter_mut() {
                match inner {
                    Item::Fn(fn_item) => {
                        if has_cfg_test(&fn_item.attrs) {
                            continue;
                        }

                        // locate fn_info by matching ident and mod_path
                        let maybe_info = fn_infos.iter().find(|info| {
                            info.fn_ident == fn_item.sig.ident && info.mod_path == *mod_path
                        });
                        let acc_ty_path = match maybe_info {
                            Some(info) => &info.acc_ty,
                            None => continue,
                        };

                        let new_param_ty: Type = if attr_cfg.enable_bitcoin_tx {
                            let max_inputs = LitInt::new(
                                &attr_cfg.btc_tx_cfg.max_inputs_to_sign.unwrap().to_string(),
                                Span::call_site(),
                            );
                            let max_modified = LitInt::new(
                                &attr_cfg
                                    .btc_tx_cfg
                                    .max_modified_accounts
                                    .unwrap()
                                    .to_string(),
                                Span::call_site(),
                            );
                            let rune_set_path = attr_cfg.btc_tx_cfg.rune_set.clone().unwrap();

                            parse_quote! {
                                saturn_account_parser::Context<
                                    '_, '_, '_, 'info,
                                    #acc_ty_path,
                                    saturn_account_parser::TxBuilderWrapper<'info, #max_modified, #max_inputs, #rune_set_path>
                                >
                            }
                        } else {
                            parse_quote! {
                                saturn_account_parser::Context<
                                    '_, '_, '_, 'info,
                                    #acc_ty_path
                                >
                            }
                        };

                        // Helper: recursively search a token stream for an identifier.
                        fn stream_contains_ident(
                            ts: &proc_macro2::TokenStream,
                            ident: &str,
                        ) -> bool {
                            for tt in ts.clone() {
                                match tt {
                                    TokenTree::Ident(ref id) if id == ident => return true,
                                    TokenTree::Group(ref g)
                                        if stream_contains_ident(&g.stream(), ident) =>
                                    {
                                        return true
                                    }
                                    _ => {}
                                }
                            }
                            false
                        }

                        // Detect if `'info` is already declared either as an explicit lifetime
                        // parameter or inside a `for<'info>` binder in the where-clause.
                        let lifetime_in_generics = fn_item.sig.generics.params.iter().any(|gp| {
                            matches!(gp, GenericParam::Lifetime(lt) if lt.lifetime.ident == "info")
                        });

                        let lifetime_in_where = fn_item
                            .sig
                            .generics
                            .where_clause
                            .as_ref()
                            .map(|wc| stream_contains_ident(&wc.to_token_stream(), "info"))
                            .unwrap_or(false);

                        let has_info_lifetime = lifetime_in_generics || lifetime_in_where;

                        if !has_info_lifetime {
                            fn_item.sig.generics.params.push(syn::parse_quote!('info));
                        }

                        if let Some(first_arg) = fn_item.sig.inputs.first_mut() {
                            if let FnArg::Typed(pat_ty) = first_arg {
                                pat_ty.ty = Box::new(new_param_ty);
                            }
                        }
                    }
                    Item::Mod(inner_mod) => {
                        if inner_mod.content.is_some() {
                            mod_path.push(inner_mod.ident.clone());
                            rewrite_in_mod(inner_mod, mod_path, fn_infos, attr_cfg);
                            mod_path.pop();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let mut path = Vec::<syn::Ident>::new();
    rewrite_in_mod(item_mod, &mut path, fn_infos, attr_cfg);
}
