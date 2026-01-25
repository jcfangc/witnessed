#![no_std]
// This crate is `no_std`: the public API depends only on `core`, so it can be used in
// embedded/bare-metal or other environments without the Rust standard library.

#[cfg(test)]
#[macro_use]
extern crate std;
// Tests run with `std` enabled. We explicitly link `std` in the test configuration so unit tests
// can use conveniences like `vec!`, `format!`, `String`/`Vec`, and `std`-only helpers.
// This does not affect the library in non-test builds.

use core::{marker::PhantomData, ops::Deref};

/// A type-level witness that can attest `T` satisfies some invariant.
///
/// Implementors are expected to check (and optionally normalize) `input`
/// and return a `Witnessed<T, Self>` on success.
pub trait Witness<T>: Sized {
    type Error;

    /// Validate and optionally normalize the input.
    /// Returns the (possibly rewritten) value on success.
    fn attest(input: T) -> Result<T, Self::Error>;

    /// Construct a witnessed value via the crate-controlled boundary.
    #[inline]
    fn witness(input: T) -> Result<Witnessed<T, Self>, Self::Error> {
        Witnessed::<T, Self>::try_new(input)
    }
}

/// A value of type `T` that carries an unforgeable witness `W`.
///
/// # Invariant & construction
///
/// `Witnessed<T, W>` can only be constructed through `W::witness` (or `Witnessed::try_new`),
/// which is expected to validate and optionally normalize the input. By keeping the internal
/// constructor crate-private, downstream crates cannot forge a `Witnessed` and must pass through
/// the witness boundary.
///
/// # Auto-traits (Send/Sync/Unpin) are driven by `T`
///
/// The marker field uses `PhantomData<fn() -> W>` intentionally. This encodes the witness at the
/// type level without *owning* a `W`, so auto-traits are not accidentally constrained by `W`.
///
/// Concretely, `Witnessed<T, W>` will be `Send`/`Sync` when `T` is `Send`/`Sync`, even if `W`
/// itself is not (e.g. it contains `Rc` or is otherwise `!Send`/`!Sync`). If you used
/// `PhantomData<W>` instead, `W`'s auto-traits would propagate to `Witnessed<T, W>` and
/// unnecessarily restrict it.
///
/// # Important note
///
/// This wrapper avoids exposing `&mut T`, but if `T` has interior mutability (e.g. `Cell`,
/// `RefCell`, `Mutex`), the invariant is only as strong as `T`'s own semantics.
#[repr(transparent)]
pub struct Witnessed<T, W: Witness<T>> {
    inner: T,
    _marker: PhantomData<fn() -> W>,
}

mod impls {
    use super::*;

    impl<T, W: Witness<T>> Witnessed<T, W> {
        /// Validate (and optionally normalize) `inner` via `W::attest`, then wrap it.
        ///
        /// This is the crate-controlled construction boundary: callers cannot forge a `Witnessed`
        /// without passing the witness check.
        #[inline]
        pub fn try_new(inner: T) -> Result<Self, W::Error> {
            W::attest(inner).map(Self::new_unchecked)
        }
    }

    #[cfg(test)]
    mod try_new_tests {
        use super::*;
        use core::sync::atomic::{AtomicUsize, Ordering};
        use std::{borrow::ToOwned, string::String, vec::Vec};

        // try_new success: returns Witnessed, and deref/as_ref can read the inner value
        struct Pos;
        #[derive(Debug, PartialEq, Eq)]
        enum PosErr {
            NonPos,
        }

        impl Witness<i32> for Pos {
            type Error = PosErr;
            fn attest(input: i32) -> Result<i32, Self::Error> {
                (input > 0).then_some(input).ok_or(PosErr::NonPos)
            }
        }

        #[test]
        fn try_new_ok_returns_witnessed_and_exposes_inner_readonly() {
            let w = Witnessed::<i32, Pos>::try_new(7).unwrap();
            assert_eq!(*w, 7);
            assert_eq!(w.as_ref(), &7);
        }

        // try_new failure: propagates the witness error unchanged
        #[test]
        fn try_new_err_propagates_witness_error() {
            let e = Witnessed::<i32, Pos>::try_new(0).unwrap_err();
            assert_eq!(e, PosErr::NonPos);
        }

        // try_new allows normalization: the witness can rewrite input (e.g. trim)
        struct TrimNonEmpty;
        #[derive(Debug, PartialEq, Eq)]
        enum StrErr {
            Empty,
        }

