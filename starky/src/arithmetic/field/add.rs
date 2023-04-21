//! Implements non-native field addition as an "instruction".
//!
//! To understand the implementation, it may be useful to refer to `mod.rs`.

use anyhow::Result;
use num::BigUint;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_builder::CircuitBuilder;

use super::constrain::packed_generic_constrain_field_operation;
use super::*;
use crate::arithmetic::builder::StarkBuilder;
use crate::arithmetic::chip::StarkParameters;
use crate::arithmetic::field::constrain::ext_circuit_constrain_field_operation;
use crate::arithmetic::instruction::Instruction;
use crate::arithmetic::polynomial::{
    to_u16_le_limbs_polynomial, Polynomial, PolynomialGadget, PolynomialOps,
};
use crate::arithmetic::register::{
    ArrayRegister, FieldRegister, MemorySlice, RegisterSerializable, U16Register,
};
use crate::arithmetic::trace::TraceWriter;
use crate::arithmetic::utils::{compute_root_quotient_and_shift, split_u32_limbs_to_u16_limbs};
use crate::vars::{StarkEvaluationTargets, StarkEvaluationVars};

#[derive(Debug, Clone, Copy)]
pub struct FpAddInstruction<P: FieldParameters> {
    a: FieldRegister<P>,
    b: FieldRegister<P>,
    result: FieldRegister<P>,
    carry: FieldRegister<P>,
    witness_low: ArrayRegister<U16Register>,
    witness_high: ArrayRegister<U16Register>,
}

impl<L: StarkParameters<F, D>, F: RichField + Extendable<D>, const D: usize> StarkBuilder<L, F, D> {
    /// Given two field elements `a` and `b`, computes the sum `a + b = c`.
    pub fn fpadd<P: FieldParameters>(
        &mut self,
        a: &FieldRegister<P>,
        b: &FieldRegister<P>,
    ) -> Result<(FieldRegister<P>, FpAddInstruction<P>)>
    where
        L::Instruction: From<FpAddInstruction<P>>,
    {
        let result = self.alloc::<FieldRegister<P>>();
        let carry = self.alloc::<FieldRegister<P>>();
        let witness_low = self.alloc_array::<U16Register>(P::NB_WITNESS_LIMBS);
        let witness_high = self.alloc_array::<U16Register>(P::NB_WITNESS_LIMBS);
        let instr = FpAddInstruction {
            a: *a,
            b: *b,
            result,
            carry,
            witness_low,
            witness_high,
        };
        self.insert_instruction(instr.into())?;
        Ok((result, instr))
    }
}

impl<F: RichField + Extendable<D>, const D: usize> TraceWriter<F, D> {
    /// Writes a `FpAddInstruction` to the trace and returns the result.
    pub fn write_fpadd<P: FieldParameters>(
        &self,
        row_index: usize,
        a: &BigUint,
        b: &BigUint,
        instruction: FpAddInstruction<P>,
    ) -> Result<BigUint> {
        // Compute field addition in the integers.
        let modulus = P::modulus();
        let result = (a + b) % &modulus;
        let carry = (a + b - &result) / &modulus;
        debug_assert!(result < modulus);
        debug_assert!(carry < modulus);
        debug_assert_eq!(&carry * &modulus, a + b - &result);

        // Make little endian polynomial limbs.
        let p_a = to_u16_le_limbs_polynomial::<F, P>(a);
        let p_b = to_u16_le_limbs_polynomial::<F, P>(b);
        let p_modulus = to_u16_le_limbs_polynomial::<F, P>(&modulus);
        let p_result = to_u16_le_limbs_polynomial::<F, P>(&result);
        let p_carry = to_u16_le_limbs_polynomial::<F, P>(&carry);

        // Compute the vanishing polynomial.
        let p_vanishing = &p_a + &p_b - &p_result - &p_carry * &p_modulus;
        debug_assert_eq!(p_vanishing.degree(), P::NB_WITNESS_LIMBS);

        // Compute the witness.
        let p_witness = compute_root_quotient_and_shift(&p_vanishing, P::WITNESS_OFFSET);
        let (p_witness_low, p_witness_high) = split_u32_limbs_to_u16_limbs(&p_witness);

        // Row must match layout of instruction.
        self.write_to_layout(
            row_index,
            instruction,
            vec![
                p_result.coefficients,
                p_carry.coefficients,
                p_witness_low,
                p_witness_high,
            ],
        )?;
        Ok(result)
    }
}

