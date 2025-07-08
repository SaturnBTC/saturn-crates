mod outer {
    use borsh::{BorshSerialize, BorshDeserialize};
    use saturn_account_macros::Accounts;
    use saturn_account_parser::codec::BorshAccount;
    use saturn_program_macros::saturn_program;

    #[derive(Accounts)]
    pub struct DummyAccounts<'info> {
        #[account(signer)]
        caller: BorshAccount<'info, u64>,
    }

    mod instruction {
        use super::*;
        #[derive(BorshSerialize, BorshDeserialize)]
        pub enum Instr {
            Call(u8),
        }
    }

    #[saturn_program(instruction = "crate::outer::instruction::Instr")]
    mod handlers {
        use super::*;
        pub fn call<'info>(
            ctx: &mut Context<'info, DummyAccounts<'info>>, // to be rewritten
            _p: u8,
        ) -> Result<(), arch_program::program_error::ProgramError> {
            let _ = ctx.program_id;
            Ok(())
        }
    }
}

fn main() {} 