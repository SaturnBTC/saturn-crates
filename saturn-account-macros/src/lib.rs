use proc_macro::TokenStream;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

mod model;
// `FieldKind` and `FieldCfg` are now used only by submodules (`parser`, `codegen`).

mod codegen;
mod parser;
mod validator;


/// # `Accounts` derive macro
///
/// This crate provides the [`Accounts`] procedural macro which automatically
/// implements the `saturn_sdk::accounts::Accounts` trait (re-exported by
/// `saturn_program`) for a Rust struct.  The derive macro is the canonical way
/// to declare **which Solana accounts an instruction expects**, together with
/// all invariants that must hold at runtime (writability, signer status, PDA
/// derivation, etc.).
///
/// ## Quick example
///
/// ```ignore
/// use saturn_account_macros::Accounts;
/// use arch_program::account::{AccountInfo, BorshAccount};
/// use arch_program::pubkey::Pubkey;
///
/// #[derive(Accounts)]
/// pub struct Transfer<'info> {
///     /// Fee payer and transaction signer.
///     #[account(signer)]
///     payer: AccountInfo<'info>,
///
///     /// Source token account (must be writable and of the expected SPL type).
///     #[account(mut, of = TokenAccount)]
///     from: BorshAccount<'info, TokenAccount>,
///
///     /// Destination token account.
///     #[account(mut, of = TokenAccount)]
///     to: BorshAccount<'info, TokenAccount>,
///
///     /// SPL-Token program that owns the token accounts.
///     token_program: AccountInfo<'info>,
/// }
/// ```
///
/// The macro generates an implementation of `Accounts` that:
/// 1. Extracts the required accounts from the runtime `&[AccountInfo]` slice in
///    declaration order.
/// 2. Checks every attribute constraint (`signer`, `mut`, `of`, …) and returns
///    a descriptive [`ProgramError`] if a constraint is violated.
///
/// ## `#[account(..)]` field attributes
///
/// | Attribute | Purpose | Example |
/// |-----------|---------|---------|
/// | `signer` | The account **must** sign the transaction. | `#[account(signer)]` |
/// | `writable` / `mut` | The account must be writable. | `#[account(mut)]` |
/// | `address = <expr>` | Enforce an **absolute** `Pubkey` for this account. | `#[account(address = token_program::ID)]` |
/// | `seeds = &[..], program_id = <expr>` | Marks the account as a **Program Derived Address**. | `#[account(seeds = &[b"vault", payer.key()], program_id = crate::ID)]` |
/// | `len = <expr>` | Enforce the exact length of a fixed-size slice or vector. | `#[account(len = 3)]` |
/// | `shards` | Indicates a `Vec<AccountInfo>` that stores PDA shards. | `#[account(shards)]` |
/// | `of = Type` | Asserts that the account data deserialises into `Type`. | `#[account(of = TokenAccount)]` |
/// | `zero_copy` | Read the account data via zero-copy. Must be combined with `of`. | `#[account(zero_copy, of = MarketState)]` |
/// | `init` | Create a brand-new account. Requires `payer` & `program_id`; optional `space`. | `#[account(init, payer = payer, program_id = crate::ID, space = 8 + State::SIZE)]` |
/// | `init_if_needed` | Same as `init` but skips creation if the account already exists. | `#[account(init_if_needed, payer = payer, program_id = crate::ID, space = 72)]` |
/// | `realloc` | Reallocate/extend an existing account. Requires `payer` & `space`. | `#[account(realloc, payer = payer, space = new_len)]` |
/// | `space = <expr>` | Byte length for `init`, `init_if_needed` or `realloc`. | `#[account(space = 8 + Config::SIZE)]` |
/// | `payer = <ident>` | Designates the account that pays rent for creation or resize. Must be a `signer`. | `#[account(init, payer = payer, …)]` |
/// | `bump` | Declares a *non-account* `u8` field that stores the PDA bump. | `bump: u8 #[account(bump)]` |
///
/// ### Sharded PDA vectors
///
/// When the `shards` flag is used on a `Vec<AccountInfo>` field, every element
/// is expected to be a PDA derived from the same seed prefix.  The vector can
/// be empty and will grow on-demand when shards are created.
///
/// ```ignore
/// #[derive(Accounts)]
/// pub struct Sharded<'info> {
///     #[account(shards, seeds = &[b"shard", user.key()], program_id = crate::ID)]
///     shards: Vec<AccountInfo<'info>>,
/// }
/// ```
///
/// ### Ignored helper fields
///
/// Marker fields such as `PhantomData<&'info ()>` are ignored by the macro,
/// which allows you to hold lifetimes or type information without influencing
/// the runtime account layout.
///
/// ## Validation rules
///
/// * `address` **cannot** be combined with `seeds`.
/// * `seeds` **requires** `program_id`.
/// * `init`, `init_if_needed`, and `realloc` are **mutually exclusive**.
/// * `realloc` **requires** `space`; `init` & `init_if_needed` accept it
///   optionally.
/// * `payer` must reference a **signer** field.
/// * The `signer` flag is invalid on `shards` vectors.
///
/// Violations are reported at **compile-time** whenever possible; otherwise they
/// surface as runtime errors.
///
/// ## Generated helpers
///
/// Besides the trait impl, the macro also generates:
/// * `const LEN: usize` – count of *primary* accounts (excluding vector
///   elements and phantom fields).
/// * `fn read<'a>(accs: &'a [AccountInfo<'a>]) -> Result<Self>` – convenience
///   wrapper around `Accounts::try_from`.
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
