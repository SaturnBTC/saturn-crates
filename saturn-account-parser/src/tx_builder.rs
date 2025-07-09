/// Thin wrapper around `saturn_bitcoin_transactions::TransactionBuilder` that
/// keeps the const-generic parameters visible to the type system while being
/// easy to store inside [`Context`].
pub struct TxBuilderWrapper<
    'a,
    const MAX_MODIFIED_ACCOUNTS: usize,
    const MAX_INPUTS_TO_SIGN: usize,
    RuneSet,
>(
    pub(crate)  saturn_bitcoin_transactions::TransactionBuilder<
        'a,
        MAX_MODIFIED_ACCOUNTS,
        MAX_INPUTS_TO_SIGN,
        RuneSet,
    >,
)
where
    RuneSet: saturn_collections::generic::fixed_set::FixedCapacitySet<
            Item = arch_program::rune::RuneAmount,
        > + Default;

impl<
        'a,
        const MAX_MODIFIED_ACCOUNTS: usize,
        const MAX_INPUTS_TO_SIGN: usize,
        RuneSet: saturn_collections::generic::fixed_set::FixedCapacitySet<
                Item = arch_program::rune::RuneAmount,
            > + Default,
    > Default for TxBuilderWrapper<'a, MAX_MODIFIED_ACCOUNTS, MAX_INPUTS_TO_SIGN, RuneSet>
{
    fn default() -> Self {
        Self(saturn_bitcoin_transactions::TransactionBuilder::<
            'a,
            MAX_MODIFIED_ACCOUNTS,
            MAX_INPUTS_TO_SIGN,
            RuneSet,
        >::new())
    }
}

impl<'a, const MAX_MODIFIED_ACCOUNTS: usize, const MAX_INPUTS_TO_SIGN: usize, RuneSet>
    core::ops::Deref
    for TxBuilderWrapper<'a, MAX_MODIFIED_ACCOUNTS, MAX_INPUTS_TO_SIGN, RuneSet>
where
    RuneSet: saturn_collections::generic::fixed_set::FixedCapacitySet<
            Item = arch_program::rune::RuneAmount,
        > + Default,
{
    type Target = saturn_bitcoin_transactions::TransactionBuilder<
        'a,
        MAX_MODIFIED_ACCOUNTS,
        MAX_INPUTS_TO_SIGN,
        RuneSet,
    >;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, const MAX_MODIFIED_ACCOUNTS: usize, const MAX_INPUTS_TO_SIGN: usize, RuneSet>
    core::ops::DerefMut
    for TxBuilderWrapper<'a, MAX_MODIFIED_ACCOUNTS, MAX_INPUTS_TO_SIGN, RuneSet>
where
    RuneSet: saturn_collections::generic::fixed_set::FixedCapacitySet<
            Item = arch_program::rune::RuneAmount,
        > + Default,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
