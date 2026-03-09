# witnessed

A small Rust pattern for carrying validated invariants through the type system.

`witnessed` separates:

- **validation at boundaries**
- **trusted use inside the program**

A value becomes `Witnessed<..., W>` only after passing witness `W`.  
This helps prevent “unchecked value slipped into internal logic” bugs across decoupled modules.

This crate models witnesses as **pure checks**:

- a witness verifies facts
- it does not rewrite values

---

## Core idea

Define a witness `W` for a carrier type `T`.

Construct:

- `Witnessed<T, W>` for intrinsic invariants
- `WitnessedInRef<'a, Env, T, W>` for contextual invariants tied to `&'a Env`
- `WitnessedInOwned<Env, T, W>` for contextual invariants validated against `&Env` without carrying a borrow lifetime

Internal APIs can then require witnessed values directly.

---

## Intrinsic API

Use `Witness<T>` for invariants that depend only on `T`.

```rust
use witnessed::{Witness, Witnessed};

#[derive(Debug, PartialEq, Eq)]
enum IdxErr {
    OutOfRange { idx: usize },
}

struct IdxLt3;

impl Witness<usize> for IdxLt3 {
    type Error = IdxErr;

    fn verify(x: &usize) -> Result<(), Self::Error> {
        (*x < 3).then_some(()).ok_or(IdxErr::OutOfRange { idx: *x })
    }
}

fn pick(xs: &[i32; 3], idx: Witnessed<usize, IdxLt3>) -> i32 {
    xs[*idx]
}
```

---

## Contextual API

Some invariants only make sense relative to an environment.

Examples:

- thresholds from config
- indices checked against a specific container
- values validated relative to precomputed stats
- facts defined with respect to a shared context

`witnessed` provides:

- `WitnessIn<T, Env>`
- `WitnessedInRef<'a, Env, T, W>`
- `WitnessedInOwned<Env, T, W>`

### Borrowed contextual witness

Use `WitnessedInRef` when the proof should be tied to the lifetime of a concrete borrowed environment.

```rust
use witnessed::contextual::{WitnessIn, WitnessedInRef};

struct Normalized;

#[derive(Debug, PartialEq)]
enum NormErr {
    EnvEmpty,
    EnvNonFinite,
    EnvSumNotOne { sum: f32 },
    ValueNonFinite,
    NotMember { x: f32 },
}

impl WitnessIn<f32, [f32]> for Normalized {
    type Error = NormErr;

    fn verify_in(env: &[f32], x: &f32) -> Result<(), Self::Error> {
        if env.is_empty() {
            return Err(NormErr::EnvEmpty);
        }
        if !x.is_finite() {
            return Err(NormErr::ValueNonFinite);
        }
        if !env.iter().all(|v| v.is_finite()) {
            return Err(NormErr::EnvNonFinite);
        }

        let sum = env.iter().copied().sum::<f32>();
        if (sum - 1.0).abs() > 1e-6 {
            return Err(NormErr::EnvSumNotOne { sum });
        }

        env.iter()
            .any(|v| v == x)
            .then_some(())
            .ok_or(NormErr::NotMember { x: *x })
    }
}

fn main() {
    let env = vec![0.2, 0.3, 0.5];
    let x = WitnessedInRef::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.3).unwrap();

    assert_eq!(*x, 0.3);
}
```

### Owned contextual witness

Use `WitnessedInOwned` when validation depends on an environment type such as `Arc<[f32]>`, but the witnessed value should remain a transparent wrapper over `T` and should not carry a borrow lifetime.

```rust
use std::sync::Arc;
use witnessed::contextual::{WitnessIn, WitnessedInOwned};

struct Normalized;

#[derive(Debug, PartialEq)]
enum NormErr {
    EnvEmpty,
    EnvNonFinite,
    EnvSumNotOne { sum: f32 },
    ValueNonFinite,
    NotMember { x: f32 },
}

impl WitnessIn<f32, Arc<[f32]>> for Normalized {
    type Error = NormErr;

    fn verify_in(env: &Arc<[f32]>, x: &f32) -> Result<(), Self::Error> {
        let env = env.as_ref();

        if env.is_empty() {
            return Err(NormErr::EnvEmpty);
        }
        if !x.is_finite() {
            return Err(NormErr::ValueNonFinite);
        }
        if !env.iter().all(|v| v.is_finite()) {
            return Err(NormErr::EnvNonFinite);
        }

        let sum = env.iter().copied().sum::<f32>();
        if (sum - 1.0).abs() > 1e-6 {
            return Err(NormErr::EnvSumNotOne { sum });
        }

        env.iter()
            .any(|v| v == x)
            .then_some(())
            .ok_or(NormErr::NotMember { x: *x })
    }
}

fn main() {
    let env: Arc<[f32]> = vec![0.2, 0.3, 0.5].into();
    let x = WitnessedInOwned::<Arc<[f32]>, f32, Normalized>::try_new_in(&env, 0.3).unwrap();

    assert_eq!(*x, 0.3);
}
```

---

## Which contextual wrapper should I use?

- Use `WitnessedInRef` when the proof must be tied to a borrowed environment lifetime.
- Use `WitnessedInOwned` when the environment is represented by an owned handle type such as `Arc<_>`, and you do not want to thread a borrow lifetime through your APIs.

In both cases, the environment is used only during validation and is **not stored at runtime**.

---

## Warrant

Sometimes a value is derived from already-witnessed inputs under a trusted closure rule.

For those cases, the crate provides:

- `Warrant<T, W>`
- `WarrantIn<T, Env, W>`

Behavior:

- in **debug** builds, validation still runs
- in **release** builds, construction can skip re-checking

This is useful when a known closure property preserves the invariant and you want zero overhead in optimized builds.

```rust
use witnessed::{Warrant, Witness, Witnessed};

struct ZeroOne;
#[derive(Debug, PartialEq)]
enum ZErr { OutOfRange(f32) }

impl Witness<f32> for ZeroOne {
    type Error = ZErr;

    fn verify(x: &f32) -> Result<(), Self::Error> {
        (0.0 <= *x && *x <= 1.0).then_some(()).ok_or(ZErr::OutOfRange(*x))
    }
}

struct Mul01;
unsafe impl Warrant<f32, ZeroOne> for Mul01 {}

fn mul01(a: Witnessed<f32, ZeroOne>, b: Witnessed<f32, ZeroOne>) -> Witnessed<f32, ZeroOne> {
    <Mul01 as Warrant<f32, ZeroOne>>::warrant(|| *a * *b)
}
```

---

## Representation

- `Witnessed<T, W>` is `repr(transparent)` over `T`
- `WitnessedInRef<'a, Env, T, W>` is `repr(transparent)` over `T`
- `WitnessedInOwned<Env, T, W>` is `repr(transparent)` over `T`

Witness types and contextual environment types participate only in the type system; they are not stored at runtime.

---

## `no_std`

This crate supports `#![no_std]`.

The core API depends only on `core`.
Tests use `std` under `#[cfg(test)]`.

---

## Why generic `Witness<T>` instead of `type Target`?

Because a single witness type can naturally apply to multiple carriers:

```rust
struct NonEmpty;

impl Witness<String> for NonEmpty { /* ... */ }
impl<T> Witness<Vec<T>> for NonEmpty { /* ... */ }
```

Using a generic parameter for `T` preserves that flexibility.

---

## Pattern

Typical usage:

- validate at boundaries
- require witnessed values in internal APIs
- use `into_inner()` only when intentionally dropping the guarantee
- use `Warrant` / `WarrantIn` for trusted derivations
