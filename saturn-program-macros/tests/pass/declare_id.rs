use saturn_program_macros::declare_id;

// Declare a constant program ID.
declare_id!("11111111111111111111111111111111");

fn main() {
    // Ensure the generated id() function is usable and returns a Pubkey.
    let _id: arch_program::pubkey::Pubkey = id();
    println!("Program ID: {:?}", _id);
} 