# witnessed

A small Rust pattern for carrying validated invariants through the type system.

## Idea

Define a witness `W` for some type `T`. Construct `Witnessed<T, W>` only via `W::witness`, so downstream code can require `Witnessed<...>` and avoid “unchecked value” bugs across decoupled functions/modules.

## Example

```rust
use witness::{Witness, Witnessed};

struct IdxLt3;
#[derive(Debug, PartialEq, Eq)]
enum Err { OutOfRange }

impl Witness<usize> for IdxLt3 {
    type Error = Err;
    fn witness(x: usize) -> Result<Witnessed<usize, Self>, Self::Error> {
        (x < 3).then(|| Witnessed::new_unchecked(x)).ok_or(Err::OutOfRange)
    }
}

fn pick(xs: &[i32; 3], idx: Witnessed<usize, IdxLt3>) -> i32 { xs[*idx] }
```

## Tests

```bash
cargo test
```
