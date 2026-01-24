# witnessed

A small Rust pattern for carrying validated invariants through the type system.

## Idea

Define a witness `W` for some type `T`. Construct `Witnessed<T, W>` only via `W::witness` (or the convenience `Witnessed::try_new`). Downstream code can then _require_ `Witnessed<...>` in function signatures, preventing “unchecked value” bugs across decoupled functions/modules.

A witness may also **normalize** input (e.g. trimming strings) while validating.

## Why not just `newtype`?

A plain newtype wraps data, but it is still forgeable by downstream crates unless you keep constructors private. `Witnessed<T, W>` makes the “validated boundary” explicit and reusable: the proof lives in the type parameter `W`.

## Auto-traits (Send/Sync)

`Witnessed<T, W>` encodes `W` at the type level without owning it (`PhantomData<fn() -> W>`), so `Send`/`Sync` are driven by `T` rather than being accidentally constrained by `W`.

## Example

```rust
use witnessed::{Witness, Witnessed};

struct IdxLt3;
#[derive(Debug, PartialEq, Eq)]
enum Err { OutOfRange }

impl Witness<usize> for IdxLt3 {
    type Error = Err;

    fn check(x: usize) -> Result<usize, Self::Error> {
        (x < 3).then_some(x).ok_or(Err::OutOfRange)
    }
}

// Consumer requires the invariant in its signature.
fn pick(xs: &[i32; 3], idx: Witnessed<usize, IdxLt3>) -> i32 { xs[*idx] }

// Boundary function: upgrade a plain value into a witnessed one.
fn parse_and_witness(s: &str) -> Result<Witnessed<usize, IdxLt3>, Err> {
    Witnessed::<usize, IdxLt3>::try_new(s.parse().unwrap())
    // or: IdxLt3::witness(s.parse().unwrap())
}
```

## Pattern

- Put validation/normalization at boundaries (parsing, request decoding, DB reads).
- Accept `Witnessed<T, W>` in internal APIs that assume the invariant.
- Use `into_inner()` only when you explicitly want to drop the guarantee.

## Tests

```bash
cargo test
```
