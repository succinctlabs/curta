use anyhow::Result;

use super::den::Den;
use super::*;
use crate::arithmetic::builder::ChipBuilder;
use crate::arithmetic::chip::ChipParameters;
use crate::arithmetic::field::mul::{FpMul, FpMulConst};
use crate::arithmetic::field::quad::FpQuad;
use crate::arithmetic::trace::TraceHandle;

#[derive(Debug, Clone, Copy)]
#[allow(non_snake_case)]
#[allow(dead_code)]
pub struct EcAddData<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize> {
    P: PointRegister<E, N_LIMBS>,
    Q: PointRegister<E, N_LIMBS>,
    R: PointRegister<E, N_LIMBS>,
    XNUM: FpQuad<E::FieldParam, N_LIMBS>,
    YNUM: FpQuad<E::FieldParam, N_LIMBS>,
    PXPY: FpMul<E::FieldParam, N_LIMBS>,
    QXQY: FpMul<E::FieldParam, N_LIMBS>,
    PXPYQXQY: FpMul<E::FieldParam, N_LIMBS>,
    DXY: FpMulConst<E::FieldParam, N_LIMBS>,
    XDEN: Den<E::FieldParam, N_LIMBS>,
    YDEN: Den<E::FieldParam, N_LIMBS>,
}

impl<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize> EcAddData<E, N_LIMBS> {
    pub const fn num_ed_add_columns() -> usize {
        6 * N_LIMBS
            + 2 * (FpQuad::<E::FieldParam, N_LIMBS>::num_quad_columns() - 4 * N_LIMBS)
            + 3 * (FpMul::<E::FieldParam, N_LIMBS>::num_mul_columns() - 2 * N_LIMBS)
            + 2 * (FpMulConst::<E::FieldParam, N_LIMBS>::num_mul_const_columns() - N_LIMBS)
            + 2 * (Den::<E::FieldParam, N_LIMBS>::num_den_columns() - 3 * N_LIMBS)
    }
}

impl<L: ChipParameters<F, D>, F: RichField + Extendable<D>, const D: usize> ChipBuilder<L, F, D> {
    #[allow(non_snake_case)]
    pub fn ed_add<E: EdwardsParameters<N>, const N: usize>(
        &mut self,
        P: &PointRegister<E, N>,
        Q: &PointRegister<E, N>,
        result: &PointRegister<E, N>,
    ) -> Result<EcAddData<E, N>>
    where
        L::Instruction: From<FpMul<E::FieldParam, N>>
            + From<FpQuad<E::FieldParam, N>>
            + From<FpMulConst<E::FieldParam, N>>
            + From<Den<E::FieldParam, N>>,
    {
        let x_num_result = self.alloc_local::<FieldRegister<E::FieldParam, N>>()?;
        let y_num_result = self.alloc_local::<FieldRegister<E::FieldParam, N>>()?;
        let px_py_result = self.alloc_local::<FieldRegister<E::FieldParam, N>>()?;
        let qx_qy_result = self.alloc_local::<FieldRegister<E::FieldParam, N>>()?;
        let all_xy_result = self.alloc_local::<FieldRegister<E::FieldParam, N>>()?;
        let dxy_result = self.alloc_local::<FieldRegister<E::FieldParam, N>>()?;

        let x_num_ins = self.fpquad(&P.x, &Q.y, &Q.x, &P.y, &x_num_result)?;
        let y_num_ins = self.fpquad(&P.y, &Q.y, &P.x, &Q.x, &y_num_result)?;

        let px_py_ins = self.fpmul(&P.x, &P.y, &px_py_result)?;
        let qx_qy_ins = self.fpmul(&Q.x, &Q.y, &qx_qy_result)?;

        let all_xy_ins = self.fpmul(&px_py_result, &qx_qy_result, &all_xy_result)?;
        let dxy_ins = self.fpmul_const(&all_xy_result, E::D, &dxy_result)?;

        let r_x_ins = self.ed_den(&x_num_result, &dxy_result, true, &result.x)?;
        let r_y_ins = self.ed_den(&y_num_result, &dxy_result, false, &result.y)?;

        Ok(EcAddData {
            P: *P,
            Q: *Q,
            R: *result,
            XNUM: x_num_ins,
            YNUM: y_num_ins,
            PXPY: px_py_ins,
            QXQY: qx_qy_ins,
            PXPYQXQY: all_xy_ins,
            DXY: dxy_ins,
            XDEN: r_x_ins,
            YDEN: r_y_ins,
        })
    }
}

