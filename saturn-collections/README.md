# Saturn Collections

Generic, fixed-capacity data structures & utility macros designed for **Arch / Saturn on-chain programs**.

These collections are **allocation-free**, `bytemuck`-compatible and suitable for deterministic, low-footprint environments such as smart-contracts and embedded runtimes.

---

## Features

-   🗃 **FixedList** – contiguous array-backed list with `push`/`pop` semantics
-   🗂 **FixedSet** – bit-set-like structure for constant-size membership tracking
-   ⚙️ **Macros**
    -   `declare_fixed_array!` – generate a `Pod + Zeroable` struct wrapping a statically-sized array with a runtime length field
    -   `declare_fixed_option!` – generate a `Pod + Zeroable` Option-like wrapper with predictable layout
-   ✨ **`PushPopCollection` trait** – abstraction over `push`, `pop`, `len` & slice access implemented for both the custom collections and `Vec<T>`
-   100 % safe Rust with extensive unit tests

## Installation

Add the crate to your `Cargo.toml`:

```toml
saturn-collections = { git = "https://github.com/arch-protocol/saturn-arch-programs", package = "saturn-collections" }
```

(Replace the git URL / version as appropriate.)

The only direct dependencies are `bytemuck` and `serde` (both re-exported from the workspace).

## Quick start

### FixedList

```rust
use saturn_collections::generic::fixed_list::FixedList;

// A list that can hold up to 4 `u32`s without a heap allocation
let mut list: FixedList<u32, 4> = FixedList::new();

list.push(10);
list.push(20);
assert_eq!(list.len(), 2);
assert_eq!(list.pop(), Some(20));
```

### FixedSet

```rust
use saturn_collections::generic::set::FixedSet;

const SIZE: usize = 8;
let mut set: FixedSet<SIZE> = FixedSet::new();

set.insert(3);
set.insert(5);
assert!(set.contains(3));
assert_eq!(set.count(), 2);
```

### declare_fixed_array!

```rust
use saturn_collections::declare_fixed_array;

// Create a wrapper that can store up to 16 `u64`s and fits into a zero-copy buffer
declare_fixed_array!(U64Array16, u64, 16);

let mut arr = U64Array16::new();
arr.add(42).unwrap();
assert_eq!(arr.len(), 1);
```

### declare_fixed_option!

```rust
use saturn_collections::declare_fixed_option;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Price(u64);

// Wrapper occupies `size_of::<Price>() + 1 + 7` bytes (padding to 8-byte alignment)
declare_fixed_option!(FixedPriceOpt, Price, 7);

let some_price = FixedPriceOpt::some(Price(10_000));
assert!(some_price.is_some());
assert_eq!(some_price.get().unwrap().0, 10_000);
```

## Why fixed-size collections?

Smart-contract platforms (and many constrained systems) disallow dynamic memory allocation or make it prohibitively expensive. Using compile-time capacities gives you:

-   Predictable layout – crucial for zero-copy deserialization & account storage.
-   Deterministic gas/fee usage.
-   Simpler safety audits.
