use std::marker::PhantomData;

use super::cell::CellType;
use super::{Register, RegisterSerializable, RegisterSized};
use crate::arithmetic::field::FieldParameters;
use crate::arithmetic::register::memory::MemorySlice;

/// A register for representing a field element. The value is decomposed into a series of U16 limbs
/// which is controlled by `NB_LIMBS` in FieldParameters. Each limb is range checked using a lookup.
#[derive(Debug, Clone, Copy)]
pub struct FieldRegister<P: FieldParameters> {
    register: MemorySlice,
    _marker: PhantomData<P>,
}

impl<P: FieldParameters> RegisterSerializable for FieldRegister<P> {
    const CELL: Option<CellType> = Some(CellType::U16);

    fn register(&self) -> &MemorySlice {
        &self.register
    }

    fn from_register_unsafe(register: MemorySlice) -> Self {
        Self {
            register,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<P: FieldParameters> RegisterSized for FieldRegister<P> {
    fn size_of() -> usize {
        P::NB_LIMBS
    }
}

impl<P: FieldParameters> Register for FieldRegister<P> {}
