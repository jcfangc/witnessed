# witnessed

A small Rust pattern for carrying validated invariants through the type system.

## Idea

Define a witness `W` for some carrier type `T`. Construct `Witnessed<T, W>` only via `W::witness` (or the convenience `Witnessed::try_new`). Downstream code can then _require_ `Witnessed<...>` in function signatures, preventing “unattested value” bugs across decoupled functions/modules.

A witness may also **normalize** input (e.g. trimming strings) while validating.

## Why not just a `newtype`?

A plain newtype wraps data, but it is still forgeable by downstream crates unless you keep constructors private. `Witnessed<T, W>` makes the “validated boundary” explicit and reusable: the proof lives in the type parameter `W`, so the same carrier `T` can be attested by different policies without proliferating wrapper types.

## Auto-traits (Send/Sync)

`Witnessed<T, W>` encodes `W` at the type level without owning it (`PhantomData<fn() -> W>`), so `Send`/`Sync` are driven by `T` rather than being accidentally constrained by `W`.

## Example

```rust
use witnessed::{Witness, Witnessed};

#[derive(Debug, PartialEq, Eq)]
enum IdxErr {
    OutOfRange { idx: usize },
}

struct IdxLt3;
impl Witness<usize> for IdxLt3 {
    type Error = IdxErr;
    fn attest(x: usize) -> Result<usize, Self::Error> {
        (x < 3).then_some(x).ok_or(IdxErr::OutOfRange { idx: x })
    }
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

## Pattern

- Put validation/normalization at boundaries (parsing, request decoding, DB reads).
- Accept `Witnessed<T, W>` in internal APIs that assume the invariant.
- Use `into_inner()` only when you explicitly want to drop the guarantee.

## Compared to `refined_type`

`refined_type` models rules as _types_ and provides a rich set of composable rule combinators. This works very well when the refinement rule is known at compile time.

`witnessed` intentionally stays smaller and pushes composition into user code.

### Advantage: dynamic policies are natural

In many real systems, validation depends on runtime inputs (request flags, tenant config, feature gates, A/B experiments, environment, etc.). Encoding such “dynamic rule trees” in the type system can become awkward or impossible when the rule structure is only known at runtime.

With `witnessed`, you write the policy directly in `W::attest` and keep the _result_ (the proof that it passed) in the type parameter `W`. The type-level guarantee stays simple even if the validation logic is dynamic.

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
    fn attest(input: T) -> Result<T, Self::Error>;
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

### What we give up by not using `type Target`

Using an associated `Target` can produce simpler type names in some designs,
especially when the witness type _is itself_ a domain object (e.g. `Email`, `UserId`, `OrderId`)
where you never intend to witness any other carrier:

```rust
struct Email; // semantically bound to `String` forever
```

In that strongly domain-coupled style, `type Target` can read nicely and prevent misuse by construction.

`witnessed` chooses the opposite end: it optimizes for **reusable invariants** rather than **one-off domain wrappers**.
If you want a domain-specific refined type, you can still build it on top of `Witness<T>`
by defining a dedicated witness type per domain concept.

### Why `Error` stays an associated type

While `T` benefits from polymorphism, `Error` benefits from determinism.
Once you pick _which witness_ (`W`) and _which carrier_ (`T`) you are validating,
the error type should be stable and precise. Keeping `type Error` as an associated type allows
structured, domain-friendly errors (enums with fields) without forcing a single global error model.

## Tests

```bash
cargo test
```
