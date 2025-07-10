use saturn_program_macros::saturn_program;

mod instruction {
    #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
    pub enum Instr {
        Dummy,
    }
}

#[saturn_program(
    btc_tx_cfg(max_inputs_to_sign = 1, max_modified_accounts = 1, rune_set = "crate::Foo", rune_capacity = 3)
)]
mod handlers {}

fn main() {} 