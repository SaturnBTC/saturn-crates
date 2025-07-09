use saturn_program_macros::saturn_program;

mod instruction {
    #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
    pub enum Instr {
        Dummy,
    }
}

#[saturn_program(foo = 1)]
mod handlers {}

fn main() {} 