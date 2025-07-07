use saturn_program_macros::saturn_program;

mod instruction {
    #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
    pub enum Instr {
        Dummy,
    }
}

#[saturn_program(
    instruction = "crate::instruction::Instr",
    bitcoin_transaction = true
)]
mod handlers {}

fn main() {} 