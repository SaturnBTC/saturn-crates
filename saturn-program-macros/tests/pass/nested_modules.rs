use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use saturn_program_macros::{declare_id, saturn_program};

// Declare a dummy program id so the macro passes the ID check.
declare_id!("5X4RQFAEUKu9yyR9pv8uXcEUTdEK7m2YkdEYY5EYXPLH");

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    caller: Account<'info, u64>,
}

mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        AdminSetFee(u8),
    }
}

// ---------------------------------------------------------------------
// The saturn_program module contains nested sub-modules. The macro
// should accept this layout and generate a working dispatcher.
// ---------------------------------------------------------------------
#[saturn_program]
mod handlers {
    use super::*;

    pub mod admin {
        use super::*;
        pub mod dex {
            use super::*;

            pub fn set_fee<'info>(
                ctx: Context<'info, DummyAccounts<'info>>,
                pct: u8,
            ) -> Result<(), arch_program::program_error::ProgramError> {
                let _ = (ctx.program_id, pct); // Access to silence warnings
                Ok(())
            }
        }
    }
}

fn main() {} 