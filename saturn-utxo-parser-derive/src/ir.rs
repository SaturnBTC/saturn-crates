#![allow(dead_code)]
//! Intermediate representation (IR) for the `UtxoParser` derive macro.
//!
//! By converting the incoming `syn::DeriveInput` into these plain Rust
//! structures first, we decouple parsing/validation from code-generation and
//! make unit testing trivial.

use proc_macro2::Span;
use syn::{Ident, Type};

/// What kind of reference collection a field represents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    /// A single `&'a UtxoInfo` reference.
    Single,
    /// A fixed-length array `[&'a UtxoInfo; N]`.
    Array(usize),
    /// A catch-all `Vec<&'a UtxoInfo>`.
    Vec,
    /// An optional reference `Option<&'a UtxoInfo>`.
    Optional,
}

/// Presence predicate coming from `runes = "..."`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunesPresence {
    None,
    Some,
    Any,
}

/// Data extracted from a single `#[utxo(..)]` attribute.
#[derive(Debug, Clone)]
pub struct UtxoAttr {
    /// Match only UTXOs whose `value` equals this amount (satoshis).
    pub value: Option<u64>,
    /// Constraints on rune presence (none / some / any).
    pub runes: Option<RunesPresence>,
    /// Expression string for a specific rune id check.
    pub rune_id_expr: Option<String>,
    /// Expression string for a specific rune amount check.
    pub rune_amount_expr: Option<String>,
    /// Whether this Vec field should capture the remaining inputs.
    pub rest: bool,
    /// Identifier of the accounts struct field to anchor against, if any.
    pub anchor_ident: Option<Ident>,
    /// Span of the attribute – kept for diagnostics.
    pub span: Span,
}

impl Default for UtxoAttr {
    fn default() -> Self {
        Self {
            value: None,
            runes: None,
            rune_id_expr: None,
            rune_amount_expr: None,
            rest: false,
            anchor_ident: None,
            span: Span::call_site(),
        }
    }
}

/// Representation of a single struct field after parsing.
#[derive(Debug, Clone)]
pub struct Field {
    pub ident: Ident,
    pub kind: FieldKind,
    pub ty: Type,
    pub attr: UtxoAttr,
    pub span: Span,
}

/// Parsed, high-level description of the entire derive input.
#[derive(Debug, Clone)]
pub struct DeriveInputIr {
    pub struct_ident: Ident,
    pub generics: syn::Generics,
    pub accounts_ty: Type,
    pub fields: Vec<Field>,
}

// TODO: define `FieldKind`, `UtxoAttr`, `Field`, `DeriveInputIr` etc. 