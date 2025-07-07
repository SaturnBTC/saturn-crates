use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use std::collections::HashSet;
use syn::{parse2, Error, FnArg, Item, ItemFn, ItemMod, PatType, Path, Type};

/// Information about each instruction handler function encountered inside the module.
#[derive(Clone)]
pub struct FnInfo {
    pub fn_ident: syn::Ident,
    pub acc_ty: Path,
}

pub struct AnalysisResult {
    pub item_mod: ItemMod,
    pub fn_infos: Vec<FnInfo>,
}

/// Performs semantic analysis on the annotated module.
///
/// On success returns an `AnalysisResult` containing the (possibly modified)
/// module item and metadata about each instruction handler.
///
/// If semantic errors are encountered we return `Err(TokenStream)` containing
/// one or more `compile_error!` invocations which will abort the compilation.
pub fn analyze(item: TokenStream) -> Result<AnalysisResult, TokenStream> {
    // ------------------------------------------------------------
    // 1. Parse the annotated item: must be an *inline* module
    // ------------------------------------------------------------
    let mut item_mod: ItemMod = match parse2(item.clone()) {
        Ok(m) => m,
        Err(e) => return Err(e.to_compile_error()),
    };

    let (brace, _items) = match &mut item_mod.content {
        Some((brace, items)) => (brace, items),
        None => {
            let err = Error::new_spanned(
                &item_mod,
                "module must be inline (e.g. `mod foo { .. }`) for #[saturn_program]",
            );
            return Err(err.to_compile_error());
        }
    };
    let items = &item_mod.content.as_ref().unwrap().1;

    // ------------------------------------------------------------
    // 2. Gather function information and perform basic checks
    // ------------------------------------------------------------
    let mut errors = Vec::<TokenStream>::new();
    let mut fn_infos = Vec::<FnInfo>::new();

    for inner_item in items.iter() {
        if let Item::Fn(ItemFn { sig, attrs, .. }) = inner_item {
            // Skip handlers that are only compiled when `cfg(test)` is active.
            if has_cfg_test(attrs) {
                continue;
            }

            let fn_ident = sig.ident.clone();

            // Expect at least 2 arguments
            if sig.inputs.len() < 2 {
                errors.push(
                    Error::new_spanned(
                        sig,
                        "handler must take (&mut InstructionContext<'a>, params)",
                    )
                    .to_compile_error(),
                );
                continue;
            }

            // ----------------------------------------------------
            // First argument: &mut Context<'_, Accounts>
            // ----------------------------------------------------
            let mut inputs_iter = sig.inputs.iter();
            let first = inputs_iter.next().unwrap();
            let acc_ty_path_opt: Option<Path> = if let FnArg::Typed(PatType { ty, .. }) = first {
                match &**ty {
                    Type::Reference(ref ref_ty) => {
                        if let Type::Path(type_path) = &*ref_ty.elem {
                            if let Some(seg) = type_path.path.segments.last() {
                                if seg.ident == "Context" {
                                    if let syn::PathArguments::AngleBracketed(gen_args) =
                                        &seg.arguments
                                    {
                                        if gen_args.args.len() == 2 {
                                            let second = gen_args.args.iter().nth(1).unwrap();
                                            if let syn::GenericArgument::Type(Type::Path(
                                                acc_path,
                                            )) = second
                                            {
                                                Some(acc_path.path.clone())
                                            } else {
                                                errors.push(Error::new_spanned(second, "expected accounts type in Context<'_, Accounts>").to_compile_error());
                                                None
                                            }
                                        } else {
                                            errors.push(Error::new_spanned(gen_args, "`Context` must have exactly two generic parameters: a lifetime and the accounts type").to_compile_error());
                                            None
                                        }
                                    } else {
                                        errors.push(
                                            Error::new_spanned(
                                                seg,
                                                "Context must have generic parameters",
                                            )
                                            .to_compile_error(),
                                        );
                                        None
                                    }
                                } else {
                                    errors.push(
                                        Error::new_spanned(
                                            seg,
                                            "first argument must be &mut Context<'_, Accounts>",
                                        )
                                        .to_compile_error(),
                                    );
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            errors.push(
                                Error::new_spanned(
                                    ref_ty.elem.as_ref(),
                                    "first arg inner type must be Context",
                                )
                                .to_compile_error(),
                            );
                            None
                        }
                    }
                    _ => {
                        errors.push(
                            Error::new_spanned(ty, "first argument must be &mut Context")
                                .to_compile_error(),
                        );
                        None
                    }
                }
            } else {
                errors.push(
                    Error::new_spanned(first, "unexpected receiver; expected typed arg")
                        .to_compile_error(),
                );
                continue;
            };

            // ----------------------------------------------------
            // Second argument becomes variant parameter – ensure it's a path type
            // ----------------------------------------------------
            let second = inputs_iter.next().unwrap();
            if let FnArg::Typed(PatType { ty, .. }) = second {
                match &**ty {
                    Type::Path(_tp) => { /* OK */ }
                    _ => {
                        errors.push(
                            Error::new_spanned(ty, "parameter type must be a path")
                                .to_compile_error(),
                        );
                        continue;
                    }
                }
            } else {
                errors.push(
                    Error::new_spanned(second, "unexpected receiver; expected typed arg")
                        .to_compile_error(),
                );
                continue;
            };

            if let Some(acc_ty) = acc_ty_path_opt {
                fn_infos.push(FnInfo { fn_ident, acc_ty });
            }
        }
    }

    // ------------------------------------------------------------
    // 3. Detect duplicate variant names after convert_case transformation
    // ------------------------------------------------------------
    {
        let mut seen: HashSet<String> = HashSet::new();
        for FnInfo { fn_ident, .. } in &fn_infos {
            let variant_name = fn_ident.to_string().to_case(Case::Pascal);
            if !seen.insert(variant_name) {
                errors.push(Error::new_spanned(fn_ident, "duplicate instruction variant after case conversion; rename the handler or use distinct names").to_compile_error());
            }
        }
    }

    // If we encountered errors, return them now so the caller can embed them.
    if !errors.is_empty() {
        let combined = quote! { #item_mod #( #errors )* };
        return Err(combined);
    }

    if fn_infos.is_empty() {
        let ts = quote! {
            #item_mod
            compile_error!("#[saturn_program] module must define at least one non-#[cfg(test)] instruction handler");
        };
        return Err(ts);
    }

    Ok(AnalysisResult { item_mod, fn_infos })
}

/// Helper: returns `true` if the attribute list includes `#[cfg(test)]`.
fn has_cfg_test(attrs: &[syn::Attribute]) -> bool {
    use proc_macro2::{Group, Ident, TokenTree};
    fn tokens_contain_test(ts: &proc_macro2::TokenStream) -> bool {
        use proc_macro2::{Group, Ident, TokenTree};
        for tt in ts.clone() {
            match tt {
                TokenTree::Ident(ref ident) if ident == "test" => return true,
                TokenTree::Group(ref grp) if tokens_contain_test(&grp.stream()) => return true,
                _ => {}
            }
        }
        false
    }

    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg") && tokens_contain_test(&attr.meta.to_token_stream())
    })
}
