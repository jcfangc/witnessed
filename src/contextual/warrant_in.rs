use crate::contextual::{WitnessIn, WitnessedInOwned, WitnessedInRef};

/// A marker trait authorizing construction of contextual witnessed values
/// without running `W::verify_in(env, ..)` in release builds.
///
/// This trait represents a *trusted derivation rule* for contextual witnesses.
/// It is analogous to a proof rule in a logic system: if the premises are known
/// to satisfy the invariant under a given environment, the conclusion may be
/// constructed without rechecking the invariant.
///
/// The environment is used only at the construction boundary and is not stored
/// at runtime by either `WitnessedInRef` or `WitnessedInOwned`.
///
/// # Safety
///
/// Implementors must guarantee that values produced under this warrant satisfy
/// `W(Env, T)` with respect to the same environment instance `env` used at
/// construction.
///
/// If the rule combines multiple contextual witnessed inputs, those inputs must
/// have been validated relative to that same environment.
///
/// In other words, the rule must be *closed under the same environment instance*.
///
/// # Debug checking
///
/// In debug builds, warrant constructors perform a defensive `W::verify_in(env, &out)`
/// and panic if the warrant is violated. In release builds this check is skipped,
/// so the rule must be correct.
pub unsafe trait WarrantIn<T, Env: ?Sized, W: WitnessIn<T, Env>> {
    /// Construct a borrowed contextual witness under a trusted derivation rule.
    ///
    /// The returned witness is type-level bound to the lifetime of `env`, but
    /// the environment reference itself is not stored at runtime.
    ///
    /// Suggested call-site pattern:
    ///
    /// ```ignore
    /// // Warrant: under this env, combining two normalized values preserves the invariant.
    /// // `a` and `b` were validated under the same env.
    /// let x =
    ///     <Rule as WarrantIn<f32, EnvTy, NormW>>::warrant_in_ref(env, || combine(*a, *b));
    /// ```
    #[inline]
    fn warrant_in_ref<'a>(env: &'a Env, f: impl FnOnce() -> T) -> WitnessedInRef<'a, Env, T, W> {
        let out = f();
        #[cfg(debug_assertions)]
        {
            if W::verify_in(env, &out).is_err() {
                panic!("warrant violated witness");
            }
        }
        WitnessedInRef::new_unchecked(out)
    }

    /// Construct an owned-form contextual witness under a trusted derivation rule.
    ///
    /// The returned witness is parameterized by `Env` but does not carry a borrow
    /// lifetime. This is useful when `Env` is itself an owned handle type such as
    /// `Arc<_>`.
    ///
    /// Suggested call-site pattern:
    ///
    /// ```ignore
    /// // Warrant: under this Arc-backed env, combining two values preserves the invariant.
    /// let x =
    ///     <Rule as WarrantIn<f32, ArcEnvTy, NormW>>::warrant_in_owned(&env, || combine(*a, *b));
    /// ```
    #[inline]
    fn warrant_in_owned(env: &Env, f: impl FnOnce() -> T) -> WitnessedInOwned<Env, T, W>
    where
        Env: Sized,
    {
        let out = f();
        #[cfg(debug_assertions)]
        {
            if W::verify_in(env, &out).is_err() {
                panic!("warrant violated witness");
            }
        }
        WitnessedInOwned::new_unchecked(out)
    }
}

#[cfg(test)]
mod warrant_in_tests {
    use crate::contextual::WarrantIn;
    use crate::contextual::test_support::{NormErr, NormalizedRef};
    use crate::contextual::{WitnessIn, WitnessedInRef};
    use core::sync::atomic::{AtomicUsize, Ordering};

    // A warrant rule claiming: if a,b are members of the same normalized env, then a+b is also
    // a member (this is NOT generally true). We'll use it with a constructed env where it holds.
    //
    // Safety rationale (restricted to our test env): env = [0.2, 0.3, 0.5], a=0.2, b=0.3 => a+b=0.5 in env.
    struct AddMember01;
    unsafe impl WarrantIn<f32, [f32], NormalizedRef> for AddMember01 {}

    #[inline]
    fn n<'a>(env: &'a [f32], x: f32) -> WitnessedInRef<'a, [f32], f32, NormalizedRef> {
        WitnessedInRef::<'a, [f32], f32, NormalizedRef>::try_new_in(env, x).unwrap()
    }

    #[test]
    fn normalized_add_member_is_normalized_member_under_same_env() {
        let env_vec = vec![0.2, 0.3, 0.5];
        let env = env_vec.as_slice();

        let a = n(env, 0.2);
        let b = n(env, 0.3);

        let c = AddMember01::warrant_in_ref(env, || *a + *b);

        assert!((*c - 0.5).abs() < 1e-6);
        assert!(NormalizedRef::verify_in(env, c.as_ref()).is_ok());
    }

    // in debug: warrant_in will verify_in, and panic if violated
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "warrant violated witness")]
    fn warrant_in_panics_if_rule_violated_in_debug() {
        let env_vec = vec![0.2, 0.3, 0.5];
        let env = env_vec.as_slice();

        // 0.2 + 0.5 = 0.7, which is not in env -> should panic in debug
        let a = n(env, 0.2);
        let b = n(env, 0.5);
        let _ = AddMember01::warrant_in_ref(env, || *a + *b);
    }

    // prove warrant_in does not do any extra checks in release, and only one verify_in in debug
    static VERIFY_IN: AtomicUsize = AtomicUsize::new(0);

    struct CountedNorm;
    impl WitnessIn<f32, [f32]> for CountedNorm {
        type Error = NormErr;

        fn verify_in(env: &[f32], x: &f32) -> Result<(), Self::Error> {
            VERIFY_IN.fetch_add(1, Ordering::Relaxed);
            NormalizedRef::verify_in(env, x)
        }
    }

    struct AddMemberCounted;
    unsafe impl WarrantIn<f32, [f32], CountedNorm> for AddMemberCounted {}

    #[inline]
    fn nc<'a>(env: &'a [f32], x: f32) -> WitnessedInRef<'a, [f32], f32, CountedNorm> {
        WitnessedInRef::<'a, [f32], f32, CountedNorm>::try_new_in(env, x).unwrap()
    }

    #[test]
    fn warrant_in_only_verifies_in_debug() {
        let env_vec = vec![0.2, 0.3, 0.5];
        let env = env_vec.as_slice();

        VERIFY_IN.store(0, Ordering::Relaxed);

        let a = nc(env, 0.2); // verify_in += 1
        let b = nc(env, 0.3); // verify_in += 1
        assert_eq!(VERIFY_IN.load(Ordering::Relaxed), 2);

        let _c = AddMemberCounted::warrant_in_ref(env, || *a + *b);

        #[cfg(debug_assertions)]
        assert_eq!(VERIFY_IN.load(Ordering::Relaxed), 3); // warrant_in verify_in += 1
        #[cfg(not(debug_assertions))]
        assert_eq!(VERIFY_IN.load(Ordering::Relaxed), 2); // no extra verify_in
    }
}

