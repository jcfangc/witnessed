use crate::WitnessExt;

use core::sync::atomic::{AtomicUsize, Ordering};
use std::{string::String, vec::Vec};

// ====================
// by: success / error
// ====================

struct Pos;

#[derive(Debug, PartialEq, Eq)]
enum PosErr {
    NonPos,
}

impl Pos {
    #[inline]
    fn prove(input: &i32) -> Result<Self, PosErr> {
        (*input > 0).then_some(Self).ok_or(PosErr::NonPos)
    }
}

#[test]
fn by_ok_returns_witnessed_and_exposes_inner_readonly() {
    let w = 7.witness().by(Pos::prove).unwrap();

    assert_eq!(*w, 7);
    assert_eq!(w.as_ref(), &7);
}

#[test]
fn by_err_propagates_witness_error() {
    let e = 0.witness().by(Pos::prove).unwrap_err();

    assert_eq!(e, PosErr::NonPos);
}

// ====================
// by: no normalization
// ====================

struct TrimNonEmpty;

#[derive(Debug, PartialEq, Eq)]
enum StrErr {
    Empty,
}

impl TrimNonEmpty {
    #[inline]
    fn prove(input: &String) -> Result<Self, StrErr> {
        (!input.trim().is_empty())
            .then_some(Self)
            .ok_or(StrErr::Empty)
    }
}

#[test]
fn by_does_not_normalize_by_default() {
    let w = String::from("  hi  ")
        .witness()
        .by(TrimNonEmpty::prove)
        .unwrap();

    assert_eq!(w.as_ref().as_str(), "  hi  ");
}

#[test]
fn by_trim_based_witness_can_still_fail() {
    let e = String::from("   ")
        .witness()
        .by(TrimNonEmpty::prove)
        .unwrap_err();

    assert_eq!(e, StrErr::Empty);
}

// ====================
// by: called exactly once
// ====================

static CALLS: AtomicUsize = AtomicUsize::new(0);

struct CountOnce;

#[derive(Debug, PartialEq, Eq)]
enum CountErr {
    Nope,
}

impl CountOnce {
    #[inline]
    fn prove(input: &u8) -> Result<Self, CountErr> {
        CALLS.fetch_add(1, Ordering::Relaxed);
        (*input == 1).then_some(Self).ok_or(CountErr::Nope)
    }
}

#[test]
fn by_calls_proof_function_exactly_once() {
    CALLS.store(0, Ordering::Relaxed);

    let _ = 1_u8.witness().by(CountOnce::prove);
    assert_eq!(CALLS.load(Ordering::Relaxed), 1);

    let _ = 2_u8.witness().by(CountOnce::prove);
    assert_eq!(CALLS.load(Ordering::Relaxed), 2);
}

// ====================
// by: context-capturing closure
// ====================

struct InRange;

#[derive(Debug, PartialEq, Eq)]
enum RangeErr {
    OutOfRange { value: i32 },
}

struct Range {
    start: i32,
    end_excl: i32,
}

impl InRange {
    #[inline]
    fn prove_in(range: &Range, value: &i32) -> Result<Self, RangeErr> {
        (range.start <= *value && *value < range.end_excl)
            .then_some(Self)
            .ok_or(RangeErr::OutOfRange { value: *value })
    }
}

#[test]
fn by_supports_context_capturing_closure() {
    let range = Range {
        start: 10,
        end_excl: 20,
    };

    let w = 15
        .witness()
        .by(|value| InRange::prove_in(&range, value))
        .unwrap();

    assert_eq!(*w, 15);
}

#[test]
fn by_context_capturing_closure_can_fail() {
    let range = Range {
        start: 10,
        end_excl: 20,
    };

    let e = 20
        .witness()
        .by(|value| InRange::prove_in(&range, value))
        .unwrap_err();

    assert_eq!(e, RangeErr::OutOfRange { value: 20 });
}

// ====================
// by: composite input
// ====================

struct AbcW;

#[derive(Debug, PartialEq, Eq)]
enum AbcErr {
    AEmpty,
    BOdd { b: u32 },
    CNonAscii,
}

impl AbcW {
    #[inline]
    fn prove(input: &(String, u32, Vec<u8>)) -> Result<Self, AbcErr> {
        let (a, b, c) = input;

        if a.trim().is_empty() {
            return Err(AbcErr::AEmpty);
        }

        if b % 2 != 0 {
            return Err(AbcErr::BOdd { b: *b });
        }

        if !c.is_ascii() {
            return Err(AbcErr::CNonAscii);
        }

        Ok(Self)
    }
}

#[test]
fn by_composite_tuple_ok_without_normalization() {
    let w = (String::from("  hello  "), 42_u32, b"ABC".to_vec())
        .witness()
        .by(AbcW::prove)
        .unwrap();

    assert_eq!(w.as_ref().0.as_str(), "  hello  ");
    assert_eq!(w.as_ref().1, 42);
    assert_eq!(w.as_ref().2.as_slice(), b"ABC");
}

// ====================
// by_unchecked
// ====================

struct TrustedPos;

#[test]
fn by_unchecked_constructs_witnessed_without_running_proof() {
    let w = unsafe { 0_i32.witness().by_unchecked::<TrustedPos>() };

    assert_eq!(*w, 0);
}
