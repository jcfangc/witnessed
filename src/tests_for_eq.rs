use super::test_support::*;

#[test]
fn partial_eq_compares_inner_only() {
    let a = witnessed(1_i32);
    let b = witnessed(1_i32);
    let c = witnessed(2_i32);

    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn eq_laws_hold_for_witnessed() {
    let a = witnessed(3_i32);
    let b = witnessed(3_i32);
    let c = witnessed(3_i32);

    // Reflexive
    assert_eq!(a, a);

    // Symmetric
    assert_eq!(a == b, b == a);

    // Transitive
    assert!(a == b && b == c && a == c);
}
