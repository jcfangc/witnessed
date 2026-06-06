use crate::test_support::*;

use super::*;
use core::mem;
use std::{string::String, vec::Vec};

struct TrimNonEmpty;
struct NonEmptyVec;

#[test]
fn witnessed_size_is_equal_to_inner_size_for_scalar() {
    assert_eq!(
        mem::size_of::<Witnessed<i32, AnyToken>>(),
        mem::size_of::<i32>()
    );
}

#[test]
fn witnessed_size_is_equal_to_inner_size_for_owned_types() {
    assert_eq!(
        mem::size_of::<Witnessed<String, TrimNonEmpty>>(),
        mem::size_of::<String>()
    );

    assert_eq!(
        mem::size_of::<Witnessed<Vec<u8>, NonEmptyVec>>(),
        mem::size_of::<Vec<u8>>()
    );
}

#[test]
fn witness_token_size_does_not_affect_witnessed_size() {
    assert_ne!(mem::size_of::<LargeToken>(), 0);

    assert_eq!(
        mem::size_of::<Witnessed<i32, LargeToken>>(),
        mem::size_of::<i32>()
    );

    assert_eq!(
        mem::size_of::<Witnessed<Vec<u8>, LargeToken>>(),
        mem::size_of::<Vec<u8>>()
    );
}
