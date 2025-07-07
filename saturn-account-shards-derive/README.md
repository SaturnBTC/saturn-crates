# StateShard Derive Macro

This crate provides the `#[derive(StateShard)]` macro for automatically generating implementations of the `StateShard` trait, which manages Bitcoin and Rune UTXO collections in on-chain account state.

## Quick Start

```rust
use saturn_account_shards_derive::StateShard;
use saturn_account_shards::declare_shard_utxo_types;

// First, declare the UTXO collection types
declare_shard_utxo_types!(
    MyRuneSet,        // Your rune set type
    BtcUtxos,         // Name for BTC UTXO array type  
    RuneUtxo,         // Name for Rune UTXO option type
    25,               // BTC UTXO capacity
    15                // Padding for alignment
);

// Then derive StateShard on your struct
#[derive(StateShard)]
#[repr(C)]
pub struct MyPool {
    pub pool_id: u64,
    pub liquidity: u128,
    pub btc_utxos: BtcUtxos,    // Fixed-size array of BTC UTXOs
    pub rune_utxo: RuneUtxo,    // Optional Rune UTXO
}
```

## Requirements

- **`#[repr(C)]`**: Required for FFI-safe memory layout
- **Named fields**: The struct must have named fields (not tuple struct)
- **No generics**: The struct cannot be generic
- **UTXO fields**: Must contain fields for BTC UTXOs (array) and Rune UTXOs (option)

## Basic Usage

### Step 1: Declare UTXO Collection Types

Use the `declare_shard_utxo_types!` macro to generate the required collection types:

```rust
use saturn_account_shards::declare_shard_utxo_types;

declare_shard_utxo_types!(
    SimpleRuneSet,     // Rune set type (e.g., SingleRuneSet, MultiRuneSet)
    PoolBtcUtxos,      // Name for the BTC UTXO array type
    PoolRuneUtxo,      // Name for the Rune UTXO option type
    50,                // Maximum BTC UTXOs (capacity)
    15                 // Padding bytes for alignment
);
```

### Step 2: Define Your Struct

```rust
use saturn_account_shards_derive::StateShard;

#[derive(StateShard)]
#[repr(C)]
pub struct LiquidityPool {
    // Your custom fields
    pub pool_id: u64,
    pub total_liquidity: u128,
    pub fee_rate: u32,
    
    // Required UTXO fields (using default names)
    pub btc_utxos: PoolBtcUtxos,
    pub rune_utxo: PoolRuneUtxo,
}
```

### Step 3: Use the Generated Implementation

```rust
let mut pool = LiquidityPool::default();

// Access BTC UTXOs
let btc_utxos = pool.btc_utxos();
let btc_utxos_mut = pool.btc_utxos_mut();

// Add a BTC UTXO
let utxo = /* create UtxoInfo */;
if let Some(index) = pool.add_btc_utxo(utxo) {
    println!("Added BTC UTXO at index: {}", index);
}

// Manage Rune UTXO
pool.set_rune_utxo(rune_utxo);
if let Some(rune_utxo) = pool.rune_utxo() {
    // Process rune UTXO
}
pool.clear_rune_utxo();
```

## Advanced Configuration

You can customize field names and types using the `#[shard]` attribute:

```rust
#[derive(StateShard)]
#[shard(
    btc_utxos_attr = "bitcoin_utxos",
    rune_utxo_attr = "rune_utxo_data",
    utxo_info_type = "CustomUtxoInfo",
    rune_set_type = "CustomRuneSet",
    fixed_option_type = "CustomFixedOption"
)]
#[repr(C)]
pub struct CustomPool {
    pub pool_id: u64,
    pub bitcoin_utxos: CustomBtcUtxoArray,
    pub rune_utxo_data: CustomRuneUtxoOption,
}
```

## Shard Attribute Parameters

The `#[shard]` attribute supports these optional parameters:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `btc_utxos_attr` | `string` | `"btc_utxos"` | Name of the field containing BTC UTXOs |
| `rune_utxo_attr` | `string` | `"rune_utxo"` | Name of the field containing Rune UTXOs |
| `utxo_info_type` | `string` | `"saturn_bitcoin_transactions::utxo_info::UtxoInfo"` | Type of UTXO info objects |
| `rune_set_type` | `string` | `"saturn_bitcoin_transactions::utxo_info::SingleRuneSet"` | Type of rune set |
| `fixed_option_type` | `string` | `"saturn_bitcoin_transactions::utxo_info::FixedOptionUtxoInfo"` | Type of fixed option for rune UTXOs |

## Generated Methods

