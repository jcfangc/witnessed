use super::test_support::*;

use core::cmp::Ordering;
use std::vec::Vec;

#[test]
fn partial_ord_delegates_to_inner_for_total_ordered_types() {
    let a = witnessed(1_i32);
    let b = witnessed(2_i32);

    assert_eq!(a.partial_cmp(&b), Some(Ordering::Less));
    assert_eq!(b.partial_cmp(&a), Some(Ordering::Greater));
    assert_eq!(a.partial_cmp(&a), Some(Ordering::Equal));
}

#[test]
fn ord_cmp_delegates_to_inner_and_sort_matches_inner_sort() {
    let mut v = [witnessed(3_i32), witnessed(1_i32), witnessed(2_i32)];

    v.sort();

    let got: Vec<i32> = v.iter().map(|x| **x).collect();
    assert_eq!(got, vec![1, 2, 3]);
}

#[test]
fn partial_ord_propagates_none_for_nan_like_inner() {
    let nan = witnessed(f32::NAN);
    let one = witnessed(1.0_f32);

    assert_eq!(nan.partial_cmp(&one), None);
    assert_eq!(one.partial_cmp(&nan), None);
    assert_eq!(nan.partial_cmp(&nan), None);
}
