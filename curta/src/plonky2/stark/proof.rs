use itertools::Itertools;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::fri::proof::{FriChallenges, FriChallengesTarget, FriProof, FriProofTarget};
use plonky2::fri::structure::{
    FriOpeningBatch, FriOpeningBatchTarget, FriOpenings, FriOpeningsTarget,
};
use plonky2::hash::hash_types::{MerkleCapTarget, RichField};
use plonky2::hash::merkle_tree::MerkleCap;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::GenericConfig;

use super::config::StarkyConfig;
use super::Starky;
use crate::air::parser::AirParser;
use crate::air::RAir;
use crate::maybe_rayon::*;
use crate::plonky2::challenger::{Plonky2Challenger, Plonky2RecursiveChallenger};
use crate::plonky2::parser::RecursiveStarkParser;

/// A proof of a STARK computation.
#[derive(Debug, Clone)]
pub struct StarkProof<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> {
    /// Merkle cap of LDEs of trace values for each round.
    pub trace_caps: Vec<MerkleCap<F, C::Hasher>>,
    /// Merkle cap of LDEs of trace values.
    pub quotient_polys_cap: MerkleCap<F, C::Hasher>,
    /// Purported values of each polynomial at the challenge point.
    pub openings: StarkOpeningSet<F, D>,
    /// A batch FRI argument for all openings.
    pub opening_proof: FriProof<F, C::Hasher, D>,
}

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> StarkProof<F, C, D> {
    /// Recover the length of the trace from a STARK proof and a STARK config.
    pub fn recover_degree_bits(&self, config: &StarkyConfig<F, C, D>) -> usize {
        let initial_merkle_proof = &self.opening_proof.query_round_proofs[0]
            .initial_trees_proof
            .evals_proofs[0]
            .1;
        let lde_bits = config.fri_config.cap_height + initial_merkle_proof.siblings.len();
        lde_bits - config.fri_config.rate_bits
    }

    pub(crate) fn get_challenges<AP: AirParser, A: RAir<AP>, const COLUMNS: usize>(
        &self,
        config: &StarkyConfig<F, C, D>,
        stark: &Starky<A, COLUMNS>,
        public_inputs: &[F],
        degree_bits: usize,
    ) -> StarkProofChallenges<F, D> {
        let StarkProof {
            trace_caps,
            quotient_polys_cap,
            openings,
            opening_proof:
                FriProof {
                    commit_phase_merkle_caps,
                    final_poly,
                    pow_witness,
                    ..
                },
        } = &self;

        let num_challenges = config.num_challenges;

        let mut challenger = Plonky2Challenger::<F, C::Hasher>::new();

        // Obsetrve public inputs
        challenger.0.observe_elements(public_inputs);

        let mut challenges = vec![];

        for (r, cap) in trace_caps.iter().enumerate() {
            challenger.0.observe_cap(cap);
            let round_challenges = challenger.0.get_n_challenges(stark.air().num_challenges(r));
            challenges.extend(round_challenges);
        }

        let stark_alphas = challenger.0.get_n_challenges(num_challenges);

        challenger.0.observe_cap(quotient_polys_cap);
        let stark_zeta = challenger.0.get_extension_challenge::<D>();

        challenger.0.observe_openings(&openings.to_fri_openings());

        StarkProofChallenges {
            stark_alphas,
            stark_betas: challenges,
            stark_zeta,
            fri_challenges: challenger.0.fri_challenges::<C, D>(
                commit_phase_merkle_caps,
                final_poly,
                *pow_witness,
                degree_bits,
                &config.fri_config,
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StarkProofTarget<const D: usize> {
    pub trace_caps: Vec<MerkleCapTarget>,
    pub quotient_polys_cap: MerkleCapTarget,
    pub openings: StarkOpeningSetTarget<D>,
    pub opening_proof: FriProofTarget<D>,
}

impl<const D: usize> StarkProofTarget<D> {
    /// Recover the length of the trace from a STARK proof and a STARK config.
    pub fn recover_degree_bits<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>>(
        &self,
        config: &StarkyConfig<F, C, D>,
    ) -> usize {
        let initial_merkle_proof = &self.opening_proof.query_round_proofs[0]
            .initial_trees_proof
            .evals_proofs[0]
            .1;
        let lde_bits = config.fri_config.cap_height + initial_merkle_proof.siblings.len();
        lde_bits - config.fri_config.rate_bits
    }

    pub fn get_challenges_target<
        F: RichField + Extendable<D>,
        A: for<'a> RAir<RecursiveStarkParser<'a, F, D>>,
        C: GenericConfig<D, F = F>,
        const COLUMNS: usize,
    >(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        config: &StarkyConfig<F, C, D>,
        public_inputs: &[Target],
        stark: &Starky<A, COLUMNS>,
    ) -> StarkProofChallengesTarget<D> {
        let StarkProofTarget {
            trace_caps,
            quotient_polys_cap,
            openings,
            opening_proof:
                FriProofTarget {
                    commit_phase_merkle_caps,
                    final_poly,
                    pow_witness,
                    ..
                },
        } = &self;

        let num_challenges = config.num_challenges;

        let mut challenger = Plonky2RecursiveChallenger::<F, C::InnerHasher, D>::new(builder);

        // Obsetrve public inputs
        challenger.0.observe_elements(public_inputs);

        let mut challenges = vec![];

        for (r, cap) in trace_caps.iter().enumerate() {
            challenger.0.observe_cap(cap);
            let round_challenges = challenger
                .0
                .get_n_challenges(builder, stark.air().num_challenges(r));
            challenges.extend(round_challenges);
        }

        let stark_alphas = challenger.0.get_n_challenges(builder, num_challenges);

        challenger.0.observe_cap(quotient_polys_cap);
        let stark_zeta = challenger.0.get_extension_challenge(builder);

        challenger.0.observe_openings(&openings.to_fri_openings());

        StarkProofChallengesTarget {
            stark_alphas,
            stark_betas: challenges,
            stark_zeta,
            fri_challenges: challenger.0.fri_challenges(
                builder,
                commit_phase_merkle_caps,
                final_poly,
                *pow_witness,
                &config.fri_config,
            ),
        }
    }
}

pub(crate) struct StarkProofChallenges<F: RichField + Extendable<D>, const D: usize> {
    /// Random values used to combine STARK constraints.
    pub stark_alphas: Vec<F>,

    /// Random values that can be used by the STARK for any purpose.
    pub stark_betas: Vec<F>,

    /// Point at which the STARK polynomials are opened.
    pub stark_zeta: F::Extension,

    pub fri_challenges: FriChallenges<F, D>,
}

pub struct StarkProofChallengesTarget<const D: usize> {
    pub stark_alphas: Vec<Target>,
    pub stark_betas: Vec<Target>,
    pub stark_zeta: ExtensionTarget<D>,
    pub fri_challenges: FriChallengesTarget<D>,
}

/// Purported values of each polynomial at the challenge point.
#[derive(Debug, Clone)]
pub struct StarkOpeningSet<F: RichField + Extendable<D>, const D: usize> {
    pub local_values: Vec<F::Extension>,
    pub next_values: Vec<F::Extension>,
    pub quotient_polys: Vec<F::Extension>,
}

impl<F: RichField + Extendable<D>, const D: usize> StarkOpeningSet<F, D> {
    pub fn new<C: GenericConfig<D, F = F>>(
        zeta: F::Extension,
        g: F,
        trace_commitments: &[PolynomialBatch<F, C, D>],
        quotient_commitment: &PolynomialBatch<F, C, D>,
    ) -> Self {
        let eval_commitment = |z: F::Extension, c: &PolynomialBatch<F, C, D>| {
            c.polynomials
                .par_iter()
                .map(|p| p.to_extension().eval(z))
                .collect::<Vec<_>>()
        };
        let zeta_next = zeta.scalar_mul(g);

        let local_values = trace_commitments
            .par_iter()
            .flat_map(|trace| eval_commitment(zeta, trace))
            .collect::<Vec<_>>();
        let next_values = trace_commitments
            .par_iter()
            .flat_map(|trace| eval_commitment(zeta_next, trace))
            .collect::<Vec<_>>();
        let quotient_polys = eval_commitment(zeta, quotient_commitment);
        Self {
            local_values,
            next_values,
            quotient_polys,
        }
    }

    pub(crate) fn to_fri_openings(&self) -> FriOpenings<F, D> {
        let zeta_batch = FriOpeningBatch {
            values: self
                .local_values
                .iter()
                .chain(&self.quotient_polys)
                .copied()
                .collect_vec(),
        };
        let zeta_next_batch = FriOpeningBatch {
            values: self.next_values.iter().copied().collect_vec(),
        };
        FriOpenings {
            batches: vec![zeta_batch, zeta_next_batch],
        }
    }
}

#[derive(Debug, Clone)]
pub struct StarkOpeningSetTarget<const D: usize> {
    pub local_values: Vec<ExtensionTarget<D>>,
    pub next_values: Vec<ExtensionTarget<D>>,
    pub quotient_polys: Vec<ExtensionTarget<D>>,
}

impl<const D: usize> StarkOpeningSetTarget<D> {
    pub(crate) fn to_fri_openings(&self) -> FriOpeningsTarget<D> {
        let zeta_batch = FriOpeningBatchTarget {
            values: self
                .local_values
                .iter()
                .chain(&self.quotient_polys)
                .copied()
                .collect_vec(),
        };
        let zeta_next_batch = FriOpeningBatchTarget {
            values: self.next_values.iter().copied().collect_vec(),
        };
        FriOpeningsTarget {
            batches: vec![zeta_batch, zeta_next_batch],
        }
    }
}
