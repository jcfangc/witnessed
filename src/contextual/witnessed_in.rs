use core::marker::PhantomData;

use crate::contextual::WitnessIn;

/// A value of type `T` that carries an unforgeable, *environment-dependent* witness `W`
/// under some environment `Env`.
///
/// This is the contextual counterpart of [`Witnessed<T, W>`]:
///
/// - [`Witnessed<T, W>`] models an *absolute* invariant `W(T)`.
/// - `WitnessedIn<'a, Env, T, W>` models a *relative* invariant `W(Env, T)`.
///
/// In other words, the carried proof should be read as:
///
/// > `inner` satisfies `W` **with respect to** some environment `env: &'a Env`.
///
/// # Invariant & construction
///
/// `WitnessedIn<'a, Env, T, W>` can only be constructed through the crate-controlled witness
/// boundary (for example `W::witness_in` or `WitnessedIn::try_new_in`), which must validate
/// `inner` against a concrete `&'a Env`.
///
/// The actual environment reference is **not stored at runtime**. Instead, the dependency on
/// `&'a Env` is encoded purely at the type level. This keeps the runtime representation minimal
/// while still allowing the compiler to track that the proof is only valid for values tied to
/// that environment lifetime.
///
/// # Why `PhantomData<fn(&'a Env) -> W>`
///
/// The marker field is intentionally written as:
///
/// `PhantomData<fn(&'a Env) -> W>`
///
/// rather than forms such as:
///
/// - `PhantomData<&'a Env>`
/// - `PhantomData<(&'a Env, W)>`
/// - `PhantomData<W>`
///
/// This choice serves two purposes:
///
/// 1. It binds the type to the environment lifetime `'a` and environment type `Env`
///    **without actually storing** an `&'a Env`.
/// 2. It mentions `W` only at the type level, without treating `WitnessedIn` as if it
///    *owned* a `W`.
///
/// Semantically, this says:
///
/// > this type is parameterized by a proof relation from `&'a Env` to witness `W`,
/// > but does not physically contain either one.
///
/// This is exactly what we want for a contextual proof wrapper:
///
/// - the proof is *about* an environment,
/// - the value does not need to store that environment,
/// - and the witness type should not impose ownership-style auto-trait constraints.
///
/// # Why the environment is not stored
///
/// A contextual witness often only needs the environment at the *construction boundary*,
/// where validation happens. After that, the runtime payload can remain just `T`.
///
/// Keeping `env` out of the runtime layout has several advantages:
///
/// - no extra pointer/reference field,
/// - no need to carry the environment through every move at runtime,
/// - and a cleaner separation between:
///   - runtime data: `inner`,
///   - compile-time proof context: `&'a Env`, `W`.
///
/// # Auto-traits
///
/// `WitnessedIn` should behave like a wrapper around `T`, while still being logically tied
/// to `&'a Env`.
///
/// Because the marker uses `PhantomData<fn(&'a Env) -> W>`:
///
/// - `Env` and `'a` participate in type/lifetime checking,
/// - but `WitnessedIn` does **not** behave as though it owns an `&'a Env`,
/// - and it does **not** behave as though it owns a `W`.
///
/// This is preferable to `PhantomData<W>`, which would make auto-traits propagate through `W`
/// as if a `W` value were actually stored.
///
/// # Important note
///
/// `WitnessedIn` prevents downstream code from forging the proof, but it does not magically
/// freeze the logical world around it.
///
/// If the meaning of `W(Env, T)` depends on facts about `env` that can later change, then the
/// guarantee is only as stable as that environment model. In practice, this pattern works best
/// when:
///
/// - `env` is immutable,
/// - or the validated relation is stable under later changes,
///
/// Also note that if `T` itself permits interior mutation, the invariant is only as strong as
/// the semantics exposed by `T`.
#[repr(transparent)]
pub struct WitnessedIn<'a, Env: ?Sized, T, W: WitnessIn<T, Env>> {
    /// The witnessed runtime value.
    inner: T,
    /// Type-level binding to the contextual witness relation `W(Env, T)`.
    ///
    /// This does not store either `&'a Env` or `W`; it only encodes their participation in the
    /// type system.
    _marker: PhantomData<fn(&'a Env) -> W>,
}

mod impls {
    use super::*;

