use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;
use saturn_program_macros::saturn_program;
use saturn_bitcoin_transactions::utxo_info::SingleRuneSet;

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    caller: BorshAccount<'info, u64>,
}

mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        Handle(u8),
    }

    // Make RuneSet accessible via the instruction module
    pub type RuneSet = SingleRuneSet;
}

#[saturn_program(
    instruction = "crate::instruction::Instr",
    btc_tx_cfg(max_inputs_to_sign = 2, max_modified_accounts = 4, rune_set = "crate::instruction::RuneSet")
)]
mod handlers {
    use super::*;
    pub fn handle<'info>(
        // `Context` path is intentionally unqualified – macro must rewrite it
        ctx: &mut Context<'info, DummyAccounts<'info>>,
        _value: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        let _ = &ctx.btc_tx;
        Ok(())
    }
}

fn main() {} 