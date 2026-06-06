use crate::Witnessing;

/// Extension trait for starting a witnessing pipeline.
pub trait WitnessExt: Sized {
    #[inline]
    fn witness(self) -> Witnessing<Self> {
        Witnessing { value: self }
    }
}

impl<T> WitnessExt for T {}
