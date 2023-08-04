//! Shift right instruction
//!
//!
//! a << (b + c) = (a << b) << c
//! a << (b + 2^i c) = (a << b) << 2^i c = ((a << b) << c) << 2^(i-1) c

use crate::chip::bool::SelectInstruction;
use crate::chip::builder::AirBuilder;
use crate::chip::register::array::ArrayRegister;
use crate::chip::register::bit::BitRegister;
use crate::chip::AirParameters;
pub use crate::math::prelude::*;

impl<L: AirParameters> AirBuilder<L> {
    pub fn shr(
        &mut self,
        a: &ArrayRegister<BitRegister>,
        b: &ArrayRegister<BitRegister>,
    ) -> ArrayRegister<BitRegister>
    where
        L::Instruction: From<SelectInstruction<BitRegister>>,
    {
        let result = self.alloc_array::<BitRegister>(a.len());
        self.set_shr(a, b, &result);
        result
    }

    pub fn set_shr(
        &mut self,
        a: &ArrayRegister<BitRegister>,
        b: &ArrayRegister<BitRegister>,
        result: &ArrayRegister<BitRegister>,
    ) where
        L::Instruction: From<SelectInstruction<BitRegister>>,
    {
        let n = a.len();
        let m = b.len();
        assert!(m <= n, "b must be shorter or eual length to a");

        let mut temp = *a;
        for (k, bit) in b.into_iter().enumerate() {
            // Calculate the shift (intermediate value << 2^k)
            let num_shift_bits = 1 << k;

            let res = if k == m - 1 {
                *result
            } else {
                self.alloc_array::<BitRegister>(n)
            };

            // For i< NUM_BITS - num_shift_bits, we have shifted_res[i] = temp[i + num_shift_bits]
            for i in 0..(n - num_shift_bits) {
                self.set_select(
                    &bit,
                    &temp.get(i + num_shift_bits),
                    &temp.get(i),
                    &res.get(i),
                );
            }

            // For i >= NUM_BITS - num_shift_bits, we have shifted_res[i] = 0
            for i in (n - num_shift_bits)..n {
                self.set_select(&bit, &bit, &temp.get(i), &res.get(i));
            }
            temp = res;
        }
    }
}

#[cfg(test)]
pub mod tests {

    use rand::{thread_rng, Rng};

    use super::*;
    use crate::chip::bool::SelectInstruction;
    pub use crate::chip::builder::tests::*;
    use crate::chip::builder::AirBuilder;
    use crate::chip::AirParameters;

    #[derive(Debug, Clone)]
    pub struct ShrTest<const N: usize, const M: usize>;

    impl<const N: usize, const M: usize> const AirParameters for ShrTest<N, M> {
        type Field = GoldilocksField;
        type CubicParams = GoldilocksCubicParameters;

        type Instruction = SelectInstruction<BitRegister>;

        const NUM_FREE_COLUMNS: usize = 2 * N + M * N + N;

        fn num_rows_bits() -> usize {
            9
        }
    }

    #[test]
    fn test_shr() {
        type F = GoldilocksField;
        type L = ShrTest<N, LOG_N>;
        const LOG_N: usize = 3;
        const N: usize = 8;
        type SC = PoseidonGoldilocksStarkConfig;

        let mut builder = AirBuilder::<L>::new();

        let a = builder.alloc_array::<BitRegister>(N);
        let b = builder.alloc_array::<BitRegister>(LOG_N);
        let result = builder.shr(&a, &b);
        let expected = builder.alloc_array::<BitRegister>(N);

        builder.assert_expressions_equal(result.expr(), expected.expr());

        let air = builder.build();

        let generator = ArithmeticGenerator::<L>::new(&air);
        let writer = generator.new_writer();

        let mut rng = thread_rng();

        let to_bits_le = |x: u8| {
            let mut bits = [0u8; 8];
            for i in 0..8 {
                bits[i] = (x >> i) & 1;
            }
            bits
        };

        let to_val = |bits: &[u8]| bits.iter().enumerate().map(|(i, b)| b << i).sum::<u8>();
        for i in 0..L::num_rows() {
            let a_val = rng.gen::<u8>();
            let a_bits = to_bits_le(a_val);
            let b_bits = [0, 0, 0];
            let b_val = to_val(&b_bits);
            assert_eq!(a_val, to_val(&a_bits));
            let expected_val = a_val >> b_val;
            let expected_bits = to_bits_le(expected_val);
            writer.write_array(&a, a_bits.map(|a| F::from_canonical_u8(a)), i);
            writer.write_array(&b, b_bits.map(|b| F::from_canonical_u8(b)), i);
            writer.write_array(&expected, expected_bits.map(|b| F::from_canonical_u8(b)), i);
            writer.write_row_instructions(&air, i);
        }

        let trace = generator.trace_clone();

        for window in trace.windows_iter() {
            let mut window_parser = TraceWindowParser::new(window, &[], &[], &[]);
            air.eval(&mut window_parser);
        }

        let stark = Starky::<_, { L::num_columns() }>::new(air);
        let config = SC::standard_fast_config(L::num_rows());

        // Generate proof and verify as a stark
        test_starky(&stark, &config, &generator, &[]);

        // Test the recursive proof.
        test_recursive_starky(stark, config, generator, &[]);
    }
}
