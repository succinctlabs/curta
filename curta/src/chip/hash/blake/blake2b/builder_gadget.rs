use core::marker::PhantomData;

use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use serde::{Deserialize, Serialize};

use super::generator::BLAKE2BHintGenerator;
use crate::chip::hash::CurtaBytes;
use crate::math::prelude::CubicParameters;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BLAKE2BBuilderGadget<F, E, const D: usize> {
    pub padded_message: Vec<Target>,
    pub digest: Vec<Target>,
    _marker: PhantomData<(F, E)>,
}

pub trait BLAKE2BBuilder<F: RichField + Extendable<D>, E: CubicParameters<F>, const D: usize> {
    type Gadget;

    fn init_blake2b(&mut self) -> Self::Gadget;

    fn blake2b<const N: usize>(
        &mut self,
        padded_message: &CurtaBytes<N>,
        message_len: Target,
        gadget: &mut Self::Gadget,
    ) -> CurtaBytes<32>;
}

impl<F: RichField + Extendable<D>, E: CubicParameters<F>, const D: usize> BLAKE2BBuilder<F, E, D>
    for CircuitBuilder<F, D>
{
    type Gadget = BLAKE2BBuilderGadget<F, E, D>;

    fn init_blake2b(&mut self) -> Self::Gadget {
        BLAKE2BBuilderGadget {
            padded_message: Vec::new(),
            digest: Vec::new(),
            _marker: PhantomData,
        }
    }

    fn blake2b<const N: usize>(
        &mut self,
        padded_message: &CurtaBytes<N>,
        message_len: Target,
        gadget: &mut Self::Gadget,
    ) -> CurtaBytes<32> {
        gadget.padded_message.extend_from_slice(&padded_message.0);
        let digest_bytes = self.add_virtual_target_arr::<32>();
        let hint = BLAKE2BHintGenerator::new(&padded_message.0, message_len, digest_bytes);
        self.add_simple_generator(hint);
        gadget.digest.extend_from_slice(&digest_bytes);
        CurtaBytes(digest_bytes)
    }
}

#[cfg(test)]
mod tests {

    use plonky2::field::types::Field;
    use plonky2::iop::witness::{PartialWitness, WitnessWrite};
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::PoseidonGoldilocksConfig;
    use plonky2::timed;
    use plonky2::util::timing::TimingTree;

    use super::*;
    pub use crate::chip::builder::tests::*;
    use crate::chip::hash::blake::blake2b::BLAKE2BGadget;

    #[test]
    fn test_blake_2b_plonky_gadget() {
        type F = GoldilocksField;
        type E = GoldilocksCubicParameters;
        type C = PoseidonGoldilocksConfig;
        const D: usize = 2;

        let _ = env_logger::builder().is_test(true).try_init();

        let mut timing = TimingTree::new("Blake2b Plonky2 gadget test", log::Level::Debug);

        let config = CircuitConfig::standard_recursion_config();
        let mut builder = CircuitBuilder::<F, D>::new(config);

        let mut gadget: BLAKE2BBuilderGadget<F, E, D> = builder.init_blake2b();

        let msg_target = CurtaBytes(builder.add_virtual_target_arr::<256>());
        let msg_length_target = builder.add_virtual_target();

        let calculated_digest = builder.blake2b(&msg_target, msg_length_target, &mut gadget);
        let expected_digest_target = CurtaBytes(builder.add_virtual_target_arr::<32>());

        for (d, e) in calculated_digest
            .0
            .iter()
            .zip(expected_digest_target.0.iter())
        {
            builder.connect(*d, *e);
        }

        //builder.constrain_blake2b_gadget::<C>(gadget);

        let data = builder.build::<C>();
        let mut pw = PartialWitness::new();

        /*
        let msg = decode("").unwrap();
        let digest = "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8";

        let msg = b"abc".to_vec();
        let digest = "bddd813c634239723171ef3fee98579b94964e3bb1cb3e427262c8c068d52319";

        let msg = b"243f6a8885a308d313198a2e03707344a4093822299f31d0082efa98ec4e6c89452821e638d01377be5466cf34e90c6cc0ac29b7c97c50dd3f84d5b5b5470917".to_vec();
        let digest = "486ce0fdbd0e2f6b798d1ef3d881585b7a3331802a995d4b7fdf886b8b03a9a4";
        */

        let msg = b"243f6a8885a308d313198a2e03707344a4093822299f31d0082efa98ec4e6c89452821e638d01377be5466cf34e90c6cc0ac29b7c97c50dd3f84d5b5b5470917243f6a8885a308d313198a2e03707344a4093822299f31d0082efa98ec4e6c89452821e638d01377be5466cf34e90c6cc0ac29b7c97c50dd3f84d5b5b5470917".to_vec();
        let digest = "369ffcc61c51d8ed04bf30a9e8cf79f8994784d1e3f90f32c3182e67873a3238";

        let padded_msg = BLAKE2BGadget::pad(&msg)
            .into_iter()
            .map(F::from_canonical_u8)
            .collect::<Vec<_>>();

        let expected_digest = hex::decode(digest)
            .unwrap()
            .into_iter()
            .map(F::from_canonical_u8)
            .collect::<Vec<_>>();

        pw.set_target_arr(&msg_target.0, &padded_msg);
        pw.set_target(msg_length_target, F::from_canonical_usize(msg.len()));
        pw.set_target_arr(&expected_digest_target.0, &expected_digest);

        let proof = timed!(
            timing,
            "Generate proof",
            plonky2::plonk::prover::prove(&data.prover_only, &data.common, pw, &mut timing)
        )
        .unwrap();
        timing.print();
        data.verify(proof).unwrap();
    }
}
