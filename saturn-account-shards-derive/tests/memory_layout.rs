mod common;
use common::*;
use saturn_account_shards::StateShard;
use std::mem;

#[test]
fn test_struct_memory_layout() {
    // Ensure alignment is multiples of 8 bytes for common ABI stability.
    assert_eq!(mem::align_of::<DefaultShard>() % 8, 0);
    assert_eq!(mem::align_of::<CustomFieldShard>() % 8, 0);
    assert_eq!(mem::align_of::<ComplexShard>() % 8, 0);

    // Basic sanity: sizes are non-zero
    let default_size = mem::size_of::<DefaultShard>();
    let custom_size = mem::size_of::<CustomFieldShard>();
    let complex_size = mem::size_of::<ComplexShard>();

    assert!(default_size > 0);
    assert!(custom_size > 0);
    assert!(complex_size > 0);

    // Complex struct should be larger due to extra fields
    assert!(complex_size > default_size);
}
