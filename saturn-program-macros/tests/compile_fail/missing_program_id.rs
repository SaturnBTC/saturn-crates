use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::Account;
use saturn_account_parser::Context;
use saturn_program_macros::saturn_program;

#[derive(Accounts)]
pub struct DummyAcc<'info> {
    #[account(signer, mut)]
    caller: Account<'info, u64>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Instr {
    pub value: u64,
}

#[saturn_program]
mod my_prog {
    use super::*;
    // Intentionally **no** declare_id! or ID constant here.

    pub fn dummy<'info>(
        _ctx: Context<'info, DummyAcc<'info>>,
        _ix: Instr,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        Ok(())
    }
}

fn main() {}
