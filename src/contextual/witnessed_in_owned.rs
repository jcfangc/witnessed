use core::marker::PhantomData;

use crate::contextual::WitnessIn;

/// A value of type `T` that carries an unforgeable, *environment-dependent* witness `W`
/// under some environment type `Env`, without storing either the environment or a borrow
/// of it at runtime.
///
/// This is the existential / owned-handle counterpart of [`WitnessedInRef<'a, Env, T, W>`].
///
/// - [`Witnessed<T, W>`] models an absolute invariant `W(T)`.
/// - [`WitnessedInRef<'a, Env, T, W>`] models a borrowed contextual invariant `W(&'a Env, T)`.
/// - `WitnessedInOwned<Env, T, W>` models a contextual invariant validated against some
///   environment of type `Env`, but without tying the resulting wrapper to a borrow lifetime.
///
/// In other words, the carried proof should be read as:
///
/// > `inner` satisfies `W` with respect to some environment of type `Env` used at
/// > the construction boundary.
///
/// # Invariant & construction
///
/// `WitnessedInOwned<Env, T, W>` can only be constructed through the crate-controlled witness
/// boundary (for example `WitnessedInOwned::try_new_in`), which must validate `inner` against
/// a concrete `&Env`.
///
/// The environment is **not stored at runtime**. Instead, the dependency on `Env` is encoded
/// purely at the type level. This keeps the runtime representation minimal while still making
/// the contextual proof part of the type.
///
/// # Why `PhantomData<fn(Env) -> W>`
///
/// The marker field is intentionally written as:
///
/// `PhantomData<fn(Env) -> W>`
///
/// rather than forms such as:
///
/// - `PhantomData<Env>`
/// - `PhantomData<(Env, W)>`
/// - `PhantomData<W>`
///
/// This choice expresses a pure type-level relation:
///
/// - the wrapper is parameterized by `Env`,
/// - the witness type `W` participates in the proof,
/// - but the wrapper behaves as though it owns neither `Env` nor `W`.
///
/// This preserves the intended "transparent wrapper around `T`" behavior.
///
/// # Why the environment is not stored
///
/// A contextual witness often only needs the environment at the validation boundary.
/// After validation, the runtime payload can remain just `T`.
///
/// Keeping `Env` out of the runtime layout means:
///
/// - no extra field,
/// - no runtime overhead from carrying an env handle,
/// - no loss of `#[repr(transparent)]`,
/// - a clean separation between runtime data (`inner`) and proof context (`Env`, `W`).
///
/// # Important note
///
/// `WitnessedInOwned` prevents downstream code from forging the proof, but it does not freeze
/// the logical world behind `Env`.
///
/// If the meaning of `W(Env, T)` depends on mutable or otherwise unstable external facts, then
/// the guarantee is only as stable as that environment model.
///
/// This pattern works best when:
///
/// - `Env` itself is immutable,
/// - or validation against `Env` is stable once established,
/// - and `T` does not later mutate in a way that would invalidate the witnessed relation.
#[repr(transparent)]
pub struct WitnessedInOwned<Env, T, W: WitnessIn<T, Env>> {
    /// The witnessed runtime value.
    inner: T,
    /// Type-level binding to the contextual witness relation `W(Env, T)`.
    ///
    /// This stores neither `Env` nor `W`; it only encodes their participation in the type
    /// system.
    _marker: PhantomData<fn(Env) -> W>,
}

mod impls {
    use super::*;

    impl<Env, T, W: WitnessIn<T, Env>> WitnessedInOwned<Env, T, W> {
        /// Validate `inner` via `W::verify_in(env, ...)`, then construct a `WitnessedInOwned`.
        ///
        /// The environment is used only at the validation boundary and is not stored at runtime.
        ///
        /// This is the crate-controlled construction boundary: callers cannot forge
        /// a `WitnessedInOwned` without passing the witness check relative to `env`.
        #[inline]
        pub fn try_new_in(env: &Env, inner: T) -> Result<Self, W::Error> {
            W::verify_in(env, &inner).map(|_| Self::new_unchecked(inner))
        }
    }

