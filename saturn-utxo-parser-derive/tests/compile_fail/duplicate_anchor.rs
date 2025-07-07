use saturn_utxo_parser_derive::UtxoParser;
use saturn_bitcoin_transactions::utxo_info::UtxoInfo;

// Dummy Accounts type so the macro has a valid reference target.
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
struct DuplicateAnchor<'a> {
    #[utxo(anchor = acc)]
    first: &'a UtxoInfo,
    #[utxo(anchor = acc)]
    second: &'a UtxoInfo,
}

fn main() {} 