#[cfg(test)]
mod tests {
    use super::{ir::RunesPresence, parse, validate, codegen::predicate};

    /// Simple helper to parse a string into `syn::DeriveInput`.
    fn parse_derive(src: &str) -> syn::DeriveInput {
        syn::parse_str::<syn::DeriveInput>(src).expect("failed to parse derive input")
    }

    #[test]
    fn parse_basic_struct() {
        let code = r#"
            #[derive(Debug)]
            struct DummyAccounts;

            #[utxo_accounts(DummyAccounts)]
            struct Simple<'a> {
                #[utxo(value = 1_000, runes = \"none\")]
                fee: &'a saturn_bitcoin_transactions::utxo_info::UtxoInfo,
            }
        "#;

        let di = parse_derive(code);
        let ir = parse::derive_input_to_ir(&di).expect("parse to ir");
        assert_eq!(ir.fields.len(), 1);
        let field = &ir.fields[0];
        assert_eq!(field.attr.value, Some(1_000));
        assert_eq!(field.attr.runes, Some(RunesPresence::None));
    }

    #[test]
    fn validate_duplicate_anchor_fails() {
        let code = r#"
            #[derive(Debug)]
            struct Accs;

            #[utxo_accounts(Accs)]
            struct S<'a> {
                #[utxo(anchor = acc1)]
                a: &'a UtxoInfo,
                #[utxo(anchor = acc2)]
                b: &'a UtxoInfo,
            }
        "#;
        let di = parse_derive(code);
        let ir = parse::derive_input_to_ir(&di).unwrap();
        assert!(validate::check(&ir).is_err());
    }

    #[test]
    fn predicate_builds_expected_snippet() {
        let mut attr = super::ir::UtxoAttr::default();
        attr.value = Some(42);
        attr.runes = Some(RunesPresence::Some);
        let ts = predicate::build(&attr);
        let ts_str = ts.to_string();
        assert!(ts_str.contains("utxo.value == 42"));
        assert!(ts_str.contains("utxo.runes.len() > 0"));
    }
} 