        impl Witness<String> for TrimNonEmpty {
            type Error = StrErr;
            fn attest(input: String) -> Result<String, Self::Error> {
                let s = input.trim().to_owned();
                (!s.is_empty()).then_some(s).ok_or(StrErr::Empty)
            }
        }

        #[test]
        fn try_new_allows_normalization_by_witness() {
            let w = Witnessed::<String, TrimNonEmpty>::try_new("  hi  ".into()).unwrap();
            assert_eq!(w.as_ref(), "hi");
        }

        #[test]
        fn try_new_normalization_can_still_fail() {
            let e = Witnessed::<String, TrimNonEmpty>::try_new("   ".into()).unwrap_err();
            assert_eq!(e, StrErr::Empty);
        }

        // try_new calls witness exactly once (avoiding duplicate checks/duplicate allocations)
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        struct CountOnce;
        #[derive(Debug, PartialEq, Eq)]
        enum CountErr {
            Nope,
        }

        impl Witness<u8> for CountOnce {
            type Error = CountErr;

            fn attest(input: u8) -> Result<u8, Self::Error> {
                CALLS.fetch_add(1, Ordering::Relaxed);
                (input == 1).then_some(input).ok_or(CountErr::Nope)
            }
        }

        #[test]
        fn try_new_calls_witness_exactly_once() {
            CALLS.store(0, Ordering::Relaxed);
            let _ = Witnessed::<u8, CountOnce>::try_new(1);
            assert_eq!(CALLS.load(Ordering::Relaxed), 1);

            let _ = Witnessed::<u8, CountOnce>::try_new(2);
            assert_eq!(CALLS.load(Ordering::Relaxed), 2);
        }

        // try_new supports a composite tuple (A, B, C) witnessed by a single W
        struct AbcW;

        #[derive(Debug, PartialEq, Eq)]
        enum AbcErr {
            AEmpty,
            BOdd { b: u32 },
            CNonAscii,
        }

        // Invariants:
        // - A (String) must be non-empty after trim; store trimmed
        // - B (u32) must be even
        // - C (Vec<u8>) must be ASCII bytes
        impl Witness<(String, u32, Vec<u8>)> for AbcW {
            type Error = AbcErr;

            fn attest(
                input: (String, u32, Vec<u8>),
            ) -> Result<(String, u32, Vec<u8>), Self::Error> {
                let (a, b, c) = input;

                let a = a.trim().to_owned();
                if a.is_empty() {
                    return Err(AbcErr::AEmpty);
                }
                if b % 2 != 0 {
                    return Err(AbcErr::BOdd { b });
                }
                if !c.is_ascii() {
                    return Err(AbcErr::CNonAscii);
                }

                Ok((a, b, c))
            }
        }

        #[test]
        fn try_new_composite_tuple_ok_and_normalizes_a() {
            let w = Witnessed::<(String, u32, Vec<u8>), AbcW>::try_new((
                "  hello  ".into(),
                42,
                b"ABC".to_vec(),
            ))
            .unwrap();

            // A normalized (trimmed)
            assert_eq!((w.as_ref().0).as_str(), "hello");
            // B preserved
            assert_eq!(w.as_ref().1, 42);
            // C preserved
            assert_eq!(w.as_ref().2.as_slice(), b"ABC");
        }

        #[test]
        fn try_new_composite_tuple_fails_on_each_invariant() {
            // A empty after trim
            let e = Witnessed::<(String, u32, Vec<u8>), AbcW>::try_new((
                "   ".into(),
                2,
                b"ABC".to_vec(),
            ))
            .unwrap_err();
            assert_eq!(e, AbcErr::AEmpty);

            // B odd
            let e = Witnessed::<(String, u32, Vec<u8>), AbcW>::try_new((
                "ok".into(),
                3,
                b"ABC".to_vec(),
            ))
            .unwrap_err();
            assert_eq!(e, AbcErr::BOdd { b: 3 });

            // C non-ascii
            let e =
                Witnessed::<(String, u32, Vec<u8>), AbcW>::try_new(("ok".into(), 4, vec![0xFF]))
                    .unwrap_err();
            assert_eq!(e, AbcErr::CNonAscii);
        }
    }

    impl<T, W: Witness<T>> Witnessed<T, W> {
        /// Consume and return the inner value.
        ///
        /// Note: extracting `T` loses the witness guarantee in the type system.
        #[inline]
        pub fn into_inner(self) -> T {
            self.inner
        }
    }
    impl<T, W: Witness<T>> Witnessed<T, W> {
        /// Internal constructor; keeps `Witnessed` unforgeable across crates.
        ///
        /// Do NOT make this public: the entire pattern relies on forcing construction through
        /// `W::attest` (via `try_new` / `W::witness`) so invariants cannot be bypassed downstream.
        #[inline]
        pub(crate) fn new_unchecked(inner: T) -> Self {
            Self {
                inner,
                _marker: PhantomData,
            }
        }
    }
}

