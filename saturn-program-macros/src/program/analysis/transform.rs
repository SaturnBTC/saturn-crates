use proc_macro2::Span;
use syn::parse_quote;
use syn::{FnArg, Item, ItemMod, LitInt, Type};

use crate::program::attr::AttrConfig;

use super::gather::FnInfo;
use super::helpers::has_cfg_test;

/// Rewrites the first parameter type of each handler function to the fully-qualified
/// `saturn_account_parser::Context<..>` path so users can write simply `Context` in
/// their source code without relying on injected type aliases.
pub fn rewrite_context_params(item_mod: &mut ItemMod, fn_infos: &[FnInfo], attr_cfg: &AttrConfig) {
    if let Some((_brace, ref mut inner_items)) = item_mod.content {
        for inner in inner_items.iter_mut() {
            if let Item::Fn(fn_item) = inner {
                if has_cfg_test(&fn_item.attrs) {
                    continue;
                }

                // Locate the analysis info for this function to get its Accounts type.
                let acc_ty_path = match fn_infos.iter().find(|i| i.fn_ident == fn_item.sig.ident) {
                    Some(info) => &info.acc_ty,
                    None => continue,
                };

                // Build the replacement type for the first parameter.
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
                        &mut saturn_account_parser::Context<
                            '_, '_, '_, 'info,
                            #acc_ty_path,
                            saturn_account_parser::TxBuilderWrapper<'info, #max_modified, #max_inputs, #rune_set_path>
                        >
                    }
                } else {
                    parse_quote! {
                        &mut saturn_account_parser::Context<
                            '_, '_, '_, 'info,
                            #acc_ty_path
                        >
                    }
                };

                // Mutate the AST: replace the first argument's type.
                if let Some(first_arg) = fn_item.sig.inputs.first_mut() {
                    if let FnArg::Typed(pat_ty) = first_arg {
                        pat_ty.ty = Box::new(new_param_ty);
                    }
                }
            }
        }
    }
}
