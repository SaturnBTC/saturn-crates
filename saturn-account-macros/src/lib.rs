use proc_macro::TokenStream;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

mod model;
// `FieldKind` and `FieldCfg` are now used only by submodules (`parser`, `codegen`).

mod codegen;
mod parser;
mod validator;

/// Derive macro that automatically implements `sdk::accounts::Accounts` for a struct.
///
/// The first, minimal version simply fetches each `AccountInfo` from the input
/// slice in declaration order and stuffs it into the resulting struct.  No extra
/// attribute handling is supported yet – those will be added incrementally.
#[proc_macro_derive(Accounts, attributes(account))]
pub fn derive_accounts(input: TokenStream) -> TokenStream {
    // Parse the struct definition.
    let input: DeriveInput = parse_macro_input!(input);

    let struct_ident = &input.ident;

    // Ensure we are processing a struct with named fields.
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return syn::Error::new_spanned(
                    &input.ident,
                    "#[derive(Accounts)] only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "#[derive(Accounts)] can only be used with structs",
            )
            .to_compile_error()
            .into();
        }
    };

    let parsed_fields = match parser::parse_fields(fields) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    if let Err(e) = validator::validate(&parsed_fields) {
        return e.to_compile_error().into();
    }

    match codegen::generate(struct_ident, &input.generics, &parsed_fields) {
        Ok(ts) => return TokenStream::from(ts),
        Err(e) => return e.to_compile_error().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arch_program::account::AccountInfo;
    use saturn_account_parser::codec::BorshAccount;
    use syn::{parse_quote, Data, DeriveInput, Fields};

    /// Helper that extracts the `named` fields of the input struct.
    fn extract_named_fields(
        di: &DeriveInput,
    ) -> &syn::punctuated::Punctuated<syn::Field, syn::token::Comma> {
        match &di.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(named) => &named.named,
                _ => panic!("expected named fields"),
            },
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn parser_accepts_basic_struct() {
        // A minimal, but valid, account struct mixing a signer account and a fixed slice.
        let di: DeriveInput = parse_quote! {
            struct MyAccounts<'info> {
                #[account(signer)]
                caller: BorshAccount<'info, u64>,
                #[account(len = 2)]
                pdas: Vec<AccountInfo<'static>>,
            }
        };

        let fields = extract_named_fields(&di);
        let parsed = parser::parse_fields(fields).expect("parse_fields should succeed");
        assert_eq!(parsed.len(), 2);

        // Verify first (signer) field.
        let caller_cfg = &parsed[0];
        assert_eq!(caller_cfg.ident.to_string(), "caller");
        assert_eq!(caller_cfg.is_signer, Some(true));
        assert!(matches!(caller_cfg.kind, model::FieldKind::Single));

        // Verify second (fixed slice) field.
        let pdas_cfg = &parsed[1];
        assert!(matches!(pdas_cfg.kind, model::FieldKind::FixedSlice(_)));
    }

    #[test]
    fn parser_rejects_seeds_without_program_id() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(seeds = &[b"seed"])]
                pda: BorshAccount<'info, u64>,
            }
        };
        let err = parser::parse_fields(extract_named_fields(&di)).unwrap_err();
        assert!(err.to_string().contains("program_id"));
    }

    #[test]
    fn parser_rejects_seeds_and_address_together() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(seeds = &[b"seed"], program_id = arch_program::pubkey::Pubkey::default(), address = arch_program::pubkey::Pubkey::default())]
                pda: BorshAccount<'info, u64>,
            }
        };
        let err = parser::parse_fields(extract_named_fields(&di)).unwrap_err();
        assert!(err.to_string().contains("cannot be combined"));
    }

    #[test]
    fn parser_rejects_vec_without_shards() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                vec_accs: Vec<AccountInfo<'static>>,
            }
        };
        let err = parser::parse_fields(extract_named_fields(&di)).unwrap_err();
        assert!(err.to_string().contains("vector field requires"));
    }

    #[test]
    fn validator_checks_init_payer_signer_relation() {
        // `payer` references a non-signer field – should error during validation.
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                // Not a signer on purpose.
                payer: BorshAccount<'info, u64>,
                #[account(init, payer = payer, program_id = arch_program::pubkey::Pubkey::default())]
                new_acc: BorshAccount<'info, u64>,
            }
        };
        let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        let err = validator::validate(&parsed).unwrap_err();
        assert!(err.to_string().contains("must be marked `signer`"));
    }

    #[test]
    fn validator_allows_valid_init_structure() {
        let di: DeriveInput = parse_quote! {
            struct Accs<'info> {
                #[account(signer)]
                payer: BorshAccount<'info, u64>,
                #[account(init, payer = payer, program_id = arch_program::pubkey::Pubkey::default())]
                new_acc: BorshAccount<'info, u64>,
            }
        };
        let parsed = parser::parse_fields(extract_named_fields(&di)).expect("parse ok");
        // Should validate without errors.
        validator::validate(&parsed).expect("validation should succeed");
    }
}

#[cfg(test)]
mod parser_tests;

#[cfg(test)]
mod validator_tests;
