use saturn_program_macros::saturn_program;

mod instruction {
    #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
    pub enum Instr {
        Dummy,
    }
}

#[saturn_program(
    instruction = "crate::instruction::Instr",
    btc_tx_cfg(max_inputs_to_sign = "four", max_modified_accounts = true)
)]
mod handlers {}

fn main() {} 