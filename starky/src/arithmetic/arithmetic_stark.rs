//! An abstraction of Starks for emulated field operations handling all the range_checks

use core::marker::PhantomData;
use std::sync::mpsc;

use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::util::transpose;
use plonky2_maybe_rayon::*;

use super::{ArithmeticLayout, ArithmeticOp, ArithmeticParser};
use crate::lookup::{eval_lookups, eval_lookups_circuit, permuted_cols};
use crate::permutation::PermutationPair;
use crate::stark::Stark;
use crate::vars::{StarkEvaluationTargets, StarkEvaluationVars};

/// A layout for a circuit that emulates field operations
pub trait EmulatedCircuitLayout<const NUM_OPERATIONS: usize>: Sized + Send + Sync {
    const PUBLIC_INPUTS: usize;
    const NUM_ARITHMETIC_COLUMNS: usize;
    const ENTRY_COLUMN: usize;
    const TABLE_INDEX: usize;
    const OPERATIONS: [ArithmeticLayout; NUM_OPERATIONS];

    /// Check that the operations allocations are consistent with total number of columns
    fn is_consistent(&self) -> bool {
        assert_eq!(
            Self::TABLE_INDEX,
            Self::ENTRY_COLUMN + Self::NUM_ARITHMETIC_COLUMNS
        );
        true
    }
}

pub const fn num_columns<L: EmulatedCircuitLayout<N>, const N: usize>() -> usize {
    L::ENTRY_COLUMN + 1 + 3 * L::NUM_ARITHMETIC_COLUMNS
}

/// A Stark for emulated field operations
///
/// This stark handles the range checks for the limbs
#[derive(Debug, Clone, Copy)]
pub struct ArithmeticStark<L, const N: usize, F, const D: usize> {
    _marker: PhantomData<(F, L)>,
}

impl<L: EmulatedCircuitLayout<N>, const N: usize, F, const D: usize> ArithmeticStark<L, N, F, D> {
    #[inline]
    pub const fn col_perm_index(i: usize) -> usize {
        2 * i + L::TABLE_INDEX + 1
    }

    #[inline]
    pub const fn table_perm_index(i: usize) -> usize {
        2 * i + 1 + L::TABLE_INDEX + 1
    }

    #[inline]
    pub const fn num_columns() -> usize {
        L::ENTRY_COLUMN + 1 + 3 * L::NUM_ARITHMETIC_COLUMNS
    }

    #[inline]
    pub const fn table_index() -> usize {
        L::ENTRY_COLUMN + L::NUM_ARITHMETIC_COLUMNS
    }
}

impl<const N: usize, L: EmulatedCircuitLayout<N>, F: RichField + Extendable<D>, const D: usize>
    ArithmeticStark<L, N, F, D>
{
    pub fn generate_trace(&self, program: Vec<ArithmeticOp>) -> Vec<PolynomialValues<F>> {
        let num_operations = program.len();
        let num_rows = num_operations;

        let mut trace_rows = vec![Vec::with_capacity(L::NUM_ARITHMETIC_COLUMNS); num_rows];

        // Collecte the trace rows which are processed in parallel
        let (tx, rx) = mpsc::channel::<(usize, Vec<F>)>();

        for (i, op) in program.into_iter().enumerate() {
            let tx = tx.clone();
            rayon::spawn(move || {
                let mut row = ArithmeticParser::<F, D>::op_trace_row(op);
                row.push(F::from_canonical_usize(i));
                tx.send((i, row)).unwrap();
            });
        }
        drop(tx);

        // Insert the trace rows into the trace
        while let Ok((i, mut row)) = rx.recv() {
            trace_rows[i].append(&mut row);
        }

        // Transpose the trace to get the columns and resize to the correct size
        let mut trace_cols = transpose(&trace_rows);
        trace_cols.resize(Self::num_columns(), Vec::with_capacity(num_rows));

        // Calculate the permutation and append permuted columbs to trace
        let (trace_values, perm_values) = trace_cols.split_at_mut(L::TABLE_INDEX + 1);
        (0..L::NUM_ARITHMETIC_COLUMNS)
            .into_par_iter()
            .map(|i| permuted_cols(&trace_values[i], &trace_values[L::TABLE_INDEX]))
            .zip(perm_values.par_iter_mut().chunks(2))
            .for_each(|((col_perm, table_perm), mut trace)| {
                trace[0].extend(col_perm);
                trace[1].extend(table_perm);
            });

        trace_cols
            .into_par_iter()
            .map(PolynomialValues::new)
            .collect()
    }
}