mod impl_fors {
    use super::*;

    impl<T, W: Witness<T>> Deref for Witnessed<T, W> {
        type Target = T;
        #[inline]
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl<T, W: Witness<T>> AsRef<T> for Witnessed<T, W> {
        #[inline]
        fn as_ref(&self) -> &T {
            &self.inner
        }
    }

    impl<T: Clone, W: Witness<T>> Clone for Witnessed<T, W> {
        #[inline]
        fn clone(&self) -> Self {
            Self::new_unchecked(self.inner.clone())
        }
    }
    impl<T: Copy, W: Witness<T>> Copy for Witnessed<T, W> {}

    impl<T: core::fmt::Debug, W: Witness<T>> core::fmt::Debug for Witnessed<T, W> {
        #[inline]
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_tuple("Witnessed").field(&self.inner).finish()
        }
    }

    #[cfg(test)]
    mod witness_debug_tests {
        use std::{format, vec::Vec};

        use super::*;

        struct Any;
        impl Witness<i32> for Any {
            type Error = core::convert::Infallible;
            fn attest(input: i32) -> Result<i32, Self::Error> {
                Ok(input)
            }
        }

        struct AnyStr;
        impl Witness<&'static str> for AnyStr {
            type Error = core::convert::Infallible;
            fn attest(input: &'static str) -> Result<&'static str, Self::Error> {
                Ok(input)
            }
        }

        #[test]
        fn debug_fmt_matches_tuple_shape_without_regex() {
            let w = Witnessed::<i32, Any>::try_new(7).unwrap();
            let s = format!("{:?}", w);

            // Check the prefix and suffix directly, then verify the middle is numeric.
            assert!(s.starts_with("Witnessed("));
            assert!(s.ends_with(')'));

            // Extract the content between the parentheses.
            let inner = &s["Witnessed(".len()..s.len() - 1];
            assert!(
                inner.chars().all(|c| c.is_ascii_digit()),
                "Inner part should be digits, got: {}",
                inner
            );
            assert_eq!(inner, "7");
        }

        #[test]
        fn debug_fmt_preserves_inner_debug_repr_exactly() {
            let w = Witnessed::<&'static str, AnyStr>::try_new("hi").unwrap();
            let s = format!("{:?}", w);

            assert_eq!(s, "Witnessed(\"hi\")");
        }

        #[test]
        fn debug_fmt_complex_structure() {
            // Verify slightly more complex types to ensure nested Debug formatting works as expected.
            struct AnyVec;
            impl Witness<Vec<i32>> for AnyVec {
                type Error = core::convert::Infallible;

                fn attest(input: Vec<i32>) -> Result<Vec<i32>, Self::Error> {
                    Ok(input)
                }
            }

            let w = Witnessed::<Vec<i32>, AnyVec>::try_new(vec![1, 2]).unwrap();
            assert_eq!(format!("{:?}", w), "Witnessed([1, 2])");
        }
    }

    impl<T: PartialEq, W: Witness<T>> PartialEq for Witnessed<T, W> {
        #[inline]
        fn eq(&self, other: &Self) -> bool {
            self.inner.eq(&other.inner)
        }
    }
    impl<T: Eq, W: Witness<T>> Eq for Witnessed<T, W> {}

    #[cfg(test)]
    mod witness_eq_tests {
        use super::*;

        // A minimal witness that always succeeds.
        struct Any;
        impl Witness<i32> for Any {
            type Error = core::convert::Infallible;

            fn attest(input: i32) -> Result<i32, Self::Error> {
                Ok(input)
            }
        }

        #[test]
        fn partial_eq_compares_inner_only() {
            let a = Witnessed::<i32, Any>::try_new(1).unwrap();
            let b = Witnessed::<i32, Any>::try_new(1).unwrap();
            let c = Witnessed::<i32, Any>::try_new(2).unwrap();

            assert_eq!(a, b);
            assert_ne!(a, c);
        }

        #[test]
        fn eq_laws_hold_for_witnessed() {
            let a = Witnessed::<i32, Any>::try_new(3).unwrap();
            let b = Witnessed::<i32, Any>::try_new(3).unwrap();
            let c = Witnessed::<i32, Any>::try_new(3).unwrap();

            // Reflexive
            assert_eq!(a, a);

            // Symmetric
            assert_eq!(a == b, b == a);

            // Transitive
            assert!(a == b && b == c && a == c);
        }
    }