The `StateShard` trait provides these methods for UTXO management:

### BTC UTXO Management
- `btc_utxos() -> &[UtxoInfo]`: Get immutable slice of BTC UTXOs
- `btc_utxos_mut() -> &mut [UtxoInfo]`: Get mutable slice of BTC UTXOs
- `btc_utxos_retain(&mut self, f: &mut dyn FnMut(&UtxoInfo) -> bool)`: Filter BTC UTXOs in-place
- `add_btc_utxo(&mut self, utxo: UtxoInfo) -> Option<usize>`: Add BTC UTXO, returns index if successful
- `btc_utxos_len() -> usize`: Get current number of BTC UTXOs
- `btc_utxos_max_len() -> usize`: Get maximum capacity for BTC UTXOs

### Rune UTXO Management
- `rune_utxo() -> Option<&UtxoInfo>`: Get immutable reference to Rune UTXO
- `rune_utxo_mut() -> Option<&mut UtxoInfo>`: Get mutable reference to Rune UTXO
- `clear_rune_utxo(&mut self)`: Remove the Rune UTXO
- `set_rune_utxo(&mut self, utxo: UtxoInfo)`: Set the Rune UTXO

## Complete Example

```rust
use saturn_account_shards_derive::StateShard;
use saturn_account_shards::declare_shard_utxo_types;
use saturn_bitcoin_transactions::utxo_info::{UtxoInfo, SingleRuneSet};

// Declare collection types
declare_shard_utxo_types!(
    SingleRuneSet,
    ExchangeBtcUtxos,
    ExchangeRuneUtxo,
    100,  // Can hold up to 100 BTC UTXOs
    15    // Padding for alignment
);

#[derive(StateShard)]
#[repr(C)]
pub struct Exchange {
    pub exchange_id: u64,
    pub total_volume: u128,
    pub fee_collected: u64,
    pub btc_utxos: ExchangeBtcUtxos,
    pub rune_utxo: ExchangeRuneUtxo,
}

impl Exchange {
    pub fn new(exchange_id: u64) -> Self {
        Self {
            exchange_id,
            total_volume: 0,
            fee_collected: 0,
            btc_utxos: ExchangeBtcUtxos::default(),
            rune_utxo: ExchangeRuneUtxo::none(),
        }
    }
    
    pub fn process_deposit(&mut self, utxo: UtxoInfo<SingleRuneSet>) {
        // Add BTC UTXO
        if let Some(index) = self.add_btc_utxo(utxo) {
            println!("Deposited BTC UTXO at index: {}", index);
        }
    }
    
    pub fn process_rune_deposit(&mut self, utxo: UtxoInfo<SingleRuneSet>) {
        // Set Rune UTXO (replaces existing one)
        self.set_rune_utxo(utxo);
        println!("Deposited Rune UTXO");
    }
    
    pub fn cleanup_small_utxos(&mut self, min_value: u64) {
        // Remove UTXOs below minimum value
        self.btc_utxos_retain(&mut |utxo| utxo.value() >= min_value);
        println!("Cleaned up small UTXOs below {} sats", min_value);
    }
}

// Usage
fn main() {
    let mut exchange = Exchange::new(1);
    
    // Process deposits
    let btc_utxo = /* create UtxoInfo */;
    exchange.process_deposit(btc_utxo);
    
    // Check UTXO count
    println!("Current BTC UTXOs: {}/{}", 
             exchange.btc_utxos_len(), 
             exchange.btc_utxos_max_len());
    
    // Cleanup
    exchange.cleanup_small_utxos(1000);
}
```

## Error Handling

The macro generates compile-time errors for:

- **Missing `#[repr(C)]`**: The struct must be `#[repr(C)]` for memory safety
- **Invalid field names**: Field names specified in `#[shard]` must exist in the struct
- **Invalid type specifications**: Type paths must be valid Rust types
- **Generic structs**: The derive macro doesn't support generic structs

## Best Practices

1. **Use descriptive names** for your UTXO collection types
2. **Choose appropriate capacities** based on your use case
3. **Add proper alignment padding** (15 bytes works for most cases)
4. **Initialize with defaults** or use builder patterns
5. **Handle capacity limits** when adding UTXOs
6. **Use retain methods** for efficient filtering

## Feature Compatibility

This derive macro works with all features of the `saturn-bitcoin-transactions` crate:

- **Basic UTXOs**: Always supported
- **Runes**: When "runes" feature is enabled
- **Consolidation**: When "utxo-consolidation" feature is enabled

The generated `StateShard` implementation provides access to all UTXO information including conditional fields based on enabled features. 