impl<F: RichField + Extendable<D>, const D: usize, P: FieldParameters> Instruction<F, D>
    for FpAddInstruction<P>
{
    fn witness_layout(&self) -> Vec<MemorySlice> {
        vec![
            *self.result.register(),
            *self.carry.register(),
            *self.witness_low.register(),
            *self.witness_high.register(),
        ]
    }

    fn packed_generic_constraints<
        FE,
        PF,
        const D2: usize,
        const COLUMNS: usize,
        const PUBLIC_INPUTS: usize,
    >(
        &self,
        vars: StarkEvaluationVars<FE, PF, { COLUMNS }, { PUBLIC_INPUTS }>,
        yield_constr: &mut crate::constraint_consumer::ConstraintConsumer<PF>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        PF: PackedField<Scalar = FE>,
    {
        // Get the packed entries.
        let p_a = self.a.register().packed_entries_slice(&vars);
        let p_b = self.b.register().packed_entries_slice(&vars);
        let p_result = self.result.register().packed_entries_slice(&vars);
        let p_carry = self.carry.register().packed_entries_slice(&vars);
        let p_witness_low = self.witness_low.register().packed_entries_slice(&vars);
        let p_witness_high = self.witness_high.register().packed_entries_slice(&vars);

        // Compute the vanishing polynomial a(x) + b(x) - result(x) - carry(x) * p(x).
        let p_a_plus_b = PolynomialOps::add(p_a, p_b);
        let p_a_plus_b_minus_result = PolynomialOps::sub(&p_a_plus_b, p_result);
        let p_modulus = Polynomial::<FE>::from_iter(modulus_field_iter::<FE, P>());
        let p_carry_mul_modulus = PolynomialOps::scalar_poly_mul(p_carry, p_modulus.as_slice());
        let p_vanishing = PolynomialOps::sub(&p_a_plus_b_minus_result, &p_carry_mul_modulus);

        // Check [a(x) + b(x) - result(x) - carry(x) * p(x)] - [witness(x) * (x-2^16)] = 0.
        packed_generic_constrain_field_operation::<F, D, FE, PF, D2, P>(
            yield_constr,
            p_vanishing,
            p_witness_low,
            p_witness_high,
        );
    }

    fn ext_circuit_constraints<const COLUMNS: usize, const PUBLIC_INPUTS: usize>(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: StarkEvaluationTargets<D, { COLUMNS }, { PUBLIC_INPUTS }>,
        yield_constr: &mut crate::constraint_consumer::RecursiveConstraintConsumer<F, D>,
    ) {
        type PG = PolynomialGadget;

        // Get the packed entries.
        let p_a = self.a.register().evaluation_targets(&vars);
        let p_b = self.b.register().evaluation_targets(&vars);
        let p_result = self.result.register().evaluation_targets(&vars);
        let p_carry = self.carry.register().evaluation_targets(&vars);
        let p_witness_low = self.witness_low.register().evaluation_targets(&vars);
        let p_witness_high = self.witness_high.register().evaluation_targets(&vars);

        // Compute the vanishing polynomial a(x) + b(x) - result(x) - carry(x) * p(x).
        let p_a_plus_b = PG::add_extension(builder, p_a, p_b);
        let p_a_plus_b_minus_result = PG::sub_extension(builder, &p_a_plus_b, p_result);
        let p_limbs = PG::constant_extension(
            builder,
            &modulus_field_iter::<F::Extension, P>().collect::<Vec<_>>()[..],
        );
        let p_mul_times_carry = PG::mul_extension(builder, p_carry, &p_limbs[..]);
        let p_vanishing = PG::sub_extension(builder, &p_a_plus_b_minus_result, &p_mul_times_carry);

        ext_circuit_constrain_field_operation::<F, D, P>(
            builder,
            yield_constr,
            p_vanishing,
            p_witness_low,
            p_witness_high,
        );
    }
}

#[cfg(test)]
mod tests {
    use num::bigint::RandBigInt;
    use plonky2::iop::witness::PartialWitness;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;
    use rand::thread_rng;

    use super::*;
    use crate::arithmetic::builder::StarkBuilder;
    use crate::arithmetic::chip::{StarkParameters, TestStark};
    use crate::arithmetic::field::Fp25519Param;
    use crate::arithmetic::trace::trace;
    use crate::config::StarkConfig;
    use crate::prover::prove;
    use crate::recursive_verifier::{
        add_virtual_stark_proof_with_pis, set_stark_proof_with_pis_target,
        verify_stark_proof_circuit,
    };
    use crate::verifier::verify_stark_proof;

    #[derive(Clone, Debug, Copy)]
    struct FpAddTest;

    impl<F: RichField + Extendable<D>, const D: usize> StarkParameters<F, D> for FpAddTest {
        const NUM_ARITHMETIC_COLUMNS: usize = 124;
        const NUM_FREE_COLUMNS: usize = 0;

        type Instruction = FpAddInstruction<Fp25519Param>;
    }

    #[test]
    fn test_fpadd() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type Fp = Fp25519;
        type S = TestStark<FpAddTest, F, D>;

        let _ = env_logger::builder().is_test(true).try_init();
        let mut builder = StarkBuilder::<FpAddTest, F, D>::new();

        let a = builder.alloc::<Fp>();
        let b = builder.alloc::<Fp>();

        let (_, a_add_b_ins) = builder.fpadd(&a, &b).unwrap();
        builder.write_data(&a).unwrap();
        builder.write_data(&b).unwrap();

        let (chip, spec) = builder.build();

        // Construct the trace
        let num_rows = 2u64.pow(16);
        let (handle, generator) = trace::<F, D>(spec);

        let p = Fp25519Param::modulus();

        let mut timing = TimingTree::new("stark_proof", log::Level::Debug);

        let trace = timed!(timing, "generate trace", {
            let mut rng = thread_rng();
            for i in 0..num_rows {
                let a_int: BigUint = rng.gen_biguint(256) % &p;
                let b_int = rng.gen_biguint(256) % &p;
                //let handle = handle.clone();
                //rayon::spawn(move || {
                handle.write_field(i as usize, &a_int, a).unwrap();
                handle.write_field(i as usize, &b_int, b).unwrap();
                handle
                    .write_fpadd(i as usize, &a_int, &b_int, a_add_b_ins)
                    .unwrap();
                //});
            }
            drop(handle);

            generator.generate_trace(&chip, num_rows as usize).unwrap()
        });

        let config = StarkConfig::standard_fast_config();
        let stark = TestStark::new(chip);

        // Verify proof as a stark
        let proof = timed!(
            timing,
            "generate proof",
            prove::<F, C, S, D>(
                stark.clone(),
                &config,
                trace,
                [],
                &mut TimingTree::default(),
            )
            .unwrap()
        );

        verify_stark_proof(stark.clone(), proof.clone(), &config).unwrap();

        // Verify recursive proof in a circuit
        let config_rec = CircuitConfig::standard_recursion_config();
        let mut recursive_builder = CircuitBuilder::<F, D>::new(config_rec);

        let degree_bits = proof.proof.recover_degree_bits(&config);
        let virtual_proof = add_virtual_stark_proof_with_pis(
            &mut recursive_builder,
            stark.clone(),
            &config,
            degree_bits,
        );

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

        let recursive_proof = timed!(
            timing,
            "generate recursive proof",
            plonky2::plonk::prover::prove(
                &recursive_data.prover_only,
                &recursive_data.common,
                rec_pw,
                &mut TimingTree::default(),
            )
            .unwrap()
        );

        timed!(
            timing,
            "verify recursive proof",
            recursive_data.verify(recursive_proof).unwrap()
        );
        timing.print();
    }
}
