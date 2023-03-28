use crate::stark::Stark;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::field::types::PrimeField64;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::util::transpose;

use core::marker::PhantomData;
use num::BigUint;

pub const N_LIMBS: usize = 16;
pub const NUM_ARITH_COLUMNS : usize = 6 * N_LIMBS;
const RANGE_MAX: usize = 1usize << 16; // Range check strict upper bound

#[derive(Clone)]
pub struct AddModStark<F, const D: usize > {
    _marker: PhantomData<F>,
}

type AdditionTuple = (BigUint, BigUint, BigUint);

impl<F: PrimeField64, const D: usize> AddModStark<F, D> {
    /// Generate trace for addition stark
    fn generate_trace(&self, additions : Vec<AdditionTuple>)-> Vec<PolynomialValues<F>> {
        let max_rows = core::cmp::max(2 * additions.len(), RANGE_MAX); // note : range_max not needed yet
        let mut trace_rows : Vec<Vec<F>> = Vec::with_capacity(max_rows);

        for (a, b, m) in additions {
            let row= ArithmeticParser::<F>::add_to_rows(a, b, m);
            trace_rows.push(row);
        }

        let trace_cols = transpose(&trace_rows);

        trace_cols.into_iter().map(PolynomialValues::new).collect()
    }
}


pub struct ArithmeticParser<F> {
    _marker: PhantomData<F>,
}

impl<F : PrimeField64> ArithmeticParser<F> {

    /// Converts two BigUint inputs into the correspinding rows of addition mod modulus
    /// 
    /// a + b = c mod m
    /// 
    /// Each element represented by a polynomial a(x), b(x), c(x), m(x) of 16 limbs of 16 bits each
    /// We will witness the relation
    ///  a(x) + b(x) - c(x) - carry * m(x) - (x - β) * s(x) == 0
    /// where carry = 0 or carry = 1
    /// the first row will contain a(x), b(x), m(x) and the second row will contain c(x), q(x), s(x)
    pub fn add_to_rows(input_0: BigUint, input_1 : BigUint, modulus : BigUint ) -> Vec<F> {
        let result = (&input_0 + &input_1) % &modulus;
        debug_assert!(&result < &modulus);
        let carry = (&input_0 + &input_1 - &result) / &modulus;
        debug_assert!(&carry == &BigUint::from(0u32) || &carry == &BigUint::from(1u32));

        let carry_digits = Self::bigint_into_u16_F_digits(&carry, N_LIMBS);
        
        let mut row = vec![F::ZERO; NUM_ARITH_COLUMNS];

        let input_0_digits = Self::bigint_into_u16_F_digits(&input_0, N_LIMBS);
        let input_1_digits = Self::bigint_into_u16_F_digits(&input_1, N_LIMBS);
        let result_digits = Self::bigint_into_u16_F_digits(&result, N_LIMBS);
        let modulus_digits = Self::bigint_into_u16_F_digits(&modulus, N_LIMBS);
        
        let carry_mod = &carry*&modulus; 
        let carry_mod_digits = Self::bigint_into_u16_F_digits(&carry_mod, N_LIMBS);
 

        // constr_poly is the array of coefficients of the polynomial
        //
        // a(x) +  b(x) - c(x) - carry*m(x) = const(x)
        // note that we don't care about the coefficients of constr(x) at all, just that it will have a root. 
        let consr_polynomial : Vec<F> = input_0_digits.iter()
            .zip(input_1_digits.iter())
            .zip(carry_mod_digits.iter())
            .map(|((a, b), c)| *a + *b - *c).collect();
        
        assert_eq!(consr_polynomial.len(), N_LIMBS);
        // By assumption β := 2^16 is a root of `a`, i.e. (x - β) divides
        // `a`; if we write
        //
        //    a(x) = \sum_{i=0}^{N-1} a[i] x^i
        //         = (x - β) \sum_{i=0}^{N-2} q[i] x^i
        //
        // then by comparing coefficients it is easy to see that
        //
        //   q[0] = -a[0] / β  and  q[i] = (q[i-1] - a[i]) / β
        //
        //  NOTE : Doing divisions in F::Goldilocks probably not the most efficient
        let mut aux_digits = Vec::new();
        aux_digits.push(-consr_polynomial[0]/F::from_canonical_u32(65536u32));

        for deg in 1..N_LIMBS-1 {
            let temp1 = aux_digits[deg - 1];
            let digit =temp1 - consr_polynomial[deg];
            let quot = digit/F::from_canonical_u32(65536u32);
            aux_digits.push(quot);
        }
        aux_digits.push(F::ZERO);

        // Add inputs and modulus as values in first row
        input_0_digits.iter().zip(input_1_digits.iter()).enumerate()
            .for_each(|(i, (x, y))| {
                row[i] = *x;
                row[i + N_LIMBS] = *y;
                row[i + 2 * N_LIMBS] = modulus_digits[i];
            });

        // Add result, quotient and aux polynomial as values in second row
        result_digits.iter().zip(carry_digits.iter()).enumerate()
            .for_each(|(i, (x, y))| {
                row[i + 3* N_LIMBS] = *x;
                row[i + N_LIMBS + 3* N_LIMBS] = *y;
                row[i + 2 * N_LIMBS + 3* N_LIMBS] = aux_digits[i];
            });        
       
       row
    }

    pub fn bigint_into_u16_digits(x: &BigUint) -> Vec<u16> {
        x.iter_u32_digits()
            .flat_map(|x| vec![x as u16, (x >> 16) as u16])
            .collect()
    }

