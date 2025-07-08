use proc_macro2::TokenStream;
use syn::parse::Parser;
use syn::{punctuated::Punctuated, Error, Lit, Meta, Path};

/// Bitcoin-transaction specific configuration parsed from the attribute list.
#[derive(Default, Clone)]
pub struct BtcTxCfg {
    pub max_inputs_to_sign: Option<usize>,
    pub max_modified_accounts: Option<usize>,
    pub rune_set: Option<Path>,
}

/// Result of parsing the procedural macro attribute.
#[derive(Clone)]
pub struct AttrConfig {
    pub instruction_path: Path,
    /// If `true`, the generated program will include Bitcoin transaction builder logic.
    /// This is now inferred from the presence of a `btc_tx_cfg(..)` section rather than
    /// an explicit `bitcoin_transaction` flag.
    pub enable_bitcoin_tx: bool,
    pub btc_tx_cfg: BtcTxCfg,
}

/// Parse the attribute list provided to `#[saturn_program(..)]`.
///
/// This function is **pure** (no side-effects) so it can be unit-tested easily.
pub fn parse(attr: TokenStream) -> Result<AttrConfig, Error> {
    // ------------------------------------------------------------
    // 1. Parse attribute list into `Meta` items
    // ------------------------------------------------------------
    let parser = Punctuated::<Meta, syn::Token![,]>::parse_terminated;
    let attr_meta: Punctuated<Meta, syn::Token![,]> = if attr.is_empty() {
        return Err(Error::new_spanned(
            TokenStream::new(),
            "expected attribute arguments",
        ));
    } else {
        parser.parse2(attr)?
    };

    let mut instruction_path: Option<Path> = None;
    let mut btc_tx_cfg: BtcTxCfg = BtcTxCfg::default();

    // Flags used during the second pass
    let mut btc_tx_cfg_seen = false;

    // ------------------------------------------------------------
    // 2. Handle each top-level attribute key/section
    // ------------------------------------------------------------
    for meta in &attr_meta {
        match meta {
            // ---------------------------
            // instruction = "path::ToInstr"
            // ---------------------------
            Meta::NameValue(nv) if nv.path.is_ident("instruction") => {
                if instruction_path.is_some() {
                    return Err(Error::new_spanned(
                        &nv.path,
                        "duplicate `instruction` key; specify it only once",
                    ));
                }
                match &nv.value {
                    syn::Expr::Lit(expr_lit) => {
                        if let Lit::Str(lit_str) = &expr_lit.lit {
                            let p: Path = syn::parse_str(&lit_str.value())?;
                            if p.segments.len() <= 1 {
                                return Err(Error::new_spanned(
                                    &nv.value,
                                    "instruction path must be namespaced (e.g. `crate::ix::MyInstr`) – add a module prefix",
                                ));
                            }
                            instruction_path = Some(p);
                        } else {
                            return Err(Error::new_spanned(
                                &nv.value,
                                "instruction value must be a string literal path",
                            ));
                        }
                    }
                    _ => {
                        return Err(Error::new_spanned(
                            &nv.value,
                            "instruction value must be string literal",
                        ));
                    }
                }
            }

            // ---------------------------
            // btc_tx_cfg = ( ... )
            // ---------------------------
            Meta::List(ml) if ml.path.is_ident("btc_tx_cfg") => {
                if btc_tx_cfg_seen {
                    return Err(Error::new_spanned(
                        &ml.path,
                        "duplicate `btc_tx_cfg` section",
                    ));
                }
                btc_tx_cfg_seen = true;

                let inner_parser = Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                let inner_meta = inner_parser.parse2(ml.tokens.clone())?;
                for nested in inner_meta {
                    match nested {
                        Meta::NameValue(nv) if nv.path.is_ident("max_inputs_to_sign") => {
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Int(int_lit) = &expr_lit.lit {
                                    btc_tx_cfg.max_inputs_to_sign = Some(int_lit.base10_parse::<usize>()?);
                                } else {
                                    return Err(Error::new_spanned(
                                        &nv.value,
                                        "max_inputs_to_sign must be an integer literal",
                                    ));
                                }
                            } else {
                                return Err(Error::new_spanned(
                                    &nv.value,
                                    "max_inputs_to_sign must be an integer literal",
                                ));
                            }
                        }
                        Meta::NameValue(nv) if nv.path.is_ident("max_modified_accounts") => {
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Int(int_lit) = &expr_lit.lit {
                                    btc_tx_cfg.max_modified_accounts = Some(int_lit.base10_parse::<usize>()?);
                                } else {
                                    return Err(Error::new_spanned(
                                        &nv.value,
                                        "max_modified_accounts must be an integer literal",
                                    ));
                                }
                            } else {
                                return Err(Error::new_spanned(
                                    &nv.value,
                                    "max_modified_accounts must be an integer literal",
                                ));
                            }
                        }
                        Meta::NameValue(nv) if nv.path.is_ident("rune_set") => {
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Str(lit_str) = &expr_lit.lit {
                                    btc_tx_cfg.rune_set = Some(syn::parse_str(&lit_str.value())?);
                                } else {
                                    return Err(Error::new_spanned(
                                        &nv.value,
                                        "rune_set must be a string literal path",
                                    ));
                                }
                            } else {
                                return Err(Error::new_spanned(
                                    &nv.value,
                                    "rune_set must be a string literal",
                                ));
                            }
                        }
                        other => {
                            return Err(Error::new_spanned(
                                other,
                                "unknown key inside btc_tx_cfg; expected `max_inputs_to_sign`, `max_modified_accounts`, or `rune_set`",
                            ));
                        }
                    }
                }
            }

            // ---------------------------
            // Removed flag: bitcoin_transaction = bool
            // --------------------------------------------------
            Meta::NameValue(nv) if nv.path.is_ident("bitcoin_transaction") => {
                return Err(Error::new_spanned(
                    &nv.path,
                    "`bitcoin_transaction` flag has been removed; use `btc_tx_cfg(...)` to enable Bitcoin transaction support",
                ));
            }

            other => {
                return Err(Error::new_spanned(
                    other,
                    "unknown attribute key; expected `instruction` or `btc_tx_cfg`",
                ));
            }
        }
    }

    // ---------------------------
    // 3. Final validations + defaults
    // ---------------------------
    let enable_bitcoin_tx = btc_tx_cfg_seen;

    if enable_bitcoin_tx {
        if btc_tx_cfg.max_inputs_to_sign.is_none() {
            return Err(Error::new_spanned(
                TokenStream::new(),
                "`btc_tx_cfg` must specify `max_inputs_to_sign`",
            ));
        }
        if btc_tx_cfg.max_modified_accounts.is_none() {
            return Err(Error::new_spanned(
                TokenStream::new(),
                "`btc_tx_cfg` must specify `max_modified_accounts`",
            ));
        }
        if btc_tx_cfg.rune_set.is_none() {
            btc_tx_cfg.rune_set = Some(syn::parse_str(
                "saturn_bitcoin_transactions::utxo_info::SingleRuneSet",
            )?);
        }
    }

    let instruction_path = instruction_path.ok_or_else(|| {
        Error::new_spanned(
            TokenStream::new(),
            "missing `instruction = \"Path\"` in attribute",
        )
    })?;

    Ok(AttrConfig {
        instruction_path,
        enable_bitcoin_tx,
        btc_tx_cfg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn parses_minimal_instruction() {
        let ts: proc_macro2::TokenStream = quote!(instruction = "crate::ix::Instr");
        let cfg = parse(ts).expect("should parse");
        assert_eq!(cfg.instruction_path.segments.last().unwrap().ident, "Instr");
        assert!(!cfg.enable_bitcoin_tx);
    }

    #[test]
    fn parses_full_bitcoin_tx_cfg() {
        let ts: proc_macro2::TokenStream = quote!(
            instruction = "crate::ix::Instr",
            btc_tx_cfg(max_inputs_to_sign = 8, max_modified_accounts = 16)
        );
        let cfg = parse(ts).expect("should parse");
        assert!(cfg.enable_bitcoin_tx);
        assert_eq!(cfg.btc_tx_cfg.max_inputs_to_sign, Some(8));
        assert_eq!(cfg.btc_tx_cfg.max_modified_accounts, Some(16));
    }

    #[test]
    fn error_on_missing_instruction() {
        // Missing the required `instruction = "Path"` key should error.
        let ts: proc_macro2::TokenStream = quote!(btc_tx_cfg(max_inputs_to_sign = 1, max_modified_accounts = 1));
        assert!(parse(ts).is_err());
    }

    #[test]
    fn error_on_duplicate_instruction_key() {
        let ts: proc_macro2::TokenStream = quote!(
            instruction = "crate::ix::Instr",
            instruction = "crate::ix::OtherInstr"
        );
        assert!(parse(ts).is_err());
    }

    #[test]
    fn error_on_duplicate_btc_tx_cfg_section() {
        let ts: proc_macro2::TokenStream = quote!(
            instruction = "crate::ix::Instr",
            btc_tx_cfg(max_inputs_to_sign = 1, max_modified_accounts = 1),
            btc_tx_cfg(max_inputs_to_sign = 2, max_modified_accounts = 2)
        );
        assert!(parse(ts).is_err());
    }

    #[test]
    fn error_on_unknown_attribute_key() {
        let ts: proc_macro2::TokenStream = quote!(instruction = "crate::ix::Instr", foo = 42);
        assert!(parse(ts).is_err());
    }

    #[test]
    fn error_on_unknown_btc_tx_cfg_key() {
        let ts: proc_macro2::TokenStream = quote!(
            instruction = "crate::ix::Instr",
            btc_tx_cfg(max_inputs_to_sign = 1, max_modified_accounts = 1, bar = 10)
        );
        assert!(parse(ts).is_err());
    }

    #[test]
    fn error_on_instruction_path_not_namespaced() {
        let ts: proc_macro2::TokenStream = quote!(instruction = "Instr");
        assert!(parse(ts).is_err());
    }

    #[test]
    fn parses_btc_tx_cfg_with_default_rune_set() {
        let ts: proc_macro2::TokenStream = quote!(
            instruction = "crate::ix::Instr",
            btc_tx_cfg(max_inputs_to_sign = 4, max_modified_accounts = 8)
        );
        let cfg = parse(ts).expect("should parse");
        // If rune_set not provided it should default to SingleRuneSet path
        let expected: syn::Path =
            syn::parse_str("saturn_bitcoin_transactions::utxo_info::SingleRuneSet").unwrap();
        assert_eq!(cfg.btc_tx_cfg.rune_set.unwrap(), expected);
    }

    #[test]
    fn parses_btc_tx_cfg_with_custom_rune_set() {
        let ts: proc_macro2::TokenStream = quote!(
            instruction = "crate::ix::Instr",
            btc_tx_cfg(
                max_inputs_to_sign = 2,
                max_modified_accounts = 4,
                rune_set = "crate::custom::RuneSet"
            )
        );
        let cfg = parse(ts).expect("should parse");
        let expected: syn::Path = syn::parse_str("crate::custom::RuneSet").unwrap();
        assert_eq!(cfg.btc_tx_cfg.rune_set.unwrap(), expected);
    }
}
