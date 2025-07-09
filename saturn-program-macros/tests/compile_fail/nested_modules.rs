use borsh::{BorshSerialize, BorshDeserialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::Account;
use saturn_program_macros::saturn_program;
use saturn_program_macros::declare_id;
use saturn_account_parser::Context;

declare_id!("8YE2m8RGmFjyWkHfMV6aA1eeaoAj8ZqEXnoY6v1WKEwd");

mod outer {
    use super::*;

    #[derive(Accounts)]
    pub struct DummyAccounts<'info> {
        #[account(signer)]
        caller: Account<'info, u64>,
    }

    mod instruction {
        use super::*;
        #[derive(BorshSerialize, BorshDeserialize)]
        pub enum Instr {
            Call(u8),
        }
    }

    #[saturn_program]
    mod handlers {
        use super::*;
        pub fn call<'info>(
            ctx: Context<'info, DummyAccounts<'info>>, // to be rewritten
            _p: u8,
        ) -> Result<(), arch_program::program_error::ProgramError> {
            let _ = ctx.program_id;
            Ok(())
        }
    }
}

fn main() {} 