    #[cfg(test)]
    mod try_new_in_tests {
        use crate::contextual::test_support::{NormErr, NormalizedOwned};

        use super::*;
        use core::sync::atomic::{AtomicUsize, Ordering};
        use std::{string::String, sync::Arc, vec::Vec};

        #[test]
        fn try_new_in_ok_for_member_and_env_sum_one() {
            let env: Arc<[f32]> = vec![0.2, 0.3, 0.5].into();
            let w = WitnessedInOwned::<Arc<[f32]>, f32, NormalizedOwned>::try_new_in(&env, 0.3)
                .unwrap();

            assert_eq!(*w, 0.3);
            assert!(NormalizedOwned::verify_in(&env, w.as_ref()).is_ok());
        }

        #[test]
        fn try_new_in_err_when_env_sum_not_one() {
            let env: Arc<[f32]> = vec![0.2, 0.3, 0.6].into(); // sum = 1.1
            let e = WitnessedInOwned::<Arc<[f32]>, f32, NormalizedOwned>::try_new_in(&env, 0.3)
                .unwrap_err();
            assert_eq!(e, NormErr::EnvSumNotOne { sum: 1.1 });
        }

        #[test]
        fn try_new_in_err_when_value_not_member() {
            let env: Arc<[f32]> = vec![0.2, 0.3, 0.5].into();
            let e = WitnessedInOwned::<Arc<[f32]>, f32, NormalizedOwned>::try_new_in(&env, 0.4)
                .unwrap_err();
            assert_eq!(e, NormErr::NotMember { x: 0.4 });
        }

        #[test]
        fn try_new_in_err_when_env_contains_non_finite() {
            let env: Arc<[f32]> = vec![0.2, f32::NAN, 0.8].into();
            let e = WitnessedInOwned::<Arc<[f32]>, f32, NormalizedOwned>::try_new_in(&env, 0.2)
                .unwrap_err();
            assert_eq!(e, NormErr::EnvNonFinite);
        }

        #[test]
        fn try_new_in_err_when_value_non_finite() {
            let env: Arc<[f32]> = vec![0.2, 0.3, 0.5].into();
            let e =
                WitnessedInOwned::<Arc<[f32]>, f32, NormalizedOwned>::try_new_in(&env, f32::NAN)
                    .unwrap_err();
            assert_eq!(e, NormErr::ValueNonFinite);
        }

        static CALLS: AtomicUsize = AtomicUsize::new(0);

        struct CountOnceNorm;
        impl WitnessIn<f32, Arc<[f32]>> for CountOnceNorm {
            type Error = NormErr;

            #[inline]
            fn verify_in(env: &Arc<[f32]>, x: &f32) -> Result<(), Self::Error> {
                CALLS.fetch_add(1, Ordering::Relaxed);
                NormalizedOwned::verify_in(env, x)
            }
        }

        #[test]
        fn try_new_in_calls_witness_exactly_once() {
            let env: Arc<[f32]> = vec![0.2, 0.3, 0.5].into();

            CALLS.store(0, Ordering::Relaxed);
            let _ = WitnessedInOwned::<Arc<[f32]>, f32, CountOnceNorm>::try_new_in(&env, 0.2);
            assert_eq!(CALLS.load(Ordering::Relaxed), 1);

            let _ = WitnessedInOwned::<Arc<[f32]>, f32, CountOnceNorm>::try_new_in(&env, 0.9);
            assert_eq!(CALLS.load(Ordering::Relaxed), 2);
        }

        #[derive(Clone)]
        struct MaxLen {
            max: usize,
        }

        struct StrNonEmptyAndMax;
        #[derive(Debug, PartialEq, Eq)]
        enum StrMaxErr {
            Empty,
            TooLong { len: usize, max: usize },
        }

