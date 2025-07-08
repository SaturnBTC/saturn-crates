# saturn-error-derive

Procedural macro that turns a Rust `enum` into a Solana-compatible error code set with stable numeric discriminants.

```rust
use saturn_error::saturn_error;

#[saturn_error(offset = 7000)]
pub enum MyError {
    #[error("Overflow occurred")]
    Overflow,
    #[error("Invalid authority")]
    InvalidAuthority,
}
```

* Automatically derives `Debug`, `Clone`, `Copy`, `PartialEq`, and `thiserror::Error`.
* Assigns consecutive `u32` codes starting at the chosen `offset` (default 6000).
* Generates `From<Enum> for arch_program::program_error::ProgramError` so the enum integrates with Solana programs.
