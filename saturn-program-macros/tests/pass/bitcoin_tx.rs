use borsh::{BorshDeserialize, BorshSerialize};
use saturn_account_macros::Accounts;
use saturn_account_parser::codec::BorshAccount;
use saturn_program_macros::saturn_program;

#[derive(Accounts)]
struct DummyAccounts<'info> {
    #[account(signer)]
    caller: BorshAccount<'info, u64>,
}

mod instruction {
    use super::*;
    use saturn_bitcoin_transactions::utxo_info::SingleRuneSet;

    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum Instr {
        MyHandler(u8),
    }

    pub type RuneSet = SingleRuneSet;
}

#[saturn_program(
    instruction = "crate::instruction::Instr",
    btc_tx_cfg(max_inputs_to_sign = 4, max_modified_accounts = 4, rune_set = "crate::instruction::RuneSet")
)]
mod handlers {
    use super::*;
    pub fn my_handler<'info>(
        ctx: &mut Context<'info, DummyAccounts<'info>>,
        _params: u8,
    ) -> Result<(), arch_program::program_error::ProgramError> {
        let _ = ctx.program_id;
        let _btc_builder = &ctx.btc_tx;
        Ok(())
    }
}

fn main() {}
