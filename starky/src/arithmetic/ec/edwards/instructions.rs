use den::Den;

use super::add::FromEdwardsAdd;
use super::*;
use crate::arithmetic::bool::Selector;
use crate::arithmetic::field::add::FpAdd;
use crate::arithmetic::field::mul::{FpMul, FpMulConst};
use crate::arithmetic::field::quad::FpQuad;
use crate::arithmetic::instruction::Instruction;
use crate::arithmetic::register::Register;

#[derive(Debug, Clone, Copy)]
pub enum EdWardsMicroInstruction<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize> {
    Den(Den<E::FieldParam, N_LIMBS>),
    FpAdd(FpAdd<E::FieldParam, N_LIMBS>),
    FpMul(FpMul<E::FieldParam, N_LIMBS>),
    FpQuad(FpQuad<E::FieldParam, N_LIMBS>),
    FpMulConst(FpMulConst<E::FieldParam, N_LIMBS>),
    Selector(Selector<FieldRegister<E::FieldParam, N_LIMBS>>),
}

impl<E: EdwardsParameters<N>, F: RichField + Extendable<D>, const D: usize, const N: usize>
    Instruction<F, D> for EdWardsMicroInstruction<E, N>
{
    fn memory_vec(&self) -> Vec<Register> {
        match self {
            EdWardsMicroInstruction::Den(den) => {
                <Den<E::FieldParam, N> as Instruction<F, D>>::memory_vec(den)
            }
            EdWardsMicroInstruction::FpAdd(fp_add) => {
                <FpAdd<E::FieldParam, N> as Instruction<F, D>>::memory_vec(fp_add)
            }
            EdWardsMicroInstruction::FpMul(fp_mul) => {
                <FpMul<E::FieldParam, N> as Instruction<F, D>>::memory_vec(fp_mul)
            }
            EdWardsMicroInstruction::FpQuad(fp_quad) => {
                <FpQuad<E::FieldParam, N> as Instruction<F, D>>::memory_vec(fp_quad)
            }
            EdWardsMicroInstruction::FpMulConst(fp_mul_const) => {
                <FpMulConst<E::FieldParam, N> as Instruction<F, D>>::memory_vec(fp_mul_const)
            }
            EdWardsMicroInstruction::Selector(selector) => {
                <Selector<FieldRegister<E::FieldParam, N>> as Instruction<F, D>>::memory_vec(
                    selector,
                )
            }
        }
    }

    fn assign_row(&self, trace_rows: &mut [Vec<F>], row: &mut [F], row_index: usize) {
        match self {
            EdWardsMicroInstruction::Den(den) => {
                <Den<E::FieldParam, N> as Instruction<F, D>>::assign_row(
                    den, trace_rows, row, row_index,
                )
            }
            EdWardsMicroInstruction::FpAdd(fp_add) => {
                <FpAdd<E::FieldParam, N> as Instruction<F, D>>::assign_row(
                    fp_add, trace_rows, row, row_index,
                )
            }
            EdWardsMicroInstruction::FpMul(fp_mul) => {
                <FpMul<E::FieldParam, N> as Instruction<F, D>>::assign_row(
                    fp_mul, trace_rows, row, row_index,
                )
            }
            EdWardsMicroInstruction::FpQuad(fp_quad) => {
                <FpQuad<E::FieldParam, N> as Instruction<F, D>>::assign_row(
                    fp_quad, trace_rows, row, row_index,
                )
            }
            EdWardsMicroInstruction::FpMulConst(fp_mul_const) => {
                <FpMulConst<E::FieldParam, N> as Instruction<F, D>>::assign_row(
                    fp_mul_const,
                    trace_rows,
                    row,
                    row_index,
                )
            }
            EdWardsMicroInstruction::Selector(selector) => {
                <Selector<FieldRegister<E::FieldParam, N>> as Instruction<F, D>>::assign_row(
                    selector, trace_rows, row, row_index,
                )
            }
        }
    }

    fn witness_data(&self) -> Option<crate::arithmetic::register::WitnessData> {
        match self {
            EdWardsMicroInstruction::Den(den) => {
                <Den<E::FieldParam, N> as Instruction<F, D>>::witness_data(den)
            }
            EdWardsMicroInstruction::FpAdd(fp_add) => {
                <FpAdd<E::FieldParam, N> as Instruction<F, D>>::witness_data(fp_add)
            }
            EdWardsMicroInstruction::FpMul(fp_mul) => {
                <FpMul<E::FieldParam, N> as Instruction<F, D>>::witness_data(fp_mul)
            }
            EdWardsMicroInstruction::FpQuad(fp_quad) => {
                <FpQuad<E::FieldParam, N> as Instruction<F, D>>::witness_data(fp_quad)
            }
            EdWardsMicroInstruction::FpMulConst(fp_mul_const) => {
                <FpMulConst<E::FieldParam, N> as Instruction<F, D>>::witness_data(fp_mul_const)
            }
            EdWardsMicroInstruction::Selector(selector) => {
                <Selector<FieldRegister<E::FieldParam, N>> as Instruction<F, D>>::witness_data(
                    selector,
                )
            }
        }
    }

    fn set_witness(&mut self, witness: Register) -> Result<()> {
        match self {
            EdWardsMicroInstruction::Den(den) => {
                <Den<E::FieldParam, N> as Instruction<F, D>>::set_witness(den, witness)
            }
            EdWardsMicroInstruction::FpAdd(fp_add) => {
                <FpAdd<E::FieldParam, N> as Instruction<F, D>>::set_witness(fp_add, witness)
            }
            EdWardsMicroInstruction::FpMul(fp_mul) => {
                <FpMul<E::FieldParam, N> as Instruction<F, D>>::set_witness(fp_mul, witness)
            }
            EdWardsMicroInstruction::FpQuad(fp_quad) => {
                <FpQuad<E::FieldParam, N> as Instruction<F, D>>::set_witness(fp_quad, witness)
            }
            EdWardsMicroInstruction::FpMulConst(fp_mul_const) => {
                <FpMulConst<E::FieldParam, N> as Instruction<F, D>>::set_witness(
                    fp_mul_const,
                    witness,
                )
            }
            EdWardsMicroInstruction::Selector(selector) => {
                <Selector<FieldRegister<E::FieldParam, N>> as Instruction<F, D>>::set_witness(
                    selector, witness,
                )
            }
        }
    }

    fn packed_generic_constraints<
        FE,
        P,
        const D2: usize,
        const COLUMNS: usize,
        const PUBLIC_INPUTS: usize,
    >(
        &self,
        vars: crate::vars::StarkEvaluationVars<FE, P, { COLUMNS }, { PUBLIC_INPUTS }>,
        yield_constr: &mut crate::constraint_consumer::ConstraintConsumer<P>,
    ) where
        FE: plonky2::field::extension::FieldExtension<D2, BaseField = F>,
        P: plonky2::field::packed::PackedField<Scalar = FE>,
    {
        match self {
            EdWardsMicroInstruction::Den(den) => {
                <Den<E::FieldParam, N> as Instruction<F, D>>::packed_generic_constraints(
                    den,
                    vars,
                    yield_constr,
                )
            }
            EdWardsMicroInstruction::FpAdd(fp_add) => <FpAdd<E::FieldParam, N> as Instruction<
                F,
                D,
            >>::packed_generic_constraints(
                fp_add, vars, yield_constr
            ),
            EdWardsMicroInstruction::FpMul(fp_mul) => <FpMul<E::FieldParam, N> as Instruction<
                F,
                D,
            >>::packed_generic_constraints(
                fp_mul, vars, yield_constr
            ),
            EdWardsMicroInstruction::FpQuad(fp_quad) => <FpQuad<E::FieldParam, N> as Instruction<
                F,
                D,
            >>::packed_generic_constraints(
                fp_quad, vars, yield_constr
            ),
            EdWardsMicroInstruction::FpMulConst(fp_mul_const) => {
                <FpMulConst<E::FieldParam, N> as Instruction<F, D>>::packed_generic_constraints(
                    fp_mul_const,
                    vars,
                    yield_constr,
                )
            }
            EdWardsMicroInstruction::Selector(selector) => <Selector<
                FieldRegister<E::FieldParam, N>,
            > as Instruction<F, D>>::packed_generic_constraints(
                selector, vars, yield_constr
            ),
        }
    }

    fn ext_circuit_constraints<const COLUMNS: usize, const PUBLIC_INPUTS: usize>(
        &self,
        builder: &mut plonky2::plonk::circuit_builder::CircuitBuilder<F, D>,
        vars: crate::vars::StarkEvaluationTargets<D, { COLUMNS }, { PUBLIC_INPUTS }>,
        yield_constr: &mut crate::constraint_consumer::RecursiveConstraintConsumer<F, D>,
    ) {
        match self {
            EdWardsMicroInstruction::Den(den) => {
                <Den<E::FieldParam, N> as Instruction<F, D>>::ext_circuit_constraints(
                    den,
                    builder,
                    vars,
                    yield_constr,
                )
            }
            EdWardsMicroInstruction::FpAdd(fp_add) => <FpAdd<E::FieldParam, N> as Instruction<
                F,
                D,
            >>::ext_circuit_constraints(
                fp_add, builder, vars, yield_constr
            ),
            EdWardsMicroInstruction::FpMul(fp_mul) => <FpMul<E::FieldParam, N> as Instruction<
                F,
                D,
            >>::ext_circuit_constraints(
                fp_mul, builder, vars, yield_constr
            ),
            EdWardsMicroInstruction::FpQuad(fp_quad) => <FpQuad<E::FieldParam, N> as Instruction<
                F,
                D,
            >>::ext_circuit_constraints(
                fp_quad, builder, vars, yield_constr
            ),
            EdWardsMicroInstruction::FpMulConst(fp_mul_const) => {
                <FpMulConst<E::FieldParam, N> as Instruction<F, D>>::ext_circuit_constraints(
                    fp_mul_const,
                    builder,
                    vars,
                    yield_constr,
                )
            }
            EdWardsMicroInstruction::Selector(selector) => <Selector<
                FieldRegister<E::FieldParam, N>,
            > as Instruction<F, D>>::ext_circuit_constraints(
                selector, builder, vars, yield_constr
            ),
        }
    }
}

