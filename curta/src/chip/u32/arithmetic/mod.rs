use crate::air::parser::AirParser;
use crate::air::AirConstraint;
use crate::chip::instruction::Instruction;
use crate::chip::register::memory::MemorySlice;
use crate::chip::trace::writer::TraceWriter;
pub use crate::math::prelude::*;

#[derive(Debug, Clone)]
pub enum U32ArithmericOperation {
    Add,
}

impl<AP: AirParser> AirConstraint<AP> for U32ArithmericOperation {
    fn eval(&self, _parser: &mut AP) {
        todo!("U32ArithmericOperation::eval")
    }
}

impl<F: Field> Instruction<F> for U32ArithmericOperation {
    fn inputs(&self) -> Vec<MemorySlice> {
        todo!("U32ArithmericOperation::inputs")
    }

    fn trace_layout(&self) -> Vec<MemorySlice> {
        todo!("U32ArithmericOperation::trace_layout")
    }

    fn write(&self, _writer: &TraceWriter<F>, _row_index: usize) {
        todo!("U32ArithmericOperation::write")
    }
}
