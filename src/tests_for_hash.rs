use super::test_support::*;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[inline]
fn hash64(x: impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    x.hash(&mut h);
    h.finish()
}

#[test]
fn hash_matches_inner_hash_exactly() {
    let w = witnessed(42_i32);

    assert_eq!(hash64(&w), hash64(&42_i32));
}

#[test]
fn equal_inner_implies_equal_hash() {
    let a = witnessed(7_i32);
    let b = witnessed(7_i32);

    assert_eq!(hash64(&a), hash64(&b));
}

#[test]
fn different_inner_usually_differs_in_hash_smoke() {
    let a = witnessed(1_i32);
    let b = witnessed(2_i32);

    assert_ne!(hash64(&a), hash64(&b));
}