impl<F: RichField + Extendable<D>, const D: usize> TraceHandle<F, D> {
    #[allow(non_snake_case)]
    pub fn write_ed_add<E: EdwardsParameters<N_LIMBS>, const N_LIMBS: usize>(
        &self,
        row_index: usize,
        P: &PointBigint,
        Q: &PointBigint,
        chip_data: EcAddData<E, N_LIMBS>,
    ) -> Result<PointBigint> {
        let x_num = self.write_fpquad(row_index, &P.x, &Q.y, &Q.x, &P.y, chip_data.XNUM)?;
        let y_num = self.write_fpquad(row_index, &P.y, &Q.y, &P.x, &Q.x, chip_data.YNUM)?;

        let px_py = self.write_fpmul(row_index, &P.x, &P.y, chip_data.PXPY)?;
        let qx_qy = self.write_fpmul(row_index, &Q.x, &Q.y, chip_data.QXQY)?;

        let all_xy = self.write_fpmul(row_index, &px_py, &qx_qy, chip_data.PXPYQXQY)?;
        let dxy = self.write_fpmul_const(row_index, &all_xy, chip_data.DXY)?;

        let r_x = self.write_ed_den(row_index, &x_num, &dxy, true, chip_data.XDEN)?;
        let r_y = self.write_ed_den(row_index, &y_num, &dxy, false, chip_data.YDEN)?;

        Ok(PointBigint { x: r_x, y: r_y })
    }
}

#[cfg(test)]
mod tests {

    //use num::bigint::RandBigInt;
    use num::Num;
    use plonky2::iop::witness::PartialWitness;
    use plonky2::plonk::circuit_builder::CircuitBuilder;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use plonky2::util::timing::TimingTree;

    //use plonky2_maybe_rayon::*;

    //use rand::thread_rng;
    use super::*;
    use crate::arithmetic::builder::ChipBuilder;
    use crate::arithmetic::chip::{ChipParameters, TestStark};
    use crate::arithmetic::ec::edwards::instructions::EdWardsMicroInstruction;
    use crate::arithmetic::trace::trace;
    use crate::config::StarkConfig;
    use crate::prover::prove;
    use crate::recursive_verifier::{
        add_virtual_stark_proof_with_pis, set_stark_proof_with_pis_target,
        verify_stark_proof_circuit,
    };
    use crate::verifier::verify_stark_proof;

    #[derive(Clone, Debug, Copy)]
    pub struct EdAddTest;

    impl<F: RichField + Extendable<D>, const D: usize> ChipParameters<F, D> for EdAddTest {
        const NUM_ARITHMETIC_COLUMNS: usize =
            EcAddData::<Ed25519Parameters, 16>::num_ed_add_columns();
        const NUM_FREE_COLUMNS: usize = 0;

        type Instruction = EdWardsMicroInstruction<Ed25519Parameters, 16>;
    }

    #[allow(non_snake_case)]
    #[test]
    fn test_ed_add_row() {
        const D: usize = 2;
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<D>>::F;
        type E = Ed25519Parameters;
        type S = TestStark<EdAddTest, F, D>;

        // build the stark
        let mut builder = ChipBuilder::<EdAddTest, F, D>::new();

        let P = builder.alloc_local_ec_point::<E, 16>().unwrap();
        let Q = builder.alloc_local_ec_point::<E, 16>().unwrap();
        let R = builder.alloc_local_ec_point::<E, 16>().unwrap();

        let ed_data = builder.ed_add::<E, 16>(&P, &Q, &R).unwrap();
        builder.write_ec_point(&P).unwrap();
        builder.write_ec_point(&Q).unwrap();

        let (chip, spec) = builder.build();

        // Construct the trace
        // Construct the trace
        let num_rows = 2u64.pow(16);
        let (handle, generator) = trace::<F, D>(spec);

        let B_x = BigUint::from_str_radix(
            "15112221349535400772501151409588531511454012693041857206046113283949847762202",
            10,
        )
        .unwrap();
        let B_y = BigUint::from_str_radix(
            "46316835694926478169428394003475163141307993866256225615783033603165251855960",
            10,
        )
        .unwrap();

        let B = PointBigint { x: B_x, y: B_y };
        let identity = PointBigint {
            x: BigUint::from(0u32),
            y: BigUint::from(1u32),
        };

        for i in 0..num_rows {
            let P_int = B.clone();
            let Q_int = identity.clone();
            //let handle = handle.clone();
            //rayon::spawn(move || {
            handle.write_ec_point(i as usize, &P_int, &P).unwrap();
            handle.write_ec_point(i as usize, &Q_int, &Q).unwrap();
            let R = handle
                .write_ed_add(i as usize, &P_int, &Q_int, ed_data)
                .unwrap();
            //assert_eq!(R, P_int);
            //});
        }
        drop(handle);

        let trace = generator.generate_trace(&chip, num_rows as usize).unwrap();

        let config = StarkConfig::standard_fast_config();
        let stark = TestStark::new(chip);

        // Verify proof as a stark
        let proof = prove::<F, C, S, D>(
            stark.clone(),
            &config,
            trace,
            [],
            &mut TimingTree::default(),
        )
        .unwrap();
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
