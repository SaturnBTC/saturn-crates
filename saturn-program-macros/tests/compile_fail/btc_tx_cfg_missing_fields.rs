use saturn_program_macros::saturn_program;

mod instruction {
    #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
    pub enum Instr {
        Dummy,
    }
}

#[saturn_program(
    instruction = "crate::instruction::Instr",
    bitcoin_transaction = true,
    btc_tx_cfg(max_inputs_to_sign = 2) // missing max_modified_accounts
)]
mod handlers {}

fn main() {} 