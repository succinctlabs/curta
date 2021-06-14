use crate::circuit_builder::CircuitBuilder;
use crate::field::extension_field::target::ExtensionTarget;
use crate::field::extension_field::Extendable;
use crate::field::field::Field;
use crate::gates::gate::{Gate, GateRef};
use crate::generator::{SimpleGenerator, WitnessGenerator};
use crate::plonk_common::{reduce_with_powers, reduce_with_powers_recursive};
use crate::target::Target;
use crate::vars::{EvaluationTargets, EvaluationVars};
use crate::witness::PartialWitness;
use std::ops::Range;

/// A gate which can sum base W limbs and the reversed limbs.
#[derive(Debug)]
pub struct BaseSumGate<const B: usize> {
    num_limbs: usize,
}

impl<const B: usize> BaseSumGate<B> {
    pub fn new<F: Extendable<D>, const D: usize>(num_limbs: usize) -> GateRef<F, D> {
        GateRef::new(BaseSumGate::<B> { num_limbs })
    }

    pub const WIRE_SUM: usize = 0;
    pub const WIRE_REVERSED_SUM: usize = 1;
    pub const WIRE_LIMBS_START: usize = 2;

    /// Returns the index of the `i`th limb wire.
    pub fn limbs(&self) -> Range<usize> {
        Self::WIRE_LIMBS_START..Self::WIRE_LIMBS_START + self.num_limbs
    }
}

impl<F: Extendable<D>, const D: usize, const B: usize> Gate<F, D> for BaseSumGate<B> {
    fn id(&self) -> String {
        format!("{:?} + Base: {}", self, B)
    }

    fn eval_unfiltered(&self, vars: EvaluationVars<F, D>) -> Vec<F::Extension> {
        let sum = vars.local_wires[Self::WIRE_SUM];
        let reversed_sum = vars.local_wires[Self::WIRE_REVERSED_SUM];
        let mut limbs = vars.local_wires[self.limbs()].to_vec();
        let computed_sum = reduce_with_powers(&limbs, F::Extension::from_canonical_usize(B));
        limbs.reverse();
        let computed_reversed_sum =
            reduce_with_powers(&limbs, F::Extension::from_canonical_usize(B));
        let mut constraints = vec![computed_sum - sum, computed_reversed_sum - reversed_sum];
        for limb in limbs {
            constraints.push(
                (0..B)
                    .map(|i| limb - F::Extension::from_canonical_usize(i))
                    .product(),
            );
        }
        constraints
    }

    fn eval_unfiltered_recursively(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: EvaluationTargets<D>,
    ) -> Vec<ExtensionTarget<D>> {
        let base = builder.constant(F::from_canonical_usize(B));
        let sum = vars.local_wires[Self::WIRE_SUM];
        let reversed_sum = vars.local_wires[Self::WIRE_REVERSED_SUM];
        let mut limbs = vars.local_wires[self.limbs()].to_vec();
        let computed_sum = reduce_with_powers_recursive(builder, &limbs, base);
        limbs.reverse();
        let reversed_computed_sum = reduce_with_powers_recursive(builder, &limbs, base);
        let mut constraints = vec![
            builder.sub_extension(computed_sum, sum),
            builder.sub_extension(reversed_computed_sum, reversed_sum),
        ];
        for limb in limbs {
            constraints.push({
                let mut acc = builder.one_extension();
                (0..B).for_each(|i| {
                    let it = builder.constant_extension(F::from_canonical_usize(i).into());
                    let diff = builder.sub_extension(limb, it);
                    acc = builder.mul_extension(acc, diff);
                });
                acc
            });
        }
        constraints
    }

    fn generators(
        &self,
        gate_index: usize,
        _local_constants: &[F],
    ) -> Vec<Box<dyn WitnessGenerator<F>>> {
        let gen = BaseSplitGenerator::<B> {
            gate_index,
            num_limbs: self.num_limbs,
        };
        vec![Box::new(gen)]
    }

    // 2 for the sum and reversed sum, then `num_limbs` for the limbs.
    fn num_wires(&self) -> usize {
        self.num_limbs + 2
    }

    fn num_constants(&self) -> usize {
        0
    }

    // Bounded by the range-check (x-0)*(x-1)*...*(x-B+1).
    fn degree(&self) -> usize {
        B
    }

    // 2 for checking the sum and reversed sum, then `num_limbs` for range-checking the limbs.
    fn num_constraints(&self) -> usize {
        2 + self.num_limbs
    }
}

#[derive(Debug)]
pub struct BaseSplitGenerator<const B: usize> {
    gate_index: usize,
    num_limbs: usize,
}

impl<F: Field, const B: usize> SimpleGenerator<F> for BaseSplitGenerator<B> {
    fn dependencies(&self) -> Vec<Target> {
        vec![Target::wire(self.gate_index, BaseSumGate::<B>::WIRE_SUM)]
    }

    fn run_once(&self, witness: &PartialWitness<F>) -> PartialWitness<F> {
        let sum_value = witness
            .get_target(Target::wire(self.gate_index, BaseSumGate::<B>::WIRE_SUM))
            .to_canonical_u64() as usize;
        debug_assert_eq!(
            (0..self.num_limbs).fold(sum_value, |acc, _| acc / B),
            0,
            "Integer too large to fit in given number of limbs"
        );

        let limbs = (BaseSumGate::<B>::WIRE_LIMBS_START
            ..BaseSumGate::<B>::WIRE_LIMBS_START + self.num_limbs)
            .map(|i| Target::wire(self.gate_index, i));
        let limbs_value = (0..self.num_limbs)
            .scan(sum_value, |acc, _| {
                let tmp = *acc % B;
                *acc /= B;
                Some(tmp)
            })
            .collect::<Vec<_>>();

        let reversed_sum = limbs_value.iter().rev().fold(0, |acc, &x| acc * B + x);

        let mut result = PartialWitness::new();
        result.set_target(
            Target::wire(self.gate_index, BaseSumGate::<B>::WIRE_REVERSED_SUM),
            F::from_canonical_usize(reversed_sum),
        );
        for (b, b_value) in limbs.zip(limbs_value) {
            result.set_target(b, F::from_canonical_usize(b_value));
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use crate::circuit_data::CircuitConfig;
    use crate::field::crandall_field::CrandallField;
    use crate::gates::base_sum::BaseSumGate;
    use crate::gates::gate_testing::test_low_degree;

    #[test]
    fn low_degree() {
        test_low_degree(BaseSumGate::<6>::new::<CrandallField, 4>(11))
    }
}
