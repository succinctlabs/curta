use anyhow::Result;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::challenger::{Challenger, RecursiveChallenger};
use plonky2::iop::target::Target;
use plonky2::iop::witness::WitnessWrite;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::timed;
use plonky2::util::timing::TimingTree;
use serde::{Deserialize, Serialize};

use super::builder::NUM_LOOKUP_ROWS;
use super::proof::{
    EmulatedStarkChallenges, EmulatedStarkChallengesTarget, EmulatedStarkProof,
    EmulatedStarkProofTarget,
};
use super::RangeParameters;
use crate::chip::register::array::ArrayRegister;
use crate::chip::register::element::ElementRegister;
use crate::chip::table::lookup::values::LogLookupValues;
use crate::chip::trace::data::AirTraceData;
use crate::chip::trace::writer::{InnerWriterData, TraceWriter};
use crate::chip::{AirParameters, Chip};
use crate::math::prelude::*;
use crate::maybe_rayon::*;
use crate::plonky2::stark::config::{CurtaConfig, StarkyConfig};
use crate::plonky2::stark::prover::{AirCommitment, StarkyProver};
use crate::plonky2::stark::verifier::{
    add_virtual_air_proof, set_air_proof_target, StarkyVerifier,
};
use crate::plonky2::stark::Starky;
use crate::plonky2::Plonky2Air;
use crate::trace::AirTrace;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct EmulatedStark<L: AirParameters, C, const D: usize> {
    pub config: StarkyConfig<C, D>,
    pub stark: Starky<Chip<L>>,
    pub air_data: AirTraceData<L>,
    pub(crate) lookup_config: StarkyConfig<C, D>,
    pub(crate) lookup_stark: Starky<Chip<RangeParameters<L::Field, L::CubicParams>>>,
    pub(crate) lookup_air_data: AirTraceData<RangeParameters<L::Field, L::CubicParams>>,
    pub(crate) lookup_values: LogLookupValues<ElementRegister, L::Field, L::CubicParams>,
    pub(crate) lookup_table: ElementRegister,
    pub(crate) multiplicity: ArrayRegister<ElementRegister>,
}