    impl<T: PartialOrd, W: Witness<T>> PartialOrd for Witnessed<T, W> {
        #[inline]
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            self.inner.partial_cmp(&other.inner)
        }
    }
    impl<T: Ord, W: Witness<T>> Ord for Witnessed<T, W> {
        #[inline]
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
            self.inner.cmp(&other.inner)
        }
    }

    #[cfg(test)]
    mod witness_ord_tests {
        use super::*;
        use core::cmp::Ordering;
        use std::vec::Vec;

        // A minimal witness that always succeeds.
        struct AnyI32;
        impl Witness<i32> for AnyI32 {
            type Error = core::convert::Infallible;

            fn attest(input: i32) -> Result<i32, Self::Error> {
                Ok(input)
            }
        }

        #[test]
        fn partial_ord_delegates_to_inner_for_total_ordered_types() {
            let a = Witnessed::<i32, AnyI32>::try_new(1).unwrap();
            let b = Witnessed::<i32, AnyI32>::try_new(2).unwrap();

            assert_eq!(a.partial_cmp(&b), Some(Ordering::Less));
            assert_eq!(b.partial_cmp(&a), Some(Ordering::Greater));
            assert_eq!(a.partial_cmp(&a), Some(Ordering::Equal));
        }

        #[test]
        fn ord_cmp_delegates_to_inner_and_sort_matches_inner_sort() {
            let mut v = [
                Witnessed::<i32, AnyI32>::try_new(3).unwrap(),
                Witnessed::<i32, AnyI32>::try_new(1).unwrap(),
                Witnessed::<i32, AnyI32>::try_new(2).unwrap(),
            ];
            v.sort(); // uses Ord::cmp

            let got: Vec<i32> = v.iter().map(|x| **x).collect();
            assert_eq!(got, vec![1, 2, 3]);
        }

        // --- PartialOrd None case (e.g., NaN) ---
        struct AnyF32;
        impl Witness<f32> for AnyF32 {
            type Error = core::convert::Infallible;

            fn attest(input: f32) -> Result<f32, Self::Error> {
                Ok(input)
            }
        }

        #[test]
        fn partial_ord_propagates_none_for_nan_like_inner() {
            let nan = Witnessed::<f32, AnyF32>::try_new(f32::NAN).unwrap();
            let one = Witnessed::<f32, AnyF32>::try_new(1.0).unwrap();

            assert_eq!(nan.partial_cmp(&one), None);
            assert_eq!(one.partial_cmp(&nan), None);
            assert_eq!(nan.partial_cmp(&nan), None);
        }
    }

    impl<T: core::hash::Hash, W: Witness<T>> core::hash::Hash for Witnessed<T, W> {
        #[inline]
        fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
            self.inner.hash(state)
        }
    }

    #[cfg(test)]
    mod witness_hash_tests {
        use super::*;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        struct Any;
        impl Witness<i32> for Any {
            type Error = core::convert::Infallible;

            fn attest(input: i32) -> Result<i32, Self::Error> {
                Ok(input)
            }
        }

        fn hash64(x: impl Hash) -> u64 {
            let mut h = DefaultHasher::new();
            x.hash(&mut h);
            h.finish()
        }

        #[test]
        fn hash_matches_inner_hash_exactly() {
            let w = Witnessed::<i32, Any>::try_new(42).unwrap();
            assert_eq!(hash64(&w), hash64(&42));
        }

        #[test]
        fn equal_inner_implies_equal_hash() {
            let a = Witnessed::<i32, Any>::try_new(7).unwrap();
            let b = Witnessed::<i32, Any>::try_new(7).unwrap();
            assert_eq!(hash64(&a), hash64(&b));
        }

        #[test]
        fn different_inner_usually_differs_in_hash_smoke() {
            let a = Witnessed::<i32, Any>::try_new(1).unwrap();
            let b = Witnessed::<i32, Any>::try_new(2).unwrap();
            assert_ne!(hash64(&a), hash64(&b));
        }
    }
}

#[cfg(test)]
mod witness_size_tests {
    use super::*;
    use core::mem;
    use std::{borrow::ToOwned, string::String, vec::Vec};

    struct TrimNonEmpty;
    #[derive(Debug, PartialEq, Eq)]
    enum StrErr {
        Empty,
    }

    impl Witness<String> for TrimNonEmpty {
        type Error = StrErr;

        fn attest(input: String) -> Result<String, Self::Error> {
            let s = input.trim().to_owned();
            (!s.is_empty()).then_some(s).ok_or(StrErr::Empty)
        }
    }

