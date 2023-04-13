use crate::arithmetic::register2::memory::MemorySlice;
use crate::arithmetic::register2::register::{Register, RegisterType};

/// A register for a single element/column in the trace that is supposed to represent a bit. The
/// value is automatically constrained to be 0 or 1 via the quadratic constraint x * (x - 1) == 0.
#[derive(Debug, Clone, Copy)]
pub struct BitRegister(MemorySlice);

impl Register for BitRegister {
    const CELL: Option<RegisterType> = Some(RegisterType::Bit);

    fn from_raw_register(register: MemorySlice) -> Self {
        Self(register)
    }

    fn register(&self) -> &MemorySlice {
        &self.0
    }

    fn size_of() -> usize {
        1
    }

    fn into_raw_register(self) -> MemorySlice {
        self.0
    }
}