#[cfg(test)]
mod warrant_in_ref_owned_bridge_tests {
    use crate::contextual::test_support::{NormalizedOwned, NormalizedRef};
    use crate::contextual::{WarrantIn, WitnessIn, WitnessedInOwned, WitnessedInRef};
    use std::sync::Arc;

    // Same logical rule, specialized once for borrowed env and once for Arc-backed env.
    struct AddMemberBridge;

    unsafe impl WarrantIn<f32, [f32], NormalizedRef> for AddMemberBridge {}
    unsafe impl WarrantIn<f32, Arc<[f32]>, NormalizedOwned> for AddMemberBridge {}

    #[inline]
    fn nr<'a>(env: &'a [f32], x: f32) -> WitnessedInRef<'a, [f32], f32, NormalizedRef> {
        WitnessedInRef::<'a, [f32], f32, NormalizedRef>::try_new_in(env, x).unwrap()
    }

    #[inline]
    fn no(env: &Arc<[f32]>, x: f32) -> WitnessedInOwned<Arc<[f32]>, f32, NormalizedOwned> {
        WitnessedInOwned::<Arc<[f32]>, f32, NormalizedOwned>::try_new_in(env, x).unwrap()
    }

    #[test]
    fn ref_and_owned_warrants_agree_on_same_logical_env() {
        let env_vec = vec![0.2, 0.3, 0.5];
        let env_ref = env_vec.as_slice();
        let env_owned: Arc<[f32]> = env_vec.clone().into();

        let a_ref = nr(env_ref, 0.2);
        let b_ref = nr(env_ref, 0.3);
        let c_ref = AddMemberBridge::warrant_in_ref(env_ref, || *a_ref + *b_ref);

        let a_owned = no(&env_owned, 0.2);
        let b_owned = no(&env_owned, 0.3);
        let c_owned = AddMemberBridge::warrant_in_owned(&env_owned, || *a_owned + *b_owned);

        assert!((*c_ref - 0.5).abs() < 1e-6);
        assert!((*c_owned - 0.5).abs() < 1e-6);

        assert_eq!(*c_ref, *c_owned);
        assert!(NormalizedRef::verify_in(env_ref, c_ref.as_ref()).is_ok());
        assert!(NormalizedOwned::verify_in(&env_owned, c_owned.as_ref()).is_ok());
    }

    #[test]
    fn ref_and_owned_direct_construction_agree_on_same_value() {
        let env_vec = vec![0.2, 0.3, 0.5];
        let env_ref = env_vec.as_slice();
        let env_owned: Arc<[f32]> = env_vec.clone().into();

        let x_ref =
            WitnessedInRef::<'_, [f32], f32, NormalizedRef>::try_new_in(env_ref, 0.3).unwrap();

        let x_owned =
            WitnessedInOwned::<Arc<[f32]>, f32, NormalizedOwned>::try_new_in(&env_owned, 0.3)
                .unwrap();

        assert_eq!(*x_ref, *x_owned);
        assert!(NormalizedRef::verify_in(env_ref, x_ref.as_ref()).is_ok());
        assert!(NormalizedOwned::verify_in(&env_owned, x_owned.as_ref()).is_ok());
    }

    // In debug builds, both ref and owned warrant paths should panic on the same invalid derivation.
    #[cfg(debug_assertions)]
    #[test]
    fn ref_and_owned_warrant_failures_are_consistent_in_debug() {
        let env_vec = vec![0.2, 0.3, 0.5];
        let env_ref = env_vec.as_slice();
        let env_owned: Arc<[f32]> = env_vec.clone().into();

        let a_ref = nr(env_ref, 0.2);
        let b_ref = nr(env_ref, 0.5);

        let a_owned = no(&env_owned, 0.2);
        let b_owned = no(&env_owned, 0.5);

        let ref_panicked = std::panic::catch_unwind(|| {
            let _ = AddMemberBridge::warrant_in_ref(env_ref, || *a_ref + *b_ref);
        })
        .is_err();

        let owned_panicked = std::panic::catch_unwind(|| {
            let _ = AddMemberBridge::warrant_in_owned(&env_owned, || *a_owned + *b_owned);
        })
        .is_err();

        assert!(ref_panicked);
        assert!(owned_panicked);
    }
}