    struct AnyVec;
    #[derive(Debug, PartialEq, Eq)]
    enum VecErr {
        EmptyVec,
    }

    impl Witness<Vec<u8>> for AnyVec {
        type Error = VecErr;

        fn attest(input: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
            if input.is_empty() {
                Err(VecErr::EmptyVec)
            } else {
                Ok(input)
            }
        }
    }

    struct Any;
    impl Witness<i32> for Any {
        type Error = core::convert::Infallible;

        fn attest(input: i32) -> Result<i32, Self::Error> {
            Ok(input)
        }
    }

    #[test]
    fn witnessed_size_is_equal_to_inner_size() {
        let _ = Witnessed::<i32, Any>::try_new(42).unwrap();

        // Verify the size of the wrapper type is the same as the inner type (i32).
        assert_eq!(mem::size_of::<Witnessed<i32, Any>>(), mem::size_of::<i32>());
    }

    #[test]
    fn witness_size_matches_for_different_types() {
        // Check with different types to ensure the size is still the same as the inner type.
        assert_eq!(mem::size_of::<Witnessed<i32, Any>>(), mem::size_of::<i32>());
        assert_eq!(
            mem::size_of::<Witnessed<String, TrimNonEmpty>>(),
            mem::size_of::<String>()
        );
        assert_eq!(
            mem::size_of::<Witnessed<Vec<u8>, AnyVec>>(),
            mem::size_of::<Vec<u8>>()
        );
    }

    #[test]
    fn witness_size_for_vec() {
        // Check the size for Vec<u8> type with the AnyVec witness.
        let _ = Witnessed::<Vec<u8>, AnyVec>::try_new(vec![1, 2, 3]).unwrap();
        // Verify the size of the wrapper type is the same as the inner type (Vec<u8>).
        assert_eq!(
            mem::size_of::<Witnessed<Vec<u8>, AnyVec>>(),
            mem::size_of::<Vec<u8>>()
        );
    }
}

#[cfg(test)]
mod demo {
    use super::*;

    // Invariant: idx < 3
    struct IdxLt3;
    #[derive(Debug, PartialEq, Eq)]
    enum IdxErr {
        OutOfRange { idx: usize },
    }

    impl Witness<usize> for IdxLt3 {
        type Error = IdxErr;

        fn attest(input: usize) -> Result<usize, Self::Error> {
            (input < 3)
                .then_some(input)
                .ok_or(IdxErr::OutOfRange { idx: input })
        }
    }

    // --- decoupled steps (simulate different modules) ---
    fn parse_idx(s: &str) -> usize {
        s.parse().unwrap()
    }
    fn compute_idx(raw: usize) -> usize {
        raw + 2 // business rule; can push it out of range
    }

    // consumer that *assumes* idx is valid (will panic if not)
    fn pick_unchecked(xs: &[i32; 3], idx: usize) -> i32 {
        xs[idx]
    }

    // boundary function: upgrades plain idx -> witnessed idx
    fn compute_idx_witnessed(raw: usize) -> Result<Witnessed<usize, IdxLt3>, IdxErr> {
        Witnessed::<usize, IdxLt3>::try_new(compute_idx(raw))
    }

    // consumer that requires the invariant in its type signature
    fn pick_checked(xs: &[i32; 3], idx: Witnessed<usize, IdxLt3>) -> i32 {
        xs[*idx]
    }

    #[test]
    #[should_panic]
    fn without_witness_decoupling_can_panic() {
        let xs = [10, 20, 30];

        // Decoupled: parse -> compute -> consume
        // "2" -> 2; +2 -> 4; xs[4] panics.
        let idx = compute_idx(parse_idx("2"));
        let _ = pick_unchecked(&xs, idx);
    }

    #[test]
    fn with_witness_panic_becomes_impossible_in_checked_path() {
        let xs = [10, 20, 30];

        // Same input, but we put the check at the boundary:
        // "2" -> 2; +2 -> 4; witnessing fails => Err; no panic.
        let bad = compute_idx_witnessed(parse_idx("2"));
        assert_eq!(bad, Err(IdxErr::OutOfRange { idx: 4 }));

        // For a valid case, witnessing succeeds and the checked consumer is safe.
        // "0" -> 0; +2 -> 2; xs[2] = 30
        let ok = compute_idx_witnessed(parse_idx("0")).unwrap();
        assert_eq!(pick_checked(&xs, ok), 30);

        // And importantly: you cannot accidentally do this:
        // pick_checked(&xs, 2);
        // ^ type mismatch: expected Witnessed<usize, IdxLt3>, found usize
    }
}
