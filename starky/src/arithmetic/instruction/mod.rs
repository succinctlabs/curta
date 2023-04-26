//! Instruction trait
//!
//! The instruction trait represents the interface of a microcomand in a chip. It is the
//! lowest level of abstraction in the arithmetic module.

pub mod empty;
mod set;
pub mod write;

pub use empty::EmptyInstructionSet;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
pub use set::{FromInstructionSet, InstructionSet};

use super::field::{
    FpAddInstruction, FpInnerProductInstruction, FpMulConstInstruction, FpMulInstruction,
};
use super::register::MemorySlice;
use crate::vars::{StarkEvaluationTargets, StarkEvaluationVars};

pub trait Instruction<F: RichField + Extendable<D>, const D: usize>:
    'static + Send + Sync + Clone
{
    /// Returns a vector of memory slices or contiguous memory regions of the row in the trace that
    /// instruction relies on. These registers must be filled in by the `TraceWriter`.
    fn trace_layout(&self) -> Vec<MemorySlice>;

    /// Assigns the row in the trace according to the `witness_layout`. Usually called by the
    /// `TraceWriter`.
    fn assign_row(&self, trace_rows: &mut [Vec<F>], row: &mut [F], row_index: usize) {
        self.trace_layout()
            .into_iter()
            .fold(0, |local_index, memory_slice| {
                memory_slice.assign(trace_rows, local_index, row, row_index)
            });
    }

    /// Constrains the instruction properly within the STARK by using the `ConstraintConsumer`.
    fn packed_generic_constraints<
        FE,
        P,
        const D2: usize,
        const COLUMNS: usize,
        const PUBLIC_INPUTS: usize,
    >(
        &self,
        vars: StarkEvaluationVars<FE, P, { COLUMNS }, { PUBLIC_INPUTS }>,
        yield_constr: &mut crate::constraint_consumer::ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;

    /// Constrains the instruction properly within Plonky2 by using the `RecursiveConstraintConsumer`.
    fn ext_circuit_constraints<const COLUMNS: usize, const PUBLIC_INPUTS: usize>(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: StarkEvaluationTargets<D, { COLUMNS }, { PUBLIC_INPUTS }>,
        yield_constr: &mut crate::constraint_consumer::RecursiveConstraintConsumer<F, D>,
    );
}
