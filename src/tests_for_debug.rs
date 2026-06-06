use super::test_support::*;

use std::format;

#[test]
fn debug_fmt_matches_tuple_shape_without_regex() {
    let w = witnessed(7_i32);
    let s = format!("{:?}", w);

    assert!(s.starts_with("Witnessed("));
    assert!(s.ends_with(')'));

    let inner = &s["Witnessed(".len()..s.len() - 1];

    assert!(
        inner.chars().all(|c| c.is_ascii_digit()),
        "inner part should be digits, got: {}",
        inner
    );

    assert_eq!(inner, "7");
}

#[test]
fn debug_fmt_preserves_inner_debug_repr_exactly() {
    let w = witnessed("hi");

    assert_eq!(format!("{:?}", w), "Witnessed(\"hi\")");
}

#[test]
fn debug_fmt_complex_structure() {
    let w = witnessed(vec![1, 2]);

    assert_eq!(format!("{:?}", w), "Witnessed([1, 2])");
}