    impl<'a, Env: ?Sized, T, W: WitnessIn<T, Env>> WitnessedIn<'a, Env, T, W> {
        /// Validate `inner` via `W::verify_in(env, ...)`, then construct a `WitnessedIn`
        /// tied to the same environment lifetime `'a`.
        ///
        /// The environment reference is **not stored at runtime**; it is only tracked
        /// at the type level so the resulting witness cannot outlive the environment
        /// used for validation.
        ///
        /// This is the crate-controlled construction boundary: callers cannot forge
        /// a `WitnessedIn` without passing the witness check relative to `env`.
        #[inline]
        pub fn try_new_in(env: &'a Env, inner: T) -> Result<Self, W::Error> {
            W::verify_in(env, &inner).map(|_| Self::new_unchecked(inner))
        }
    }

    #[cfg(test)]
    mod try_new_in_tests {
        use crate::contextual::test_support::{NormErr, Normalized};

        use super::*;
        use core::sync::atomic::{AtomicUsize, Ordering};
        use std::{string::String, vec::Vec};

        #[test]
        fn try_new_in_ok_for_member_and_env_sum_one() {
            let env = vec![0.2, 0.3, 0.5];
            let w = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.3).unwrap();

            assert_eq!(*w, 0.3);
            assert!(Normalized::verify_in(env.as_slice(), w.as_ref()).is_ok());
        }

