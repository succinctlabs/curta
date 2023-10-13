use core::marker::PhantomData;

use plonky2::field::goldilocks_field::GoldilocksField;
use serde::{Deserialize, Serialize};

use crate::chip::uint::operations::instruction::UintInstruction;
use crate::chip::AirParameters;
use crate::math::goldilocks::cubic::GoldilocksCubicParameters;
use crate::math::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByteParameters<F, E>(pub PhantomData<(F, E)>);

impl<F: PrimeField64, E: CubicParameters<F>> AirParameters for ByteParameters<F, E> {
    type Field = GoldilocksField;
    type CubicParams = GoldilocksCubicParameters;

    type Instruction = UintInstruction;

    const NUM_ARITHMETIC_COLUMNS: usize = 0;
    const NUM_FREE_COLUMNS: usize = 107;
    const EXTENDED_COLUMNS: usize = 21;
}
