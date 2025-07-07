#![allow(dead_code)]
//! Semantic checks for the `UtxoParser` IR.

use crate::ir::{DeriveInputIr, FieldKind};

/// Perform post-parse validation on the IR. Returns `Ok(())` if everything
/// is semantically sound; otherwise an appropriate `syn::Error`.
///
/// These checks are intentionally kept separate from parsing so that the logic
/// is unit-testable and the error messages remain focused on semantic
/// problems rather than syntax.
pub fn check(ir: &DeriveInputIr) -> syn::Result<()> {
    use proc_macro2::Span;
    use syn::Error;

    // ---------------------------------------------------------------------
    // Verify anchor rules.
    // ---------------------------------------------------------------------
    let mut anchor_seen: Option<Span> = None;
    for field in &ir.fields {
        if let Some(id) = &field.attr.anchor_ident {
            if let Some(prev) = anchor_seen {
                return Err(Error::new(
                    prev,
                    "Multiple fields specify `anchor` attribute; only one field is allowed",
                ));
            }
            anchor_seen = Some(id.span());
        }
    }

    // ---------------------------------------------------------------------
    // Vec-related constraints.
    // ---------------------------------------------------------------------
    for field in &ir.fields {
        if let FieldKind::Vec = field.kind {
            match (
                field.attr.anchor_ident.is_some(),
                field.attr.rest,
            ) {
                // Vec + anchor but no rest → OK
                (true, false) => {}
                // Vec + anchor + rest → invalid
                (true, true) => {
                    return Err(Error::new(
                        field.span,
                        "Vec field cannot combine `anchor = <field>` with `rest` flag",
                    ));
                }
                // Vec + rest (no anchor) → OK
                (false, true) => {}
                // Vec without rest or anchor → invalid
                (false, false) => {
                    return Err(Error::new(
                        field.span,
                        "Vec field must be marked with `rest` flag: #[utxo(rest, ...)]",
                    ));
                }
            }
        } else {
            // Non-Vec field must not use `rest` flag.
            if field.attr.rest {
                return Err(Error::new(
                    field.span,
                    "`rest` flag is only allowed on Vec fields",
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn ir_from(src: &str) -> crate::ir::DeriveInputIr {
        let di: syn::DeriveInput = syn::parse_str(src).unwrap();
        parse::derive_input_to_ir(&di).unwrap()
    }

    #[test]
    fn duplicate_anchor_is_error() {
        let code = r#"
            #[utxo_accounts(Accs)]
            struct S<'a> {
                #[utxo(anchor = acc1)]
                a: &'a UtxoInfo,
                #[utxo(anchor = acc2)]
                b: &'a UtxoInfo,
            }
        "#;
        let ir = ir_from(code);
        assert!(check(&ir).is_err());
    }
} 