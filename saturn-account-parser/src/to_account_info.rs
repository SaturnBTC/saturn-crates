// -------------------------------------------------------------------------------------------------
// Trait: ToAccountInfo
// -------------------------------------------------------------------------------------------------
/// Lightweight abstraction that allows any Saturn/Anchor-style account wrapper to be treated as an
/// [`arch_program::account::AccountInfo`].  This removes the need for callers (including our code
/// generation macros) to know the concrete wrapper type when they only need the underlying
/// `AccountInfo`, e.g. to fetch the public key or pass the account to CPI helpers.
///
/// Implementations are provided for:
/// * Raw `AccountInfo<'info>` – returns itself.
/// * `codec::BorshAccount<'info, T>` (`Account<'info, T>`) – returns the embedded `AccountInfo`.
/// * `codec::ZeroCopyAccount<'info, T>` (`AccountLoader<'info, T>`) – same.
pub trait ToAccountInfo<'info> {
    /// Returns an **owned** [`AccountInfo`] pointing at the same underlying data. This mirrors
    /// Anchor's `ToAccountInfo` behaviour and lets the caller pass the value directly into CPI
    /// helpers which take `AccountInfo` by value.
    fn to_account_info(&self) -> arch_program::account::AccountInfo<'info>;
}

// -------------------------------------------------------------------------------------------------
// Blanket implementation based on `AsRef<AccountInfo>` — matches Anchor's design.
// Any type that can be referenced as an `AccountInfo` automatically gets `ToAccountInfo`.
// -------------------------------------------------------------------------------------------------

impl<'info, T> ToAccountInfo<'info> for T
where
    T: AsRef<arch_program::account::AccountInfo<'info>>,
{
    #[inline]
    fn to_account_info(&self) -> arch_program::account::AccountInfo<'info> {
        self.as_ref().clone()
    }
}
