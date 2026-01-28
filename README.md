# witnessed

A small Rust pattern for carrying validated invariants through the type system.

## Idea

Define a witness `W` for some carrier type `T`. Construct `Witnessed<T, W>` only via the witness boundary (`W::witness` or the convenience `Witnessed::try_new`). Downstream code can then **require** `Witnessed<...>` in function signatures, preventing “unverified value” bugs across decoupled functions/modules.

A witness may also **normalize** input (e.g. trimming strings) while validating.

## Why not just a `newtype`?

A plain newtype wraps data, but it is still forgeable by downstream crates unless you keep constructors private. `Witnessed<T, W>` makes the “validated boundary” explicit and reusable: the proof lives in the type parameter `W`, so the same carrier `T` can be attested by different policies without proliferating wrapper types.

## Auto-traits (Send/Sync)

`Witnessed<T, W>` encodes `W` at the type level without owning it (`PhantomData<fn() -> W>`), so `Send`/`Sync` are driven by `T` rather than being accidentally constrained by `W`.

## `Witness<T>` API (verify required, attest optional)

A witness defines an invariant for a carrier `T`.

- `verify(&T) -> Result<(), Error>` is **required**: it checks the invariant on a borrowed value.
- `attest(T) -> Result<T, Error>` is **optional**: override it when you want to **normalize** (rewrite) input while validating.
    - The default `attest` typically calls `verify(&input)` and returns `Ok(input)`.

This split keeps “check-only” invariants lightweight while still supporting normalization when needed.

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

    // `attest` can be omitted: default is verify + identity.
}

fn pick(xs: &[i32; 3], idx: Witnessed<usize, IdxLt3>) -> i32 {
    xs[*idx]
}

#[derive(Debug, PartialEq, Eq)]
enum StrErr {
    Empty,
}

struct TrimNonEmpty;
impl Witness<String> for TrimNonEmpty {
    type Error = StrErr;

    fn verify(s: &String) -> Result<(), Self::Error> {
        (!s.trim().is_empty()).then_some(()).ok_or(StrErr::Empty)
    }

    // Override `attest` to normalize (trim) and validate the normalized value.
    fn attest(s: String) -> Result<String, Self::Error> {
        let s = s.trim().to_owned();
        (!s.is_empty()).then_some(s).ok_or(StrErr::Empty)
    }
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

    // === normalization demo ===
    let name = Witnessed::<String, TrimNonEmpty>::try_new("   hi   ".into()).unwrap();
    println!("normalized = {:?}", name.as_ref()); // "hi"
}
```

## `Warrant`: construct under a trusted rule (skip `attest`)

Sometimes you _derive_ a value from already-witnessed inputs, and the invariant is guaranteed by a known closure property (e.g. multiplying two `ZeroOne` values still yields `ZeroOne`). In such cases, you may want to avoid re-running `attest` (and any normalization / allocations it might perform).

This crate provides an `unsafe` trait:

- `Warrant<T, W>` authorizes constructing `Witnessed<T, W>` from a closure **without calling `W::attest`**.
- In **debug** builds, `W::verify(&out)` is still executed to catch violations early.
- In **release** builds, it is **zero overhead**.

> `unsafe` is on the _rule implementor_: they must ensure the produced value always satisfies the invariant.

Example (sketch):

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

## Zero-Cost Wrapper

`Witnessed<T, W>` is a **zero-cost wrapper** in terms of memory layout: it has the same size as `T` because `W` is only encoded in the type system via `PhantomData`, not stored at runtime.

However, validation/normalization has whatever cost your witness implements:

- `verify` costs whatever checks you perform.
- overriding `attest` may allocate/transform (e.g. trimming a `String`) and thus may cost more.

### Example: Memory Size Test

```rust
#[cfg(test)]
mod witness_size_tests {
    use witnessed::{Witness, Witnessed};
    use core::mem;

    struct Any;
    impl Witness<i32> for Any {
        type Error = core::convert::Infallible;
        fn verify(_: &i32) -> Result<(), Self::Error> { Ok(()) }
        // `attest` can be omitted (defaults to verify + identity).
    }