impl<L: AirParameters, C, const D: usize> EmulatedStark<L, C, D>
where
    L::Field: RichField + Extendable<D>,
    C: CurtaConfig<D, F = L::Field, FE = <L::Field as Extendable<D>>::Extension>,
    Chip<L>: Plonky2Air<L::Field, D>,
{
    pub const fn stark(&self) -> &Starky<Chip<L>> {
        &self.stark
    }

    pub const fn config(&self) -> &StarkyConfig<C, D> {
        &self.config
    }

    pub const fn lookup_stark(&self) -> &Starky<Chip<RangeParameters<L::Field, L::CubicParams>>> {
        &self.lookup_stark
    }

    pub const fn lookup_config(&self) -> &StarkyConfig<C, D> {
        &self.lookup_config
    }

    #[inline]
    pub fn range_fn(element: L::Field) -> (usize, usize) {
        (element.as_canonical_u64() as usize, 0)
    }

    fn generate_execution_traces(
        &self,
        execution_trace: &AirTrace<L::Field>,
        public_values: &[L::Field],
    ) -> (TraceWriter<L::Field>, TraceWriter<L::Field>) {
        // Initialize writers.
        let main_writer = TraceWriter::new(&self.air_data, execution_trace.height());
        let lookup_writer = TraceWriter::new(&self.lookup_air_data, NUM_LOOKUP_ROWS);

        // Insert execution trace and into main writer.
        let execution_trace_length = self.stark.air.execution_trace_length;
        main_writer
            .write_trace()
            .unwrap()
            .rows_par_mut()
            .zip(execution_trace.rows_par())
            .for_each(|(row, execution_row)| {
                row[0..execution_trace_length]
                    .copy_from_slice(&execution_row[0..execution_trace_length]);
            });
        // Insert public inputs into both writers.
        main_writer
            .public_mut()
            .unwrap()
            .copy_from_slice(public_values);
        lookup_writer
            .public_mut()
            .unwrap()
            .copy_from_slice(public_values);

        // Write lookup table values
        for i in 0..NUM_LOOKUP_ROWS {
            lookup_writer.write(&self.lookup_table, &L::Field::from_canonical_usize(i), i);
        }
        for i in 0..NUM_LOOKUP_ROWS {
            lookup_writer.write_row_instructions(&self.lookup_air_data, i);
        }
        // Write multiplicities
        let multiplicities = main_writer.get_multiplicities_from_fn(
            1,
            NUM_LOOKUP_ROWS,
            &self.lookup_values.trace_values,
            &self.lookup_values.public_values,
            Self::range_fn,
        );

        lookup_writer.write_lookup_multiplicities(self.multiplicity, &[multiplicities]);

        (main_writer, lookup_writer)
    }

    fn generate_extended_traces(
        &self,
        main_writer: &TraceWriter<L::Field>,
        lookup_writer: &TraceWriter<L::Field>,
    ) {
        self.air_data.write_extended_trace(main_writer);

        // Update global values
        lookup_writer
            .global
            .write()
            .unwrap()
            .copy_from_slice(&main_writer.global.read().unwrap());

        // Write the extended trace values
        self.lookup_air_data.write_extended_trace(lookup_writer);

        // Update global values
        main_writer
            .global
            .write()
            .unwrap()
            .copy_from_slice(&lookup_writer.global.read().unwrap());
    }

    fn generate_trace(
        &self,
        execution_trace: &AirTrace<L::Field>,
        public_values: &[L::Field],
        challenger: &mut Challenger<L::Field, C::Hasher>,
        timing: &mut TimingTree,
    ) -> (AirCommitment<L::Field, C, D>, AirCommitment<L::Field, C, D>) {
        // Absorve public values into the challenger.
        challenger.observe_elements(public_values);

        // Generate execution traces.
        let (main_writer, lookup_writer) =
            self.generate_execution_traces(execution_trace, public_values);

        let main_execution_trace_values = main_writer
            .read_trace()
            .unwrap()
            .rows_par()
            .flat_map(|row| row[0..self.stark.air.execution_trace_length].to_vec())
            .collect::<Vec<_>>();
        let main_execution_trace = AirTrace {
            values: main_execution_trace_values,
            width: self.stark.air.execution_trace_length,
        };

        let lookup_execution_trace_values = lookup_writer
            .read_trace()
            .unwrap()
            .rows_par()
            .flat_map(|row| row[0..self.lookup_stark.air.execution_trace_length].to_vec())
            .collect::<Vec<_>>();

        let lookup_execution_trace = AirTrace {
            values: lookup_execution_trace_values,
            width: self.lookup_stark.air.execution_trace_length,
        };

        // Commit to execution traces
        let main_execution_commitment = timed!(
            timing,
            "Commit to execution trace",
            self.config.commit(&main_execution_trace, timing)
        );

        let lookup_execution_commitment = timed!(
            timing,
            "Commit to lookup execution trace",
            self.lookup_config.commit(&lookup_execution_trace, timing)
        );

        // Absorve the trace commitments into the challenger.
        challenger.observe_cap(&main_execution_commitment.merkle_tree.cap);
        challenger.observe_cap(&lookup_execution_commitment.merkle_tree.cap);

        // Get random AIR challenges.
        let challenges = challenger.get_n_challenges(self.stark.air.num_challenges);
        // Save challenges to both writers.
        main_writer
            .challenges
            .write()
            .unwrap()
            .extend_from_slice(&challenges);
        lookup_writer
            .challenges
            .write()
            .unwrap()
            .extend_from_slice(&challenges);

        // Generate extended traces.
        self.generate_extended_traces(&main_writer, &lookup_writer);

        let InnerWriterData {
            trace: main_trace,
            public: main_public,
            global: main_global,
            challenges: main_challenges,
            ..
        } = main_writer.into_inner().unwrap();
        let InnerWriterData {
            trace: lookup_trace,
            public: lookup_public,
            global: lookup_global,
            challenges: global_challenges,
            ..
        } = lookup_writer.into_inner().unwrap();

        // Commit to extended traces.
        let main_extended_trace_values = main_trace
            .rows_par()
            .flat_map(|row| row[self.stark.air.execution_trace_length..].to_vec())
            .collect::<Vec<_>>();
        let main_extended_trace = AirTrace {
            values: main_extended_trace_values,
            width: L::num_columns() - self.stark.air.execution_trace_length,
        };
        let main_extended_commitment = timed!(
            timing,
            "Commit to extended trace",
            self.config.commit(&main_extended_trace, timing)
        );

        let lookup_extended_trace_values = lookup_trace
            .rows_par()
            .flat_map(|row| row[self.lookup_stark.air.execution_trace_length..].to_vec())
            .collect::<Vec<_>>();
        let lookup_extended_trace = AirTrace {
            values: lookup_extended_trace_values,
            width: RangeParameters::<L::Field, L::CubicParams>::num_columns()
                - self.lookup_stark.air.execution_trace_length,
        };
        let lookup_extended_commitment = timed!(
            timing,
            "Commit to lookup extended trace",
            self.lookup_config.commit(&lookup_extended_trace, timing)
        );

        // Obsderve global values.
        challenger.observe_elements(&main_global);
        // Observe extended trace commitments.
        challenger.observe_cap(&main_extended_commitment.merkle_tree.cap);
        challenger.observe_cap(&lookup_extended_commitment.merkle_tree.cap);

        // Return the air commitments.
        (
            AirCommitment {
                trace_commitments: vec![main_execution_commitment, main_extended_commitment],
                public_inputs: main_public,
                global_values: main_global,
                challenges: main_challenges,
            },
            AirCommitment {
                trace_commitments: vec![lookup_execution_commitment, lookup_extended_commitment],
                public_inputs: lookup_public,
                global_values: lookup_global,
                challenges: global_challenges,
            },
        )
    }

    pub fn prove(
        &self,
        execution_trace: &AirTrace<L::Field>,
        public_values: &[L::Field],
        timing: &mut TimingTree,
    ) -> Result<EmulatedStarkProof<L::Field, C, D>> {
        // Initialize challenger.
        let mut challenger = Challenger::new();

        // Generate stark commitment.
        let (main_air_commitment, lookup_air_commitment) = timed!(
            timing,
            "Generate stark trace",
            self.generate_trace(execution_trace, public_values, &mut challenger, timing)
        );

        // Generate individual stark proofs.
        let main_proof = timed!(
            timing,
            "Generate main proof",
            StarkyProver::prove_with_trace(
                &self.config,
                &self.stark,
                main_air_commitment,
                &mut challenger,
                &mut TimingTree::default(),
            )?
        );

        let lookup_proof = timed!(
            timing,
            "Generate lookup proof",
            StarkyProver::prove_with_trace(
                &self.lookup_config,
                &self.lookup_stark,
                lookup_air_commitment,
                &mut challenger,
                &mut TimingTree::default(),
            )?
        );

        // Return the proof.
        Ok(EmulatedStarkProof {
            main_proof: main_proof.air_proof,
            lookup_proof: lookup_proof.air_proof,
            global_values: lookup_proof.global_values,
        })
    }

    pub fn get_challenges(
        &self,
        proof: &EmulatedStarkProof<L::Field, C, D>,
        public_values: &[L::Field],
    ) -> EmulatedStarkChallenges<L::Field, D> {
        // Initialize challenger.
        let mut challenger = Challenger::<L::Field, C::Hasher>::new();

        // Observe public values.
        challenger.observe_elements(public_values);

        // Observe execution trace commitments.
        challenger.observe_cap(&proof.main_proof.trace_caps[0]);
        challenger.observe_cap(&proof.lookup_proof.trace_caps[0]);

        // Get challenges.
        let challenges = challenger.get_n_challenges(self.stark.air.num_challenges);

        // Observe global values.
        challenger.observe_elements(&proof.global_values);
        // Observe extended trace commitments.
        challenger.observe_cap(&proof.main_proof.trace_caps[1]);
        challenger.observe_cap(&proof.lookup_proof.trace_caps[1]);

        // Get all challenges.
        let main_challenges = proof.main_proof.get_iop_challenges(
            &self.config,
            self.config.degree_bits,
            challenges.clone(),
            &mut challenger,
        );
        let lookup_challenges = proof.lookup_proof.get_iop_challenges(
            &self.lookup_config,
            self.lookup_config.degree_bits,
            challenges,
            &mut challenger,
        );

        EmulatedStarkChallenges {
            main_challenges,
            lookup_challenges,
        }
    }

    pub fn verify(
        &self,
        proof: EmulatedStarkProof<L::Field, C, D>,
        public_values: &[L::Field],
    ) -> Result<()> {
        let EmulatedStarkChallenges {
            main_challenges,
            lookup_challenges,
        } = self.get_challenges(&proof, public_values);

        let EmulatedStarkProof {
            main_proof,
            lookup_proof,
            global_values,
        } = proof;

        StarkyVerifier::verify_with_challenges(
            &self.config,
            &self.stark,
            main_proof,
            public_values,
            &global_values,
            main_challenges,
        )?;
        StarkyVerifier::verify_with_challenges(
            &self.lookup_config,
            &self.lookup_stark,
            lookup_proof,
            public_values,
            &global_values,
            lookup_challenges,
        )
    }

    pub fn add_virtual_proof_with_pis_target(
        &self,
        builder: &mut CircuitBuilder<L::Field, D>,
    ) -> (EmulatedStarkProofTarget<D>, Vec<Target>) {
        let main_proof = add_virtual_air_proof(builder, &self.stark, &self.config);
        let lookup_proof = add_virtual_air_proof(builder, &self.lookup_stark, &self.lookup_config);

        let num_global_values = self.stark.air.num_global_values;
        let global_values = builder.add_virtual_targets(num_global_values);
        let public_inputs = builder.add_virtual_targets(self.stark.air.num_public_values);

        (
            EmulatedStarkProofTarget {
                main_proof,
                lookup_proof,
                global_values,
            },
            public_inputs,
        )
    }

    pub fn get_challenges_target(
        &self,
        builder: &mut CircuitBuilder<L::Field, D>,
        proof: &EmulatedStarkProofTarget<D>,
        public_values: &[Target],
    ) -> EmulatedStarkChallengesTarget<D> {
        // Initialize challenger.
        let mut challenger = RecursiveChallenger::<L::Field, C::InnerHasher, D>::new(builder);

        // Observe public values.
        challenger.observe_elements(public_values);

        // Observe execution trace commitments.
        challenger.observe_cap(&proof.main_proof.trace_caps[0]);
        challenger.observe_cap(&proof.lookup_proof.trace_caps[0]);

        // Get challenges.
        let challenges = challenger.get_n_challenges(builder, self.stark.air.num_challenges);

        // Observe global values.
        challenger.observe_elements(&proof.global_values);
        // Observe extended trace commitments.
        challenger.observe_cap(&proof.main_proof.trace_caps[1]);
        challenger.observe_cap(&proof.lookup_proof.trace_caps[1]);

        // Get all challenges.
        let main_challenges = proof.main_proof.get_iop_challenges_target(
            builder,
            &self.config,
            challenges.clone(),
            &mut challenger,
        );
        let lookup_challenges = proof.lookup_proof.get_iop_challenges_target(
            builder,
            &self.lookup_config,
            challenges,
            &mut challenger,
        );

        EmulatedStarkChallengesTarget {
            main_challenges,
            lookup_challenges,
        }
    }

    pub fn verify_circuit(
        &self,
        builder: &mut CircuitBuilder<L::Field, D>,
        proof: &EmulatedStarkProofTarget<D>,
        public_values: &[Target],
    ) {
        let challenges = self.get_challenges_target(builder, proof, public_values);
        let EmulatedStarkProofTarget {
            main_proof,
            lookup_proof,
            global_values,
        } = proof;

        StarkyVerifier::verify_with_challenges_circuit(
            builder,
            &self.config,
            &self.stark,
            main_proof,
            public_values,
            global_values,
            challenges.main_challenges,
        );

        StarkyVerifier::verify_with_challenges_circuit(
            builder,
            &self.lookup_config,
            &self.lookup_stark,
            lookup_proof,
            public_values,
            global_values,
            challenges.lookup_challenges,
        )
    }

    pub fn set_proof_target<W: WitnessWrite<L::Field>>(
        &self,
        witness: &mut W,
        proof_tagret: &EmulatedStarkProofTarget<D>,
        proof: EmulatedStarkProof<L::Field, C, D>,
    ) {
        let EmulatedStarkProofTarget {
            main_proof,
            lookup_proof,
            global_values,
        } = proof_tagret;

        set_air_proof_target(witness, main_proof, &proof.main_proof);
        set_air_proof_target(witness, lookup_proof, &proof.lookup_proof);

        witness.set_target_arr(global_values, &proof.global_values);
    }
}

