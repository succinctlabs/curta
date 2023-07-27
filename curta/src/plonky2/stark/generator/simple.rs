use core::fmt::Debug;

use plonky2::field::extension::Extendable;
use plonky2::field::packable::Packable;
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::generator::{GeneratedValues, SimpleGenerator};
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartitionWitness, Witness};
use plonky2::plonk::circuit_data::CommonCircuitData;
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::util::serialization::IoResult;

use super::super::config::StarkyConfig;
use super::super::proof::StarkProofTarget;
use super::super::prover::StarkyProver;
use super::super::verifier::set_stark_proof_target;
use crate::air::RAir;
use crate::plonky2::parser::StarkParser;
use crate::plonky2::stark::Starky;
use crate::trace::generator::TraceGenerator;

#[derive(Debug, Clone)]
pub struct SimpleStarkWitnessGenerator<A, T: Clone, F, C, P, const D: usize, const COLUMNS: usize> {
    config: StarkyConfig<F, C, D>,
    pub stark: Starky<A, COLUMNS>,
    pub proof_target: StarkProofTarget<D>,
    pub public_input_targets: Vec<Target>,
    pub trace_generator: T,
    _marker: core::marker::PhantomData<P>,
}

impl<A, T: Clone, F: RichField, C, const D: usize, const COLUMNS: usize>
    SimpleStarkWitnessGenerator<A, T, F, C, <F as Packable>::Packing, D, COLUMNS>
{
    pub fn new(
        config: StarkyConfig<F, C, D>,
        stark: Starky<A, COLUMNS>,
        proof_target: StarkProofTarget<D>,
        public_input_targets: Vec<Target>,
        trace_generator: T,
    ) -> Self {
        Self {
            config,
            stark,
            proof_target,
            public_input_targets,
            trace_generator,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<A: 'static + Debug + Send + Sync, T: Clone, F, C, P, const D: usize, const COLUMNS: usize>
    SimpleGenerator<F, D> for SimpleStarkWitnessGenerator<A, T, F, C, P, D, COLUMNS>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F> + 'static,
    C::Hasher: AlgebraicHasher<F>,
    P: PackedField<Scalar = F>,
    A: for<'a> RAir<StarkParser<'a, F, F, P, D, 1>>,
    T: Debug + Send + Sync + 'static + TraceGenerator<F, A>,
    T::Error: Into<anyhow::Error>,
{
    fn id(&self) -> String {
        "SimpleStarkWitnessGenerator".to_string()
    }

    fn dependencies(&self) -> Vec<Target> {
        self.public_input_targets.clone()
    }

    fn run_once(&self, witness: &PartitionWitness<F>, out_buffer: &mut GeneratedValues<F>) {
        let public_inputs = witness.get_targets(&self.public_input_targets);

        let proof = StarkyProver::<F, C, F, P, D, 1>::prove(
            &self.config,
            &self.stark,
            &self.trace_generator,
            &public_inputs,
        )
        .unwrap();

        set_stark_proof_target(out_buffer, &self.proof_target, &proof);
    }

    fn serialize(
        &self,
        _dst: &mut Vec<u8>,
        _common_data: &CommonCircuitData<F, D>,
    ) -> IoResult<()> {
        unimplemented!("SimpleStarkWitnessGenerator::serialize")
    }

    fn deserialize(
        _src: &mut plonky2::util::serialization::Buffer,
        _common_data: &CommonCircuitData<F, D>,
    ) -> IoResult<Self>
    where
        Self: Sized,
    {
        unimplemented!("SimpleStarkWitnessGenerator::deserialize")
    }
}
