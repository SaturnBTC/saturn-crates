use saturn_bitcoin_transactions::utxo_info::UtxoInfo;
use saturn_utxo_parser_derive::UtxoParser;

// Dummy Accounts type deliberately missing the `missing` field referenced by the
// `anchor` attribute below. This should trigger a compile-time error.
#[derive(Debug)]
struct DummyAccounts<'info> {
    acc: arch_program::account::AccountInfo<'info>,
}

impl<'info> saturn_account_parser::Accounts<'info> for DummyAccounts<'info> {
    fn try_accounts(
        _accounts: &'info [arch_program::account::AccountInfo<'info>],
    ) -> Result<Self, arch_program::program_error::ProgramError> {
        unimplemented!()
    }
}

#[derive(UtxoParser)]
#[utxo_accounts(DummyAccounts<'a>)]
struct MissingAnchor<'a> {
    // The `anchor = missing` identifier does not exist on `DummyAccounts`; the macro
    // expansion will therefore reference a non-existent field and the compiler
    // must emit an error.
    #[utxo(anchor = missing)]
    utxo: &'a UtxoInfo,
}

fn main() {}
