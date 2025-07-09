use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::Context;
use saturn_account_parser::codec::Account;
use saturn_program_macros::saturn_program;
use saturn_utxo_parser::{TryFromUtxos, UtxoParser};
use saturn_program_macros::declare_id;

declare_id!("8YE2m8RGmFjyWkHfMV6aA1eeaoAj8ZqEXnoY6v1WKEwd");

/// Simple Accounts struct used by the handler.
#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    caller: Account<'info, u64>,
}

/// UTXO parser that references the above `DummyAccounts` via the mandatory
/// `#[utxo_accounts(..)]` attribute. For the purposes of this compile-time test
/// we only require a single fee UTXO and ignore the rest.
#[derive(UtxoParser)]
#[utxo_accounts(DummyAccounts)]
struct DummyUtxos {
    /// Mandatory fee UTXO worth exactly 10_000 sats.
    #[utxo(value = 10_000)]
    fee: saturn_bitcoin_transactions::utxo_info::UtxoInfo,
}

// Instruction enum used by the `#[saturn_program]` macro.
mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        /// Calls `my_handler` with a dummy parameter.
        MyHandler(u8),
    }
}

/// Integration test: make sure the Saturn program macro compiles when the
/// handler makes use of both the `Accounts` and `UtxoParser` derive macros.
#[saturn_program]
mod handlers {
    use super::*;

    pub fn my_handler<'info>(
        ctx: Context<'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        // Call the generated `try_utxos` to ensure the UtxoParser impl links
        // correctly together with the Accounts struct coming from `ctx`.
        // We pass an empty slice here because the compile-time check is what
        // matters – no runtime validation is performed in a trybuild test.
        let _ = DummyUtxos::try_utxos(ctx.accounts, &[]);
        Ok(())
    }
}

fn main() {}
