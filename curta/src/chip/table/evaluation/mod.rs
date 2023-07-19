use core::marker::PhantomData;

use crate::chip::builder::AirBuilder;
use crate::chip::constraint::arithmetic::expression::ArithmeticExpression;
use crate::chip::constraint::Constraint;
use crate::chip::register::array::ArrayRegister;
use crate::chip::register::element::ElementRegister;
use crate::chip::register::extension::ExtensionRegister;
use crate::chip::register::{Register, RegisterSerializable};
use crate::chip::AirParameters;
use crate::math::prelude::*;

pub mod constraint;
pub mod trace;

#[derive(Debug, Clone)]
pub enum Digest<F: Field, E: CubicParameters<F>> {
    Values(Vec<ArrayRegister<ElementRegister>>),
    Extended(ExtensionRegister<3>),
    Expression(ArithmeticExpression<F>),
    None,
    _Marker(PhantomData<E>),
}

#[derive(Debug, Clone)]
pub struct Evaluation<F: Field, E: CubicParameters<F>> {
    pub beta: ExtensionRegister<3>,
    beta_powers: ExtensionRegister<3>,
    pub alphas: ArrayRegister<ExtensionRegister<3>>,
    pub values: Vec<ElementRegister>,
    pub filter: ArithmeticExpression<F>,
    accumulator: ExtensionRegister<3>,
    row_accumulator: ExtensionRegister<3>,
    pub digest: Digest<F, E>,
    _marker: PhantomData<(F, E)>,
}

impl<L: AirParameters> AirBuilder<L> {
    pub fn alloc_digest_column(&mut self) -> Digest<L::Field, L::CubicParams> {
        Digest::Extended(self.alloc_extended::<ExtensionRegister<3>>())
    }

    pub fn evaluation<T: Register>(
        &mut self,
        values: &[T],
        filter: ArithmeticExpression<L::Field>,
        digest: Digest<L::Field, L::CubicParams>,
    ) -> Evaluation<L::Field, L::CubicParams> {
        // Get the running evaluation challenge
        let beta = self.alloc_challenge::<ExtensionRegister<3>>();
        let beta_powers = self.alloc_extended::<ExtensionRegister<3>>();
        // get the row accumulation challenge
        let alphas =
            self.alloc_challenge_array::<ExtensionRegister<3>>(values.len() * T::size_of());

        let mut elem_vals = vec![];
        for val in values {
            let elem_array =
                ArrayRegister::<ElementRegister>::from_register_unsafe(*val.register());
            for e in elem_array.into_iter() {
                elem_vals.push(e);
            }
        }

        let row_accumulator = self.alloc_extended::<ExtensionRegister<3>>();
        let accumulator = self.alloc_extended::<ExtensionRegister<3>>();

        let evaluation = Evaluation {
            beta,
            beta_powers,
            alphas,
            values: elem_vals,
            filter,
            row_accumulator,
            accumulator,
            digest,
            _marker: PhantomData,
        };
        self.constraints
            .push(Constraint::evaluation(evaluation.clone()));
        self.evaluation_data.push(evaluation.clone());
        evaluation
    }
}

impl<F: Field, E: CubicParameters<F>> Digest<F, E> {
    pub fn none() -> Self {
        Digest::None
    }

    pub fn from_expression(expression: ArithmeticExpression<F>) -> Self {
        Digest::Expression(expression)
    }

    pub fn from_values(values: &[ArrayRegister<ElementRegister>]) -> Self {
        Digest::Values(values.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};

    use super::*;
    use crate::chip::{builder::tests::*, register::bit::BitRegister};

    #[derive(Debug, Clone)]
    pub struct EvalTest;

    impl const AirParameters for EvalTest {
        type Field = GoldilocksField;
        type CubicParams = GoldilocksCubicParameters;
        type Instruction = EmptyInstruction<GoldilocksField>;
        const NUM_ARITHMETIC_COLUMNS: usize = 2;
        const NUM_FREE_COLUMNS: usize = 3;
        const EXTENDED_COLUMNS: usize = 23;

        fn num_rows_bits() -> usize {
            16
        }
    }

    #[test]
    fn test_evaluation() {
        type F = GoldilocksField;
        type L = EvalTest;
        type SC = PoseidonGoldilocksStarkConfig;

        let mut builder = AirBuilder::<L>::new();
        let x_0 = builder.alloc::<U16Register>();
        let x_1 = builder.alloc::<U16Register>();

        let cycle = builder.cycle(10);

        let bit = builder.alloc::<BitRegister>();

        let acc = builder.alloc_digest_column();

        let _eval = builder.evaluation(&[x_0, x_1], bit.expr(), acc);

        let (air, _) = builder.build();

        let generator = ArithmeticGenerator::<L>::new(&[]);

        let (tx, rx) = channel();
        for i in 0..L::num_rows() {
            let writer = generator.new_writer();
            let handle = tx.clone();
            writer.write_instruction(&cycle, i);
            rayon::spawn(move || {
                let mut rng = thread_rng();
                let bit_val = rng.gen_bool(0.5);
                writer.write(&bit, &[F::from_canonical_u32(bit_val as u32)], i);
                writer.write(&x_0, &[F::ONE], i);
                writer.write(&x_1, &[F::from_canonical_usize(i)], i);
                handle.send(1).unwrap();
            });
        }
        drop(tx);
        for msg in rx.iter() {
            assert!(msg == 1);
        }
        let stark = Starky::<_, { L::num_columns() }>::new(air);
        let config = SC::standard_fast_config(L::num_rows());

        // Generate proof and verify as a stark
        test_starky(&stark, &config, &generator, &[]);

        // Test the recursive proof.
        test_recursive_starky(stark, config, generator, &[]);
    }
}
