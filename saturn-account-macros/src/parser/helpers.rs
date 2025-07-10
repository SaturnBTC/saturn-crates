use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Type, TypePath, PathArguments, GenericArgument};

use crate::model::FieldKind;

/// Detects whether the field represented by `base_ty` should be treated as a
/// fixed-length slice or a sharded vector and returns the appropriate
/// `FieldKind`.  The logic is *syntactic* – semantic validation is performed
/// later by the `validator` module.
///
/// * `base_ty`       – the type **without** `&` reference wrapper (already
///                     stripped by the caller).
/// * `is_shards`     – whether the user specified the `shards` flag.
/// * `len_expr_opt`  – optional `len = <expr>` attribute captured while
///                     parsing the raw attribute list.
/// * `span`          – span of the originating field for error reporting.
pub(super) fn detect_field_kind(
    base_ty: &Type,
    is_shards: bool,
    len_expr_opt: Option<Expr>,
    span: proc_macro2::Span,
) -> Result<FieldKind, syn::Error> {
    // First determine if the base type is a `Vec<...>`.
    if let Some(elem_ty) = extract_vec_elem(base_ty) {
        // The field is a `Vec<_>`.
        let len_expr = match len_expr_opt {
            Some(e) => e,
            None => {
                return Err(syn::Error::new(
                    span,
                    "vector field requires `#[account(len = <expr>)]` or the `shards` flag",
                ));
            }
        };

        let len_ts: TokenStream = quote!(#len_expr);

        if is_shards {
            // Return Shards variant – we need both len expression and element type.
            let elem_ts: TokenStream = quote!(#elem_ty);
            return Ok(FieldKind::Shards(len_ts, elem_ts));
        } else {
            // Regular fixed slice (no shards).
            return Ok(FieldKind::FixedSlice(len_ts));
        }
    }

    // Error out if the user attempted to use the `shards` flag on a non-Vec field.
    if is_shards {
        return Err(syn::Error::new(span, "`shards` flag can only be used on `Vec<_>` fields"));
    }

    // For the moment we treat non-vector types as single accounts.  Future
    // iterations may add support for reference slices.
    Ok(FieldKind::Single)
}

/// Attempts to extract the element type `T` from `Vec<T>`.
/// Returns `Some(&Type)` if `ty` is recognised as a `Vec<T>`; otherwise `None`.
fn extract_vec_elem(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            // Match on the **last** segment (`Vec` or fully-qualified path).
            if let Some(seg) = path.segments.last() {
                if seg.ident == "Vec" {
                    match &seg.arguments {
                        PathArguments::AngleBracketed(args) => {
                            for arg in &args.args {
                                if let GenericArgument::Type(inner_ty) = arg {
                                    return Some(inner_ty);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            None
        }
        _ => None,
    }
} 