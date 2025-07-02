# Saturn Crates – Bitcoin-ready building blocks for Arch Network programs

> "Write your program as if Bitcoin was a first-class citizen of the VM. We will take care of the bytes on-chain."

This repository gathers a **set of small, focused Rust libraries** that make it ergonomic to write on-chain programs for the **Arch Network** – a high-performance execution environment inspired by Solana's BPF runtime – **while still being able to compose and broadcast native Bitcoin transactions**.

All crates live in the same Cargo workspace and are published under the `saturn-*` prefix:

| Crate                                                                    | Description                                                                                                                                                                                       |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`saturn-bitcoin-transactions`](./saturn-bitcoin-transactions/README.md) | Zero-heap, `no_std`-friendly builder to craft, fee-tune and finalise Bitcoin transactions directly from an Arch program. Handles mempool ancestry, fee-rate validation and signing orchestration. |
| [`saturn-collections`](./saturn-collections/README.md)                   | Deterministic, fixed-capacity collections (`FixedList`, `FixedOption`, macros…) tailored for constrained BPF environments where heap allocation is not available.                                 |
| [`saturn-safe-math`](./saturn-safe-math/README.md)                       | Thin wrappers around `num` that provide overflow-checked arithmetic (`safe_add`, `safe_mul`, …) and a handy `mul_div` helper based on `primitive_types::U256`.                                    |

---

## Why does this exist?

Working with Bitcoin transactions from inside a Solana-style VM is _hard_:

- the program runs in a **no-std, heap-less** environment;
- transactions must be built deterministically and in a single pass;
- fee-calculation needs to be aware of **mempool ancestors** and dynamic fee rates;
- overflows and panics can brick the program.

The Saturn crates try to hide that complexity behind safe, well-tested abstractions so you can focus on _your_ business logic.

---

## Quick start

Add the crate you need as a **path dependency** in your program's `Cargo.toml` (until the packages are published):

```toml
[dependencies]
saturn-bitcoin-transactions = { path = "../saturn-crates/saturn-bitcoin-transactions" }
# or
saturn-safe-math = { path = "../saturn-crates/saturn-safe-math" }
```

### Crafting a Bitcoin transaction inside an Arch instruction

```rust
use saturn_bitcoin_transactions::{TransactionBuilder, fee_rate::FeeRate};

// Builder that can handle up to 8 modified accounts and 4 inputs still to be signed.
let mut builder: TransactionBuilder<8, 4> = TransactionBuilder::new();

// …add inputs / outputs, track modified PDAs, etc…

let target_fee_rate = FeeRate::try_from(20.0).unwrap(); // 20 sat/vB
builder.adjust_transaction_to_pay_fees(&target_fee_rate, None)?;

// Make the unsigned TX available to Arch so off-chain signers can provide witnesses.
builder.finalize()?;
```

### Safer arithmetic

```rust
use saturn_safe_math::{safe_add, mul_div};

let a: u64 = 2;
let b: u64 = 3;
let sum = safe_add(a, b)?; // 5 – or a `MathError` if it had overflowed

// Compute (a * b) / c without losing precision or panicking.
let result = mul_div(10u128, 20u128, 3u128)?;
```

---

## Feature flags

Some crates expose optional capabilities behind Cargo features:

- `serde` – enable (de)serialisation support for types that normally stay `no_std`.
- `runes` – track Ordinal Runic assets flowing through UTXOs.
- `utxo-consolidation` – automatically sweep small pool-owned UTXOs when fees are low.

Enable them as usual:

```toml
saturn-bitcoin-transactions = { path = "../saturn-crates/saturn-bitcoin-transactions", features = ["serde", "utxo-consolidation"] }
```

---

## Contributing

Pull requests, features and bug reports are welcome! Please open an issue first if you plan to work on something sizeable so we can avoid duplicated efforts.

1. Fork the repo & create a branch.
2. Run `cargo fmt` / `cargo clippy --all-targets`.
3. Add tests for your change.
4. Open a PR and describe _why_ the change is needed.

---

## License

Unless stated otherwise in sub-crates, this project is dual-licensed under **Apache-2.0** and **MIT**. You may freely choose either license.