impl<const N: usize, L: EmulatedCircuitLayout<N>, F: RichField + Extendable<D>, const D: usize>
    Stark<F, D> for ArithmeticStark<L, N, F, D>
{
    const COLUMNS: usize = Self::num_columns();
    const PUBLIC_INPUTS: usize = L::PUBLIC_INPUTS;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut crate::constraint_consumer::ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
    {
        for layout in L::OPERATIONS {
            ArithmeticParser::op_packed_generic_constraints(layout, vars, yield_constr);
        }
        // lookp table values
        yield_constr.constraint_first_row(vars.local_values[L::TABLE_INDEX]);
        let table_values_relation =
            vars.local_values[L::TABLE_INDEX] + FE::ONE - vars.next_values[L::TABLE_INDEX];
        yield_constr.constraint_transition(table_values_relation);
        // permutations
        for i in 0..L::NUM_ARITHMETIC_COLUMNS {
            eval_lookups(
                vars,
                yield_constr,
                Self::col_perm_index(i),
                Self::table_perm_index(i),
            );
        }
    }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
        yield_constr: &mut crate::constraint_consumer::RecursiveConstraintConsumer<F, D>,
    ) {
        for layout in L::OPERATIONS {
            ArithmeticParser::op_ext_circuit(layout, builder, vars, yield_constr);
        }
        // lookup table values
        yield_constr.constraint_first_row(builder, vars.local_values[L::TABLE_INDEX]);
        let one = builder.constant_extension(F::Extension::ONE);
        let table_plus_one = builder.add_extension(vars.local_values[L::TABLE_INDEX], one);
        let table_relation =
            builder.sub_extension(table_plus_one, vars.next_values[L::TABLE_INDEX]);
        yield_constr.constraint_transition(builder, table_relation);
        // lookup argumment
        for i in 0..L::NUM_ARITHMETIC_COLUMNS {
            eval_lookups_circuit(
                builder,
                vars,
                yield_constr,
                Self::col_perm_index(i),
                Self::table_perm_index(i),
            );
        }
    }

    fn constraint_degree(&self) -> usize {
        2
    }

    fn permutation_pairs(&self) -> Vec<PermutationPair> {
        (0..L::NUM_ARITHMETIC_COLUMNS)
            .flat_map(|i| {
                [
                    PermutationPair::singletons(i, Self::col_perm_index(i)),
                    PermutationPair::singletons(L::TABLE_INDEX, Self::table_perm_index(i)),
                ]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {

    use num::bigint::RandBigInt;
    use num::BigUint;
    use plonky2::iop::witness::PartialWitness;
    use plonky2::plonk::circuit_builder::CircuitBuilder;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use plonky2::util::timing::TimingTree;

    use super::*;
    use crate::arithmetic::{add, Register};
    use crate::config::StarkConfig;
    use crate::prover::prove;
    use crate::recursive_verifier::{
        add_virtual_stark_proof_with_pis, set_stark_proof_with_pis_target,
        verify_stark_proof_circuit,
    };
    use crate::verifier::verify_stark_proof;

    #[derive(Clone, Copy, Debug)]
    pub struct AddModLayoutCircuit;

    impl EmulatedCircuitLayout<1> for AddModLayoutCircuit {
        const PUBLIC_INPUTS: usize = 0;
        const NUM_ARITHMETIC_COLUMNS: usize = add::NUM_ARITH_COLUMNS;
        const ENTRY_COLUMN: usize = 0;
        const TABLE_INDEX: usize = add::NUM_ARITH_COLUMNS;
        const OPERATIONS: [ArithmeticLayout; 1] = [ArithmeticLayout::Add(add::AddModLayout::new(
            Register::Local(0, add::N_LIMBS),
            Register::Local(add::N_LIMBS, add::N_LIMBS),
            Register::Local(2 * add::N_LIMBS, add::N_LIMBS),
            Register::Local(3 * add::N_LIMBS, add::N_LIMBS),
            Register::Local(4 * add::N_LIMBS, add::NUM_ADD_WITNESS_COLUMNS),
        ))];
    }

    #[test]
    fn test_arithmetic_stark_add() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = ArithmeticStark<AddModLayoutCircuit, 1, F, D>;

        let num_rows = 2u64.pow(16);
        let config = StarkConfig::standard_fast_config();

        let p22519 = BigUint::from(2u32).pow(255) - BigUint::from(19u32);

        let mut rng = rand::thread_rng();

        let mut additions = Vec::new();

        for _ in 0..num_rows {
            let a: BigUint = rng.gen_biguint(255) % &p22519;
            let b = rng.gen_biguint(255) % &p22519;
            let p = p22519.clone();

            let operation = ArithmeticOp::AddMod(a.clone(), b.clone(), p.clone());
            additions.push(operation);
        }

        let stark = S {
            _marker: PhantomData,
        };

        let trace = stark.generate_trace(additions);

        // Verify proof as a stark
        let proof =
            prove::<F, C, S, D>(stark, &config, trace, [], &mut TimingTree::default()).unwrap();
        verify_stark_proof(stark, proof.clone(), &config).unwrap();

        // Verify recursive proof in a circuit
        let config_rec = CircuitConfig::standard_recursion_config();
        let mut recursive_builder = CircuitBuilder::<F, D>::new(config_rec);

        let degree_bits = proof.proof.recover_degree_bits(&config);
        let virtual_proof =
            add_virtual_stark_proof_with_pis(&mut recursive_builder, stark, &config, degree_bits);

        recursive_builder.print_gate_counts(0);

        let mut rec_pw = PartialWitness::new();
        set_stark_proof_with_pis_target(&mut rec_pw, &virtual_proof, &proof);

        verify_stark_proof_circuit::<F, C, S, D>(
            &mut recursive_builder,
            stark,
            virtual_proof,
            &config,
        );

        let recursive_data = recursive_builder.build::<C>();

        let mut timing = TimingTree::new("recursive_proof", log::Level::Debug);
        let recursive_proof = plonky2::plonk::prover::prove(
            &recursive_data.prover_only,
            &recursive_data.common,
            rec_pw,
            &mut timing,
        )
        .unwrap();

        timing.print();
        recursive_data.verify(recursive_proof).unwrap();
    }

    #[derive(Clone, Copy, Debug)]
    pub struct MulModLayoutCircuit;

    use crate::arithmetic::mul;
    impl EmulatedCircuitLayout<1> for MulModLayoutCircuit {
        const PUBLIC_INPUTS: usize = 0;
        const NUM_ARITHMETIC_COLUMNS: usize = mul::NUM_ARITH_COLUMNS;
        const ENTRY_COLUMN: usize = 0;
        const TABLE_INDEX: usize = mul::NUM_ARITH_COLUMNS;
        const OPERATIONS: [ArithmeticLayout; 1] = [ArithmeticLayout::Mul(mul::MulModLayout::new(
            Register::Local(0, mul::N_LIMBS),
            Register::Local(mul::N_LIMBS, mul::N_LIMBS),
            Register::Local(2 * mul::N_LIMBS, mul::N_LIMBS),
            Register::Local(3 * mul::N_LIMBS, mul::NUM_OUTPUT_COLUMNS),
            Register::Local(
                4 * mul::N_LIMBS,
                mul::NUM_CARRY_COLUMNS + mul::NUM_WITNESS_COLUMNS,
            ),
        ))];
    }

    #[test]
    fn test_arithmetic_stark_mul() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = ArithmeticStark<MulModLayoutCircuit, 1, F, D>;

        let num_rows = 2u64.pow(16);
        let config = StarkConfig::standard_fast_config();

        let p22519 = BigUint::from(2u32).pow(255) - BigUint::from(19u32);

        let mut rng = rand::thread_rng();

        let mut multiplication = Vec::new();

        for _ in 0..num_rows {
            let a: BigUint = rng.gen_biguint(255) % &p22519;
            let b = rng.gen_biguint(255) % &p22519;
            let p = p22519.clone();

            let operation = ArithmeticOp::MulMod(a.clone(), b.clone(), p.clone());
            multiplication.push(operation);
        }

        let stark = S {
            _marker: PhantomData,
        };

        let trace = stark.generate_trace(multiplication);

        // Verify proof as a stark
        let proof =
            prove::<F, C, S, D>(stark, &config, trace, [], &mut TimingTree::default()).unwrap();
        verify_stark_proof(stark, proof.clone(), &config).unwrap();

        // Verify recursive proof in a circuit
        let config_rec = CircuitConfig::standard_recursion_config();
        let mut recursive_builder = CircuitBuilder::<F, D>::new(config_rec);

        let degree_bits = proof.proof.recover_degree_bits(&config);
        let virtual_proof =
            add_virtual_stark_proof_with_pis(&mut recursive_builder, stark, &config, degree_bits);

        recursive_builder.print_gate_counts(0);

        let mut rec_pw = PartialWitness::new();
        set_stark_proof_with_pis_target(&mut rec_pw, &virtual_proof, &proof);

        verify_stark_proof_circuit::<F, C, S, D>(
            &mut recursive_builder,
            stark,
            virtual_proof,
            &config,
        );

        let recursive_data = recursive_builder.build::<C>();

        let mut timing = TimingTree::new("recursive_proof", log::Level::Debug);
        let recursive_proof = plonky2::plonk::prover::prove(
            &recursive_data.prover_only,
            &recursive_data.common,
            rec_pw,
            &mut timing,
        )
        .unwrap();

        timing.print();
        recursive_data.verify(recursive_proof).unwrap();
    }
}
