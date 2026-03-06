# witnessed

A small Rust pattern for carrying validated invariants through the type system.

## Idea

Define a witness `W` for some carrier type `T`. Construct `Witnessed<T, W>` only via the witness boundary (`W::witness` or `Witnessed::try_new`). Downstream code can then **require** `Witnessed<...>` in function signatures, preventing “unverified value” bugs across decoupled functions/modules.

This crate intentionally models witnesses as **pure checks**: a witness verifies facts; it does not rewrite values.

## Why not just a `newtype`?

A plain newtype wraps data, but it is still forgeable by downstream crates unless you keep constructors private. `Witnessed<T, W>` makes the “validated boundary” explicit and reusable: the proof lives in the type parameter `W`, so the same carrier `T` can be attested by different policies without proliferating wrapper types.

## Auto-traits (Send/Sync)

`Witnessed<T, W>` encodes `W` purely at the type level (`PhantomData<fn() -> W>`), so auto-traits such as `Send`/`Sync` are driven by `T` rather than being accidentally constrained by `W`.

For the contextual wrapper `WitnessedIn<'a, Env, T, W>`, the environment is **not stored at runtime**. The dependency on `Env` only appears in the type system via `PhantomData<fn(&'a Env) -> W>`. As a result:

- the runtime representation contains only `T`
- auto-traits are determined by `T`
- the lifetime `'a` ensures the witness cannot outlive the environment used for validation

---

# Intrinsic (environment-free) API

## `Witness<T>` API (verify only)

A witness defines an invariant for a carrier `T`.

- `verify(&T) -> Result<(), Error>` checks the invariant on a borrowed value.
- `witness(T) -> Result<Witnessed<T, Self>, Error>` constructs a witnessed value via the crate-controlled boundary.

```rust
pub trait Witness<T>: Sized {
    type Error;
    fn verify(input: &T) -> Result<(), Self::Error>;

    #[inline]
    fn witness(input: T) -> Result<Witnessed<T, Self>, Self::Error> {
        Witnessed::<T, Self>::try_new(input)
    }
}
```

## Example: General Usage

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

fn main() {
    let xs = [10, 20, 30];

    // === boundary: parse/compute -> witness ===
    let raw = "2".parse::<usize>().unwrap();
    let computed = raw + 2; // 4, would panic if used directly as index

    match Witnessed::<usize, IdxLt3>::try_new(computed) {
        Ok(idx) => println!("picked = {}", pick(&xs, idx)),
        Err(e) => println!("index rejected: {:?}", e),
    }
}
```

---

# Contextual (environment-dependent) API

Note: the environment reference is used only during validation.

The resulting `WitnessedIn` value **does not store the environment at runtime**. Instead, the type system tracks that the witness is tied to the lifetime of the environment used for validation.

Some invariants are not intrinsic properties of `T`, but relations that only make sense **relative to some environment**:

- stateful/parameterized checks (thresholds, indices, precomputed stats) provided by `Env`
- invariants whose meaning depends on external context (e.g. “normalized w.r.t. dataset D”)

This crate models that with:

- `WitnessIn<T, Env>`: a witness for the relation `W(Env, T)`
- `WitnessedIn<'a, Env, T, W>`: a value of type `T` witnessed under a concrete `&'a Env`

## `WitnessIn<T, Env>` API

```rust
use witnessed::contextual::WitnessedIn;

pub trait WitnessIn<T, Env: ?Sized>: Sized {
    type Error;

    fn verify_in(env: &Env, input: &T) -> Result<(), Self::Error>;

    #[inline]
    fn witness_in<'a>(
        env: &'a Env,
        input: T,
    ) -> Result<WitnessedIn<'a, Env, T, Self>, Self::Error> {
        WitnessedIn::<'a, Env, T, Self>::try_new_in(env, input)
    }
}
```

## Example: `Normalized` relative to a container

Below, `Normalized` is meaningful only with respect to an environment `env: &[f32]`.

Invariant (relative to `env`):

- `env` is non-empty
- `env` is finite
- `sum(env) == 1` (within epsilon)
- `x` is finite
- `x` is a member of `env` (exact equality)

```rust
use witnessed::contextual::{WitnessIn, WitnessedIn};

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
    let x = WitnessedIn::<[f32], f32, Normalized>::try_new_in(env.as_slice(), 0.3).unwrap();

    assert_eq!(*x, 0.3);
    assert!(Normalized::verify_in(env.as_slice(), x.as_ref()).is_ok());
}
```

---

# `Warrant`: construct under a trusted rule (skip verification in release)

Sometimes you _derive_ a value from already-witnessed inputs, and the invariant is guaranteed by a known closure property. In such cases, you may want to avoid re-running verification.

This crate provides `unsafe` marker traits:

- `Warrant<T, W>` authorizes constructing `Witnessed<T, W>` from a closure.
- `WarrantIn<T, Env, W>` authorizes constructing `WitnessedIn<'a, Env, T, W>` from a closure under a concrete `env`.

Behavior:

- In **debug** builds, the witness check is still executed to catch violations early.
- In **release** builds, construction is **zero overhead**.

> `unsafe` is on the _rule implementor_: they must ensure the produced value always satisfies the invariant (intrinsically, or relative to `env`).

## `Warrant` (intrinsic) example