impl<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize> From<FpMul<E::FieldParam, N_LIMBS>>
    for EdWardsMicroInstruction<E, N_LIMBS>
{
    fn from(fp_mul: FpMul<E::FieldParam, N_LIMBS>) -> Self {
        EdWardsMicroInstruction::FpMul(fp_mul)
    }
}

impl<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize> From<FpAdd<E::FieldParam, N_LIMBS>>
    for EdWardsMicroInstruction<E, N_LIMBS>
{
    fn from(fp_add: FpAdd<E::FieldParam, N_LIMBS>) -> Self {
        EdWardsMicroInstruction::FpAdd(fp_add)
    }
}

impl<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize> From<FpQuad<E::FieldParam, N_LIMBS>>
    for EdWardsMicroInstruction<E, N_LIMBS>
{
    fn from(fp_quad: FpQuad<E::FieldParam, N_LIMBS>) -> Self {
        EdWardsMicroInstruction::FpQuad(fp_quad)
    }
}

impl<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize> From<FpMulConst<E::FieldParam, N_LIMBS>>
    for EdWardsMicroInstruction<E, N_LIMBS>
{
    fn from(fp_mul_const: FpMulConst<E::FieldParam, N_LIMBS>) -> Self {
        EdWardsMicroInstruction::FpMulConst(fp_mul_const)
    }
}

impl<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize> From<Den<E::FieldParam, N_LIMBS>>
    for EdWardsMicroInstruction<E, N_LIMBS>
{
    fn from(den: Den<E::FieldParam, N_LIMBS>) -> Self {
        EdWardsMicroInstruction::Den(den)
    }
}

impl<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize>
    From<Selector<FieldRegister<E::FieldParam, N_LIMBS>>> for EdWardsMicroInstruction<E, N_LIMBS>
{
    fn from(selector: Selector<FieldRegister<E::FieldParam, N_LIMBS>>) -> Self {
        EdWardsMicroInstruction::Selector(selector)
    }
}

impl<E: EdwardsParameters<N>, const N: usize> FromEdwardsAdd<E, N>
    for EdWardsMicroInstruction<E, N>
{
}