    pub fn bigint_into_u16_F_digits(x: &BigUint, digits : usize) -> Vec<F> {
        let mut x_limbs :Vec<_> = Self::bigint_into_u16_digits(x)
            .iter()
            .map(|xi| F::from_canonical_u16(*xi))
            .collect();
        assert!(x_limbs.len() <= digits, "Number too large to fit in {} digits", digits);
        for i in x_limbs.len()..digits {
            x_limbs.push(F::ZERO);
        }
        x_limbs
    }
}




impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for AddModStark<F, D> {
    const COLUMNS: usize = NUM_ARITH_COLUMNS;
    const PUBLIC_INPUTS: usize = 0;

    fn eval_packed_generic<FE, P, const D2: usize>(
            &self,
            vars: crate::vars::StarkEvaluationVars<FE, P, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
            yield_constr: &mut crate::constraint_consumer::ConstraintConsumer<P>,
        ) where
            FE: plonky2::field::extension::FieldExtension<D2, BaseField = F>,
            P: plonky2::field::packed::PackedField<Scalar = FE> {
       
        // a(x) + b(x) - c(x) - carry * m(x) - (x - β) * s(x) == 0
        // the first row = (a(x), b(x), m(x)) and the second ro = (c(x), carry(x), s(x))
        let mut sum_minus_carry = vec![P::default(); N_LIMBS];
        for i in 0..N_LIMBS {
            sum_minus_carry[i] = vars.local_values[i] + vars.local_values[i + N_LIMBS]
                                 - vars.local_values[i + 3*N_LIMBS]
                                 -vars.local_values[0 + 3*N_LIMBS]*vars.local_values[i + 2*N_LIMBS];
        }
        let mut auxillary = vec![P::default(); N_LIMBS];
        let pow_2 = P::Scalar::from_canonical_u32(2u32.pow(16));
        auxillary[0] = -vars.local_values[2*N_LIMBS + 3*N_LIMBS].mul(pow_2);
        for i in 1..N_LIMBS-1 {
            auxillary[i] = vars.local_values[i - 1 + 2*N_LIMBS + 3*N_LIMBS] - vars.local_values[i + 2*N_LIMBS+ 3*N_LIMBS].mul(pow_2);
        } 
        auxillary[N_LIMBS-1] = vars.local_values[N_LIMBS-1 + 2*N_LIMBS + 3*N_LIMBS];       
        for i in 0..N_LIMBS {
            yield_constr.constraint_transition(sum_minus_carry[i] - auxillary[i]);
        }
    }

    fn eval_ext_circuit(
            &self,
            builder: &mut plonky2::plonk::circuit_builder::CircuitBuilder<F, D>,
            vars: crate::vars::StarkEvaluationTargets<D, { Self::COLUMNS }, { Self::PUBLIC_INPUTS }>,
            yield_constr: &mut crate::constraint_consumer::RecursiveConstraintConsumer<F, D>,
        ) {
            let mut sub_minus_carry = Vec::new();
            for i in 0..N_LIMBS {
                let sum = builder.add_extension(vars.local_values[i], vars.local_values[i + N_LIMBS]);
                let sub_minus_res = builder.sub_extension(sum, vars.local_values[i+ 3*N_LIMBS]);
                let carry_mod = builder.mul_extension(vars.local_values[0 + 3*N_LIMBS], vars.local_values[i + 2*N_LIMBS]);
                sub_minus_carry.push(builder.sub_extension(sub_minus_res, carry_mod));
            }

            let mut auxilary = Vec::new();
            let pow_2 = builder.constant_extension(
                              <F as Extendable<D>>::Extension::from(
                                  F::from_canonical_u32(2u32.pow(16))));
            let first_coeff = builder.mul_extension(vars.local_values[2*N_LIMBS + 3*N_LIMBS], pow_2);
            let neg_one = builder.neg_one_extension();
            auxilary.push(builder.mul_extension(neg_one, first_coeff));
            for i in 1..N_LIMBS-1 {
                let aux_pow2 = builder.mul_extension(vars.local_values[i + 2*N_LIMBS+ 3*N_LIMBS], pow_2);
                auxilary.push(builder.sub_extension(vars.local_values[i + 1 + 2*N_LIMBS+ 3*N_LIMBS], aux_pow2));
            }
            auxilary.push(vars.local_values[N_LIMBS-1 + 2*N_LIMBS+ 3*N_LIMBS]);

            for i in 0..N_LIMBS {
                let constraint = builder.sub_extension(sub_minus_carry[i], auxilary[i]);
                yield_constr.constraint_transition(builder, constraint);
            }
        }
        
    fn constraint_degree(&self) -> usize {
        2
    }
}



#[cfg(test)]
mod tests {
    use plonky2::plonk::config::{PoseidonGoldilocksConfig, GenericConfig};
    use plonky2::util::timing::TimingTree;
    use crate::prover::prove;
    use crate::verifier::verify_stark_proof;

    use crate::config::StarkConfig;

    use super::*;

    #[test]
    fn test_add_stark() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type S = AddModStark<F, D>;

        let config = StarkConfig::standard_fast_config();
        let num_rows = 2;

        let stark = S {_marker: PhantomData};

        let a = BigUint::from(1u32);
        let b = BigUint::from(2u32);
        let m = BigUint::from(3u32);

        let additions = vec![(a, b, m)];

        let trace = stark.generate_trace(additions);
        let proof = prove::<F, C, S, D>(
            stark.clone(),
            &config,
            trace,
            [],
            &mut TimingTree::default(),
        ).unwrap();
        verify_stark_proof(stark, proof.clone(), &config).unwrap();
    }
}