        #[test]
        fn try_new_in_err_when_env_sum_not_one() {
            let env = vec![0.2, 0.3, 0.6]; // sum = 1.1
            let e =
                WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.3).unwrap_err();
            assert_eq!(e, NormErr::EnvSumNotOne { sum: 1.1 });
        }

        #[test]
        fn try_new_in_err_when_value_not_member() {
            let env = vec![0.2, 0.3, 0.5];
            let e =
                WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.4).unwrap_err();
            assert_eq!(e, NormErr::NotMember { x: 0.4 });
        }

        #[test]
        fn try_new_in_err_when_env_contains_non_finite() {
            let env = vec![0.2, f32::NAN, 0.8];
            let e =
                WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.2).unwrap_err();
            assert_eq!(e, NormErr::EnvNonFinite);
        }

        #[test]
        fn try_new_in_err_when_value_non_finite() {
            let env = vec![0.2, 0.3, 0.5];
            let e = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), f32::NAN)
                .unwrap_err();
            assert_eq!(e, NormErr::ValueNonFinite);
        }

        // verify_in called exactly once by try_new_in
        static CALLS: AtomicUsize = AtomicUsize::new(0);

        struct CountOnceNorm;
        impl WitnessIn<f32, [f32]> for CountOnceNorm {
            type Error = NormErr;

            #[inline]
            fn verify_in(env: &[f32], x: &f32) -> Result<(), Self::Error> {
                CALLS.fetch_add(1, Ordering::Relaxed);
                Normalized::verify_in(env, x)
            }
        }

        #[test]
        fn try_new_in_calls_witness_exactly_once() {
            let env = vec![0.2, 0.3, 0.5];

            CALLS.store(0, Ordering::Relaxed);
            let _ = WitnessedIn::<[f32], f32, CountOnceNorm>::try_new_in(env.as_slice(), 0.2);
            assert_eq!(CALLS.load(Ordering::Relaxed), 1);

            let _ = WitnessedIn::<[f32], f32, CountOnceNorm>::try_new_in(env.as_slice(), 0.9);
            assert_eq!(CALLS.load(Ordering::Relaxed), 2);
        }

        // composite tuple witness relative to env (e.g., max length)
        #[derive(Clone, Copy)]
        struct MaxLen {
            max: usize,
        }

        struct StrNonEmptyAndMax;
        #[derive(Debug, PartialEq, Eq)]
        enum StrMaxErr {
            Empty,
            TooLong { len: usize, max: usize },
        }

        impl WitnessIn<String, MaxLen> for StrNonEmptyAndMax {
            type Error = StrMaxErr;

            fn verify_in(env: &MaxLen, s: &String) -> Result<(), Self::Error> {
                if s.is_empty() {
                    return Err(StrMaxErr::Empty);
                }
                let len = s.len();
                (len <= env.max)
                    .then_some(())
                    .ok_or(StrMaxErr::TooLong { len, max: env.max })
            }
        }

        #[test]
        fn try_new_in_env_dependent_string_invariant_ok() {
            let env = MaxLen { max: 5 };
            let w =
                WitnessedIn::<MaxLen, String, StrNonEmptyAndMax>::try_new_in(&env, "hello".into())
                    .unwrap();

            assert_eq!(w.as_ref(), "hello");
            assert!(StrNonEmptyAndMax::verify_in(&env, w.as_ref()).is_ok());
        }

        #[test]
        fn try_new_in_env_dependent_string_invariant_fails() {
            let env = MaxLen { max: 5 };

            let e = WitnessedIn::<MaxLen, String, StrNonEmptyAndMax>::try_new_in(&env, "".into())
                .unwrap_err();
            assert_eq!(e, StrMaxErr::Empty);

            let e = WitnessedIn::<MaxLen, String, StrNonEmptyAndMax>::try_new_in(
                &env,
                "toolong".into(),
            )
            .unwrap_err();
            assert_eq!(
                e,
                StrMaxErr::TooLong {
                    len: "toolong".len(),
                    max: 5
                }
            );
        }

        // a slightly richer composite: (String, u32, Vec<u8>) checked under env.max_a_len
        #[derive(Clone, Copy)]
        struct AbcEnv {
            max_a_len: usize,
        }

        struct AbcIn;
        #[derive(Debug, PartialEq, Eq)]
        enum AbcInErr {
            AEmpty,
            ATooLong { len: usize, max: usize },
            BOdd { b: u32 },
            CNonAscii,
        }

        impl WitnessIn<(String, u32, Vec<u8>), AbcEnv> for AbcIn {
            type Error = AbcInErr;

            fn verify_in(env: &AbcEnv, input: &(String, u32, Vec<u8>)) -> Result<(), Self::Error> {
                let (a, b, c) = input;

                if a.trim().is_empty() {
                    return Err(AbcInErr::AEmpty);
                }
                let len = a.len();
                if len > env.max_a_len {
                    return Err(AbcInErr::ATooLong {
                        len,
                        max: env.max_a_len,
                    });
                }
                if b % 2 != 0 {
                    return Err(AbcInErr::BOdd { b: *b });
                }
                if !c.is_ascii() {
                    return Err(AbcInErr::CNonAscii);
                }
                Ok(())
            }
        }

        #[test]
        fn try_new_in_composite_tuple_ok() {
            let env = AbcEnv { max_a_len: 16 };
            let w = WitnessedIn::<AbcEnv, (String, u32, Vec<u8>), AbcIn>::try_new_in(
                &env,
                ("  hello  ".into(), 42, b"ABC".to_vec()),
            )
            .unwrap();

            assert_eq!((w.as_ref().0).as_str(), "  hello  "); // no normalization
            assert_eq!(w.as_ref().1, 42);
            assert_eq!(w.as_ref().2.as_slice(), b"ABC");
            assert!(AbcIn::verify_in(&env, w.as_ref()).is_ok());
        }

        #[test]
        fn try_new_in_composite_tuple_fails_on_each_invariant() {
            let env = AbcEnv { max_a_len: 16 };

            let e = WitnessedIn::<AbcEnv, (String, u32, Vec<u8>), AbcIn>::try_new_in(
                &env,
                ("   ".into(), 2, b"ABC".to_vec()),
            )
            .unwrap_err();
            assert_eq!(e, AbcInErr::AEmpty);

            let e = WitnessedIn::<AbcEnv, (String, u32, Vec<u8>), AbcIn>::try_new_in(
                &env,
                ("ok".into(), 3, b"ABC".to_vec()),
            )
            .unwrap_err();
            assert_eq!(e, AbcInErr::BOdd { b: 3 });

            let e = WitnessedIn::<AbcEnv, (String, u32, Vec<u8>), AbcIn>::try_new_in(
                &env,
                ("ok".into(), 4, vec![0xFF]),
            )
            .unwrap_err();
            assert_eq!(e, AbcInErr::CNonAscii);

            let e = WitnessedIn::<AbcEnv, (String, u32, Vec<u8>), AbcIn>::try_new_in(
                &env,
                ("this string is way too long".into(), 4, b"ABC".to_vec()),
            )
            .unwrap_err();
            assert_eq!(
                e,
                AbcInErr::ATooLong {
                    len: "this string is way too long".len(),
                    max: 16
                }
            );
        }
    }

    impl<'a, Env: ?Sized, T, W: WitnessIn<T, Env>> WitnessedIn<'a, Env, T, W> {
        /// Consume and return the inner value.
        ///
        /// Note: extracting `T` loses the witness guarantee in the type system.
        #[inline]
        pub fn into_inner(self) -> T {
            self.inner
        }
    }

    impl<'a, Env: ?Sized, T, W: WitnessIn<T, Env>> WitnessedIn<'a, Env, T, W> {
        /// Internal constructor; keeps `WitnessedIn` unforgeable across crates.
        ///
        /// Do NOT make this public: the entire pattern relies on forcing construction through
        /// `W::verify_in` (via `try_new_in` / `W::witness_in`) so invariants cannot be bypassed
        /// downstream.
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
    use core::{fmt, hash, ops::Deref};

    use super::*;

    impl<'a, Env: ?Sized, T, W: WitnessIn<T, Env>> Deref for WitnessedIn<'a, Env, T, W> {
        type Target = T;
        #[inline]
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl<'a, Env: ?Sized, T, W: WitnessIn<T, Env>> AsRef<T> for WitnessedIn<'a, Env, T, W> {
        #[inline]
        fn as_ref(&self) -> &T {
            &self.inner
        }
    }

    impl<'a, Env: ?Sized, T: Clone, W: WitnessIn<T, Env>> Clone for WitnessedIn<'a, Env, T, W> {
        #[inline]
        fn clone(&self) -> Self {
            Self::new_unchecked(self.inner.clone())
        }
    }

    impl<'a, Env: ?Sized, T: Copy, W: WitnessIn<T, Env>> Copy for WitnessedIn<'a, Env, T, W> {}

    impl<'a, Env: ?Sized, T: fmt::Debug, W: WitnessIn<T, Env>> fmt::Debug
        for WitnessedIn<'a, Env, T, W>
    {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            // Intentionally omit env from Debug to keep output stable and avoid leaking internals.
            f.debug_tuple("WitnessedIn").field(&self.inner).finish()
        }
    }

    #[cfg(test)]
    mod witnessed_in_debug_tests {
        use crate::contextual::test_support::Normalized;

        use super::*;
        use std::format;

        #[test]
        fn debug_fmt_matches_tuple_shape_without_regex() {
            let env = vec![0.2, 0.3, 0.5];
            let w = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.3).unwrap();
            let s = format!("{:?}", w);

            assert!(s.starts_with("WitnessedIn("));
            assert!(s.ends_with(')'));

            let inner = &s["WitnessedIn(".len()..s.len() - 1];
            assert!(
                inner.chars().all(|c| c.is_ascii_digit() || c == '.'),
                "Inner part should look numeric, got: {inner}"
            );
        }

        #[test]
        fn debug_fmt_preserves_inner_debug_repr_exactly() {
            let env = vec![0.2, 0.3, 0.5];
            let w = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.3).unwrap();
            assert_eq!(format!("{:?}", w), "WitnessedIn(0.3)");
        }
    }

    impl<'a, Env: ?Sized, T: PartialEq, W: WitnessIn<T, Env>> PartialEq for WitnessedIn<'a, Env, T, W> {
        #[inline]
        fn eq(&self, other: &Self) -> bool {
            self.inner.eq(&other.inner)
        }
    }
    impl<'a, Env: ?Sized, T: Eq, W: WitnessIn<T, Env>> Eq for WitnessedIn<'a, Env, T, W> {}

    #[cfg(test)]
    mod witnessed_in_eq_tests {
        use crate::contextual::test_support::Normalized;

        use super::*;

        #[test]
        fn partial_eq_compares_inner_only_same_env() {
            let env = vec![0.2, 0.3, 0.5];
            let a = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.3).unwrap();
            let b = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.3).unwrap();
            let c = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.2).unwrap();

            assert_eq!(a, b);
            assert_ne!(a, c);
        }

        #[test]
        fn eq_laws_hold_for_witnessed_in() {
            let env = vec![0.2, 0.3, 0.5];
            let a = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.2).unwrap();
            let b = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.2).unwrap();
            let c = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.2).unwrap();

            // Reflexive
            assert_eq!(a, a);
            // Symmetric
            assert_eq!(a == b, b == a);
            // Transitive
            assert!(a == b && b == c && a == c);
        }

        #[test]
        fn equality_ignores_env_identity_by_design() {
            // Two different env values, both valid, both containing x.
            // PartialEq intentionally compares inner only, not the construction environment.
            let env1 = vec![0.2, 0.3, 0.5];
            let env2 = vec![0.3, 0.2, 0.5];

            let a =
                WitnessedIn::<[f32], f32, Normalized>::try_new_in(env1.as_slice(), 0.3).unwrap();
            let b =
                WitnessedIn::<[f32], f32, Normalized>::try_new_in(env2.as_slice(), 0.3).unwrap();

            assert_eq!(a, b);
        }

        #[test]
        fn equality_does_not_require_env_to_contain_both_values() {
            // Demonstrates that equality is purely structural on inner: it doesn't re-validate
            // cross-env semantics, and doesn't require the two envs to be identical.
            let env1 = vec![0.2, 0.3, 0.5];
            let env2 = vec![0.1, 0.3, 0.6];

            let a =
                WitnessedIn::<[f32], f32, Normalized>::try_new_in(env1.as_slice(), 0.3).unwrap();
            let b =
                WitnessedIn::<[f32], f32, Normalized>::try_new_in(env2.as_slice(), 0.3).unwrap();

            assert_eq!(a, b);
        }
    }

    impl<'a, Env: ?Sized, T: PartialOrd, W: WitnessIn<T, Env>> PartialOrd
        for WitnessedIn<'a, Env, T, W>
    {
        #[inline]
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            self.inner.partial_cmp(&other.inner)
        }
    }
    impl<'a, Env: ?Sized, T: Ord, W: WitnessIn<T, Env>> Ord for WitnessedIn<'a, Env, T, W> {
        #[inline]
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
            self.inner.cmp(&other.inner)
        }
    }

    #[cfg(test)]
    mod witnessed_in_ord_tests {
        use crate::contextual::test_support::AnyIn;

        use super::*;
        use core::cmp::Ordering;
        use std::vec::Vec;

        #[test]
        fn partial_ord_delegates_to_inner_for_total_ordered_types() {
            let env = vec![1, 2, 3];
            let a = WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 1).unwrap();
            let b = WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 2).unwrap();

            assert_eq!(a.partial_cmp(&b), Some(Ordering::Less));
            assert_eq!(b.partial_cmp(&a), Some(Ordering::Greater));
            assert_eq!(a.partial_cmp(&a), Some(Ordering::Equal));
        }

        #[test]
        fn ord_cmp_delegates_to_inner_and_sort_matches_inner_sort() {
            let env = vec![1, 2, 3];
            let mut v = [
                WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 3).unwrap(),
                WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 1).unwrap(),
                WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 2).unwrap(),
            ];
            v.sort();

            let got: Vec<i32> = v.iter().map(|x| **x).collect();
            assert_eq!(got, vec![1, 2, 3]);
        }

        // --- PartialOrd None case (e.g., NaN) ---
        struct AnyF32In;
        #[derive(Debug, PartialEq)]
        enum AnyF32Err {
            EmptyEnv,
        }
        impl WitnessIn<f32, [f32]> for AnyF32In {
            type Error = AnyF32Err;
            fn verify_in(env: &[f32], _x: &f32) -> Result<(), Self::Error> {
                (!env.is_empty()).then_some(()).ok_or(AnyF32Err::EmptyEnv)
            }
        }

        #[test]
        fn partial_ord_propagates_none_for_nan_like_inner() {
            let env = vec![0.0]; // non-empty env so construction succeeds
            let nan =
                WitnessedIn::<[f32], f32, AnyF32In>::try_new_in(env.as_slice(), f32::NAN).unwrap();
            let one = WitnessedIn::<[f32], f32, AnyF32In>::try_new_in(env.as_slice(), 1.0).unwrap();

            assert_eq!(nan.partial_cmp(&one), None);
            assert_eq!(one.partial_cmp(&nan), None);
            assert_eq!(nan.partial_cmp(&nan), None);
        }
    }

    impl<'a, Env: ?Sized, T: hash::Hash, W: WitnessIn<T, Env>> hash::Hash
        for WitnessedIn<'a, Env, T, W>
    {
        #[inline]
        fn hash<H: hash::Hasher>(&self, state: &mut H) {
            self.inner.hash(state)
        }
    }

    #[cfg(test)]
    mod witnessed_in_hash_tests {
        use crate::contextual::test_support::AnyIn;

        use super::*;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash64(x: impl Hash) -> u64 {
            let mut h = DefaultHasher::new();
            x.hash(&mut h);
            h.finish()
        }

        #[test]
        fn hash_matches_inner_hash_exactly() {
            let env = vec![1];
            let w = WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 42).unwrap();
            assert_eq!(hash64(&w), hash64(&42));
        }

        #[test]
        fn equal_inner_implies_equal_hash() {
            let env = vec![1];
            let a = WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 7).unwrap();
            let b = WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 7).unwrap();
            assert_eq!(hash64(&a), hash64(&b));
        }

        #[test]
        fn different_inner_usually_differs_in_hash_smoke() {
            let env = vec![1];
            let a = WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 1).unwrap();
            let b = WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env.as_slice(), 2).unwrap();
            assert_ne!(hash64(&a), hash64(&b));
        }

        #[test]
        fn hash_ignores_env_identity_by_design() {
            let env1 = vec![1];
            let env2 = vec![2];

            let a = WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env1.as_slice(), 7).unwrap();
            let b = WitnessedIn::<[i32], i32, AnyIn>::try_new_in(env2.as_slice(), 7).unwrap();

            assert_eq!(hash64(&a), hash64(&b));
        }
    }
}