```rust
use witnessed::{Witness, Witnessed, Warrant};

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
    // Warrant: if a,b∈[0,1] then a*b∈[0,1].
    <Mul01 as Warrant<f32, ZeroOne>>::warrant(|| *a * *b)
}
```

## `WarrantIn` (contextual) example

```rust
use witnessed::contextual::{WitnessIn, WitnessedIn, WarrantIn};

struct Normalized;
#[derive(Debug, PartialEq)]
enum NormErr { Bad }

impl WitnessIn<f32, [f32]> for Normalized {
    type Error = NormErr;
    fn verify_in(_env: &[f32], _x: &f32) -> Result<(), Self::Error> { Ok(()) }
}

// A warrant rule claiming: under the same env, combining two values preserves the invariant.
// (In real code, you must justify this rule carefully.)
struct SomeRule;
unsafe impl WarrantIn<f32, [f32], Normalized> for SomeRule {}

fn derive<'a>(
    env: &'a [f32],
    a: WitnessedIn<'a, [f32], f32, Normalized>,
    b: WitnessedIn<'a, [f32], f32, Normalized>,
) -> WitnessedIn<'a, [f32], f32, Normalized> {
    // Warrant: trusted derivation under the same env.
    <SomeRule as WarrantIn<f32, [f32], Normalized>>::warrant_in(env, || *a + *b)
}
```

---

# Wrapper behavior

## Zero-cost (intrinsic)

`Witnessed<T, W>` is a **zero-cost wrapper** in terms of memory layout: it has the same size as `T` because `W` is only encoded in the type system via `PhantomData`, not stored at runtime.

## Contextual representation

`WitnessedIn<'a, Env, T, W>` stores only the runtime value `T`.

The dependency on the environment is tracked purely at the type level via `PhantomData<fn(&'a Env) -> W>`. The environment reference is **not stored**.

This design ensures:

- the witness cannot outlive the environment used for validation
- no environment pointer needs to be carried at runtime
- the runtime layout remains minimal

`WitnessedIn` is therefore `repr(transparent)` over `T`.

---

# `no_std`

This crate supports `#![no_std]`: the core API depends only on `core`. Unit tests use `std` via `#[cfg(test)] extern crate std;`.

Note: the crate does not require `alloc`, but your own witnesses may choose to validate `String`/`Vec` etc. (which requires an allocator on the consumer side).

---

# Pattern

- Put validation at boundaries (parsing, request decoding, DB reads).
- Accept `Witnessed<T, W>` or `WitnessedIn<'a, Env, T, W>` in internal APIs that assume the invariant.
- Use `into_inner()` only when you explicitly want to drop the guarantee.
- Use `Warrant` / `WarrantIn` when a **trusted derivation rule** preserves the invariant and you want to skip checks in release.

When using contextual witnesses, ensure that derived values are constructed relative to the same environment instance that validated their inputs.

---

# Compared to `refined_type`

`refined_type` models rules as _types_ and provides a rich set of composable rule combinators. This works very well when the refinement rule is known at compile time.

`witnessed` intentionally stays smaller and pushes composition into user code.

### Advantage: dynamic and contextual policies are natural

In many real systems, validation depends on runtime inputs (request flags, tenant config, feature gates, A/B experiments, environment, etc.). Encoding such “dynamic rule trees” in the type system can become awkward or impossible when the rule structure is only known at runtime.

With `witnessed`, you write the policy directly in `verify` / `verify_in` and keep the _result_ (the proof that it passed) in the type parameter `W`. The type-level guarantee stays simple even if the validation logic is dynamic or environment-dependent.

### Trade-off: fewer built-in combinators

Because each witness can define its own structured `Error`, there is no single, uniform error type that makes generic rule-combinator libraries trivial to build. If you want `AND/OR/NOT`-style composition with unified reporting, you typically implement it inside a witness (or add your own adapters) rather than relying on a standardized combinator stack.

---

# Why `Witness<T>` (generic) instead of `type Target` (associated type)?

This crate makes an explicit choice in the Rust type-system trade-off space:
**polymorphism (one witness, many target types)** versus **uniqueness (one witness, one target type)**.

### The core constraint: associated types must be unique per implementor

In Rust, once a type `W` implements a trait with an associated type, that associated type is fixed for `W`.
You cannot implement the same trait again for the same `W` with a different associated type.

If `Witness` were defined like this:

```rust
trait Witness {
    type Target;
    type Error;
    fn witness(input: Self::Target) -> Result<_, Self::Error>;
}
```

then a witness type like `NonEmpty` would be forced to pick exactly one `Target` forever.

### The generic design: one witness can apply to many `T`

By making the target an explicit generic parameter:

```rust
pub trait Witness<T>: Sized {
    type Error;
    fn verify(input: &T) -> Result<(), Self::Error>;
    fn witness(input: T) -> Result<Witnessed<T, Self>, Self::Error>;
}
```

the same witness type can be reused across multiple targets:

```rust
struct NonEmpty;

impl Witness<String> for NonEmpty { /* ... */ }
impl<T> Witness<Vec<T>> for NonEmpty { /* ... */ }
```

### Why `Error` stays an associated type

While `T` benefits from polymorphism, `Error` benefits from determinism.
Once you pick _which witness_ (`W`) and _which carrier_ (`T`) you are validating,
the error type should be stable and precise.

---

# Tests

```bash
cargo test
```