    #[test]
    fn witnessed_size_is_equal_to_inner_size() {
        let _ = Witnessed::<i32, Any>::try_new(42).unwrap();
        assert_eq!(mem::size_of::<Witnessed<i32, Any>>(), mem::size_of::<i32>());
    }
}
```

## `no_std`

This crate supports `#![no_std]`: the core API (`Witness`, `Witnessed`, `Warrant`) depends only on `core`. Unit tests use `std` via `#[cfg(test)] extern crate std;`.

Note: the crate does not require `alloc`, but your own witnesses may choose to validate/normalize `String`/`Vec` etc. (which requires an allocator on the consumer side).

## Pattern

- Put validation/normalization at boundaries (parsing, request decoding, DB reads).
- Accept `Witnessed<T, W>` in internal APIs that assume the invariant.
- Use `into_inner()` only when you explicitly want to drop the guarantee.
- Use `Warrant` when a **trusted derivation rule** preserves the invariant and you want to skip `attest` overhead.

## Compared to `refined_type`

`refined_type` models rules as _types_ and provides a rich set of composable rule combinators. This works very well when the refinement rule is known at compile time.

`witnessed` intentionally stays smaller and pushes composition into user code.

### Advantage: dynamic policies are natural

In many real systems, validation depends on runtime inputs (request flags, tenant config, feature gates, A/B experiments, environment, etc.). Encoding such “dynamic rule trees” in the type system can become awkward or impossible when the rule structure is only known at runtime.

With `witnessed`, you write the policy directly in `verify`/`attest` and keep the _result_ (the proof that it passed) in the type parameter `W`. The type-level guarantee stays simple even if the validation logic is dynamic.

### Trade-off: less uniformity and fewer built-in combinators

Because each `Witness` can define its own structured `Error`, there is no single, uniform error type that makes generic rule-combinator libraries trivial to build. If you want `AND/OR/NOT`-style composition with unified reporting, you typically implement it inside a witness (or add your own adapters) rather than relying on a standardized combinator stack.

## Why `Witness<T>` (generic) instead of `type Target` (associated type)?

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
    fn attest(input: Self::Target) -> Result<Self::Target, Self::Error>;
}
```

then a witness type like `NonEmpty` would be forced to pick exactly one `Target` forever:

```rust
struct NonEmpty;

impl Witness for NonEmpty {
    type Target = String;
    /* ... */
}

// You cannot also do:
// impl Witness for NonEmpty { type Target = Vec<u8>; ... }
// because `NonEmpty` already implemented `Witness` once.
```

For a general-purpose utility crate, this quickly becomes restrictive: users often want to apply the _same logical invariant_
(e.g. “non-empty”, “sorted”, “bounded”, “ASCII”, “normalized”) to different carriers (`String`, `Vec<T>`, maps, domain collections, etc.).
With associated types, they must introduce boilerplate witness types like `NonEmptyString`, `NonEmptyVec`, `NonEmptyMap`, purely to satisfy the uniqueness rule.

### The generic design: one witness can apply to many `T`

By making the target an explicit generic parameter:

```rust
pub trait Witness<T>: Sized {
    type Error;
    fn verify(input: &T) -> Result<(), Self::Error>;
    fn attest(input: T) -> Result<T, Self::Error>; // optional to override
}
```

the same witness type can be reused across multiple targets:

```rust
struct NonEmpty;

impl Witness<String> for NonEmpty { /* ... */ }
impl<T> Witness<Vec<T>> for NonEmpty { /* ... */ }
```

This is a better fit for a small “pattern” crate: it keeps the witness as a reusable _logic label_,
while `T` is the concrete carrier being validated/normalized.

### Why `Error` stays an associated type

While `T` benefits from polymorphism, `Error` benefits from determinism.
Once you pick _which witness_ (`W`) and _which carrier_ (`T`) you are validating,
the error type should be stable and precise. Keeping `type Error` as an associated type allows
structured, domain-friendly errors (enums with fields) without forcing a single global error model.

## Tests

```bash
cargo test
```
