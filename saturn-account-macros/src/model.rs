use proc_macro2::TokenStream;
use syn::{Expr, Ident, Type};

/// Describes the different collection kinds a struct field can represent when parsed by the
/// `#[derive(Accounts)]` procedural macro.
#[derive(Debug, Clone)]
pub enum FieldKind {
    /// A single account (the common case).
    Single,
    /// A fixed-length slice `&[AccountInfo]`. Holds the expression that evaluates to the length.
    FixedSlice(TokenStream),
    /// A vector of shard accounts (zero-copy). First token stream is the length expression, the
    /// second is the element type.
    Shards(TokenStream, TokenStream),
    /// Marker field such as `PhantomData` that is not backed by an on-chain account.
    /// This does **not** consume an item from the account slice during parsing.
    Phantom,
    /// A bump value (u8) derived from PDA seeds. Does **not** consume an account.
    Bump,
}

/// Configuration collected for every field while parsing the user-declared struct.
///
/// This is a pure data structure – there is **no** dependency on the `proc_macro` crate so it can
/// be fully unit-tested without involving macro expansion.
#[derive(Debug, Clone)]
pub struct FieldCfg {
    pub ident: Ident,
    pub is_signer: Option<bool>,
    pub is_writable: Option<bool>,
    pub address: Option<Expr>,
    pub seeds: Option<Expr>,
    pub program_id: Option<Expr>,
    pub payer: Option<Expr>,
    pub is_shards: bool,
    pub kind: FieldKind,
    pub is_zero_copy: bool,
    pub is_init: bool,
    pub is_realloc: bool,
    pub is_init_if_needed: bool,
    pub base_ty: Type,
    pub space: Option<Expr>,
    /// Optional type specified via `of = MyShard` inside the `#[account(shards)]` attribute.
    pub of_type: Option<Type>,
}