        impl WitnessIn<String, Arc<MaxLen>> for StrNonEmptyAndMax {
            type Error = StrMaxErr;

            fn verify_in(env: &Arc<MaxLen>, s: &String) -> Result<(), Self::Error> {
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
            let env = Arc::new(MaxLen { max: 5 });
            let w = WitnessedInOwned::<Arc<MaxLen>, String, StrNonEmptyAndMax>::try_new_in(
                &env,
                "hello".into(),
            )
            .unwrap();

            assert_eq!(w.as_ref(), "hello");
            assert!(StrNonEmptyAndMax::verify_in(&env, w.as_ref()).is_ok());
        }

        #[test]
        fn try_new_in_env_dependent_string_invariant_fails() {
            let env = Arc::new(MaxLen { max: 5 });

            let e = WitnessedInOwned::<Arc<MaxLen>, String, StrNonEmptyAndMax>::try_new_in(
                &env,
                "".into(),
            )
            .unwrap_err();
            assert_eq!(e, StrMaxErr::Empty);

            let e = WitnessedInOwned::<Arc<MaxLen>, String, StrNonEmptyAndMax>::try_new_in(
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

        #[derive(Clone)]
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

        impl WitnessIn<(String, u32, Vec<u8>), Arc<AbcEnv>> for AbcIn {
            type Error = AbcInErr;

            fn verify_in(
                env: &Arc<AbcEnv>,
                input: &(String, u32, Vec<u8>),
            ) -> Result<(), Self::Error> {
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
            let env = Arc::new(AbcEnv { max_a_len: 16 });
            let w = WitnessedInOwned::<Arc<AbcEnv>, (String, u32, Vec<u8>), AbcIn>::try_new_in(
                &env,
                ("  hello  ".into(), 42, b"ABC".to_vec()),
            )
            .unwrap();

            assert_eq!((w.as_ref().0).as_str(), "  hello  ");
            assert_eq!(w.as_ref().1, 42);
            assert_eq!(w.as_ref().2.as_slice(), b"ABC");
            assert!(AbcIn::verify_in(&env, w.as_ref()).is_ok());
        }

        #[test]
        fn try_new_in_composite_tuple_fails_on_each_invariant() {
            let env = Arc::new(AbcEnv { max_a_len: 16 });

            let e = WitnessedInOwned::<Arc<AbcEnv>, (String, u32, Vec<u8>), AbcIn>::try_new_in(
                &env,
                ("   ".into(), 2, b"ABC".to_vec()),
            )
            .unwrap_err();
            assert_eq!(e, AbcInErr::AEmpty);

            let e = WitnessedInOwned::<Arc<AbcEnv>, (String, u32, Vec<u8>), AbcIn>::try_new_in(
                &env,
                ("ok".into(), 3, b"ABC".to_vec()),
            )
            .unwrap_err();
            assert_eq!(e, AbcInErr::BOdd { b: 3 });

            let e = WitnessedInOwned::<Arc<AbcEnv>, (String, u32, Vec<u8>), AbcIn>::try_new_in(
                &env,
                ("ok".into(), 4, vec![0xFF]),
            )
            .unwrap_err();
            assert_eq!(e, AbcInErr::CNonAscii);

            let e = WitnessedInOwned::<Arc<AbcEnv>, (String, u32, Vec<u8>), AbcIn>::try_new_in(
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

    impl<Env, T, W: WitnessIn<T, Env>> WitnessedInOwned<Env, T, W> {
        /// Consume and return the inner value.
        ///
        /// Note: extracting `T` loses the witness guarantee in the type system.
        #[inline]
        pub fn into_inner(self) -> T {
            self.inner
        }

        /// Internal constructor; keeps `WitnessedInOwned` unforgeable across crates.
        ///
        /// Do NOT make this public: the entire pattern relies on forcing construction through
        /// `W::verify_in` so invariants cannot be bypassed downstream.
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

    impl<Env, T, W: WitnessIn<T, Env>> Deref for WitnessedInOwned<Env, T, W> {
        type Target = T;

        #[inline]
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl<Env, T, W: WitnessIn<T, Env>> AsRef<T> for WitnessedInOwned<Env, T, W> {
        #[inline]
        fn as_ref(&self) -> &T {
            &self.inner
        }
    }

    impl<Env, T: Clone, W: WitnessIn<T, Env>> Clone for WitnessedInOwned<Env, T, W> {
        #[inline]
        fn clone(&self) -> Self {
            Self::new_unchecked(self.inner.clone())
        }
    }

    impl<Env, T: Copy, W: WitnessIn<T, Env>> Copy for WitnessedInOwned<Env, T, W> {}

    impl<Env, T: fmt::Debug, W: WitnessIn<T, Env>> fmt::Debug for WitnessedInOwned<Env, T, W> {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_tuple("WitnessedInOwned")
                .field(&self.inner)
                .finish()
        }
    }

    impl<Env, T: PartialEq, W: WitnessIn<T, Env>> PartialEq for WitnessedInOwned<Env, T, W> {
        #[inline]
        fn eq(&self, other: &Self) -> bool {
            self.inner.eq(&other.inner)
        }
    }

    impl<Env, T: Eq, W: WitnessIn<T, Env>> Eq for WitnessedInOwned<Env, T, W> {}

    impl<Env, T: PartialOrd, W: WitnessIn<T, Env>> PartialOrd for WitnessedInOwned<Env, T, W> {
        #[inline]
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            self.inner.partial_cmp(&other.inner)
        }
    }

    impl<Env, T: Ord, W: WitnessIn<T, Env>> Ord for WitnessedInOwned<Env, T, W> {
        #[inline]
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
            self.inner.cmp(&other.inner)
        }
    }

    impl<Env, T: hash::Hash, W: WitnessIn<T, Env>> hash::Hash for WitnessedInOwned<Env, T, W> {
        #[inline]
        fn hash<H: hash::Hasher>(&self, state: &mut H) {
            self.inner.hash(state)
        }
    }

    #[cfg(test)]
    mod basic_behavior_tests {
        use crate::contextual::test_support::{AnyInOwned, NormalizedOwned};

        use super::*;
        use std::{
            collections::hash_map::DefaultHasher,
            format,
            hash::{Hash, Hasher},
            sync::Arc,
        };

        fn hash64(x: impl Hash) -> u64 {
            let mut h = DefaultHasher::new();
            x.hash(&mut h);
            h.finish()
        }

        #[test]
        fn debug_fmt_preserves_inner_debug_repr_exactly() {
            let env: Arc<[f32]> = vec![0.2, 0.3, 0.5].into();
            let w = WitnessedInOwned::<Arc<[f32]>, f32, NormalizedOwned>::try_new_in(&env, 0.3)
                .unwrap();
            assert_eq!(format!("{:?}", w), "WitnessedInOwned(0.3)");
        }

        #[test]
        fn eq_ord_hash_delegate_to_inner_only() {
            let env1: Arc<[i32]> = vec![1, 2, 3].into();
            let env2: Arc<[i32]> = vec![9, 8, 7].into();

            let a = WitnessedInOwned::<Arc<[i32]>, i32, AnyInOwned>::try_new_in(&env1, 7).unwrap();
            let b = WitnessedInOwned::<Arc<[i32]>, i32, AnyInOwned>::try_new_in(&env2, 7).unwrap();
            let c = WitnessedInOwned::<Arc<[i32]>, i32, AnyInOwned>::try_new_in(&env1, 9).unwrap();

            assert_eq!(a, b);
            assert!(a < c);
            assert_eq!(hash64(&a), hash64(&b));
            assert_ne!(hash64(&a), hash64(&c));
        }
    }
}