#[cfg(test)]
mod tests {
    use num::bigint::RandBigInt;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::iop::witness::{PartialWitness, WitnessWrite};
    use plonky2::plonk::circuit_data::CircuitConfig;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::chip::field::instruction::FpInstruction;
    use crate::chip::field::parameters::tests::Fp25519;
    use crate::chip::field::parameters::FieldParameters;
    use crate::chip::field::register::FieldRegister;
    use crate::chip::trace::writer::data::AirWriterData;
    use crate::chip::trace::writer::AirWriter;
    use crate::machine::builder::Builder;
    use crate::machine::emulated::builder::EmulatedBuilder;
    use crate::math::goldilocks::cubic::GoldilocksCubicParameters;
    use crate::plonky2::stark::config::CurtaPoseidonGoldilocksConfig;
    use crate::polynomial::Polynomial;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RangeTest;

    impl AirParameters for RangeTest {
        type Field = GoldilocksField;
        type CubicParams = GoldilocksCubicParameters;

        type Instruction = FpInstruction<Fp25519>;

        const NUM_ARITHMETIC_COLUMNS: usize = 124;
        const NUM_FREE_COLUMNS: usize = 1;
        const EXTENDED_COLUMNS: usize = 192;
    }

    #[test]
    fn test_fp_multi_stark() {
        type L = RangeTest;
        type F = GoldilocksField;
        type C = CurtaPoseidonGoldilocksConfig;
        type Config = <C as CurtaConfig<2>>::GenericConfig;

        let _ = env_logger::builder().is_test(true).try_init();

        let mut timing = TimingTree::new("test_byte_multi_stark", log::Level::Debug);

        let mut builder = EmulatedBuilder::<L>::new();

        let a = builder.alloc::<FieldRegister<Fp25519>>();
        let b = builder.alloc::<FieldRegister<Fp25519>>();
        let _ = builder.add(a, b);

        let num_rows = 1 << 5;
        let stark = builder.build::<C, 2>(num_rows);

        let mut writer_data = AirWriterData::new(&stark.air_data, num_rows);

        let p = Fp25519::modulus();
        let air_data = &stark.air_data;
        air_data.write_global_instructions(&mut writer_data.public_writer());

        let k = 1 << 0;
        writer_data.chunks(k).for_each(|mut chunk| {
            let mut rng = rand::thread_rng();
            for i in 0..k {
                let mut writer = chunk.row_writer(i);
                let a_int = rng.gen_biguint(256) % &p;
                let b_int = rng.gen_biguint(256) % &p;
                let p_a = Polynomial::<F>::from_biguint_field(&a_int, 16, 16);
                let p_b = Polynomial::<F>::from_biguint_field(&b_int, 16, 16);
                writer.write(&a, &p_a);
                writer.write(&b, &p_b);
                air_data.write_trace_instructions(&mut writer);
            }
        });

        let (trace, public) = (writer_data.trace, writer_data.public);

        let proof = stark.prove(&trace, &public, &mut timing).unwrap();

        stark.verify(proof.clone(), &public).unwrap();

        let config_rec = CircuitConfig::standard_recursion_config();
        let mut recursive_builder = CircuitBuilder::<GoldilocksField, 2>::new(config_rec);

        let (proof_target, public_input) =
            stark.add_virtual_proof_with_pis_target(&mut recursive_builder);
        stark.verify_circuit(&mut recursive_builder, &proof_target, &public_input);

        let data = recursive_builder.build::<Config>();

        let mut pw = PartialWitness::new();

        pw.set_target_arr(&public_input, &public);
        stark.set_proof_target(&mut pw, &proof_target, proof);

        let rec_proof = data.prove(pw).unwrap();
        data.verify(rec_proof).unwrap();

        timing.print();
    }
}
