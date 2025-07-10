use crate::model::FieldCfg;

/// Shared context passed to each individual validation rule.
///
/// Besides the slice of parsed fields we pre-compute an index from field
/// identifier (`String`) → `(position_in_struct, &FieldCfg)`.  Many rules
/// require this lookup (e.g. payer validation, duplicate detection) so doing
/// it once here avoids repetition.
#[derive(Debug)]
pub struct ValidationCtx<'a> {
    pub fields: &'a [FieldCfg],
    pub by_ident: std::collections::HashMap<String, (usize, &'a FieldCfg)>,
}

impl<'a> ValidationCtx<'a> {
    pub fn new(fields: &'a [FieldCfg]) -> Self {
        let mut by_ident = std::collections::HashMap::with_capacity(fields.len());
        for (idx, f) in fields.iter().enumerate() {
            // Keep the first occurrence; duplicate detection is handled by a rule.
            by_ident.entry(f.ident.to_string()).or_insert((idx, f));
        }
        Self { fields, by_ident }
    }
}

/// Trait implemented by every standalone validation rule.
///
/// A rule receives an immutable reference to the shared `ValidationCtx`
/// and returns `Ok(())` when the input passes its checks or a `syn::Error`
/// locating the problem otherwise.
pub trait Validator {
    fn validate(&self, ctx: &ValidationCtx) -> Result<(), syn::Error>;
}
