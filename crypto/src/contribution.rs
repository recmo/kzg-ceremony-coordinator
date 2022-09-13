use crate::{
    crypto::g1_mul_glv, g1_subgroup_check, g2_subgroup_check, json_schema::CONTRIBUTION_SCHEMA,
    parse_g, zcash_format::encode_p, ParseError,
};
use ark_bls12_381::{g1, g2, Bls12_381, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{msm::VariableBaseMSM, AffineCurve, PairingEngine, ProjectiveCurve};
use ark_ff::{One, PrimeField, UniformRand, Zero};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{cmp::max, iter};
use thiserror::Error;
use tracing::{error, info, instrument};
use valico::json_schema::{self, schema::ScopedSchema};
use zeroize::Zeroizing;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Transcript {
    pub g1_powers: Vec<G1Affine>,
    pub g2_powers: Vec<G2Affine>,
    pub products:  Vec<G1Affine>,
    pub pubkeys:   Vec<G2Affine>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Contribution {
    pub pubkey:    G2Affine,
    pub g1_powers: Vec<G1Affine>,
    pub g2_powers: Vec<G2Affine>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionsJson {
    pub sub_contributions: Vec<ContributionJson>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionJson {
    pub num_g1_powers: usize,
    pub num_g2_powers: usize,
    pub powers_of_tau: PowersOfTau,
    pub pot_pubkey:    Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowersOfTau {
    pub g1_powers: Vec<String>,
    pub g2_powers: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Debug, Error)]
pub enum ContributionsError {
    #[error("Error in contribution {0}: {1}")]
    InvalidContribution(usize, #[source] ContributionError),
    #[error("Unexpected number of contributions: expected {0}, got {1}")]
    InvalidContributionCount(usize, usize),
    #[error("Error validating schema")]
    InvalidSchema(),
}

#[derive(Clone, Copy, PartialEq, Debug, Error)]
pub enum ContributionError {
    #[error("Unexpected number of G1 powers: expected {0}, got {1}")]
    UnexpectedNumG1Powers(usize, usize),
    #[error("Unexpected number of G2 powers: expected {0}, got {1}")]
    UnexpectedNumG2Powers(usize, usize),
    #[error("Inconsistent number of G1 powers: numG1Powers = {0}, len = {1}")]
    InconsistentNumG1Powers(usize, usize),
    #[error("Inconsistent number of G2 powers: numG2Powers = {0}, len = {1}")]
    InconsistentNumG2Powers(usize, usize),
    #[error("Error parsing G1 power {0}: {1}")]
    InvalidG1Power(usize, #[source] ParseError),
    #[error("Error parsing G2 power {0}: {1}")]
    InvalidG2Power(usize, #[source] ParseError),
    #[error("Error parsing potPubkey: {0}")]
    InvalidPubKey(#[source] ParseError),
}

impl ContributionsJson {
    pub fn initial() -> Self {
        Self {
            sub_contributions: crate::SIZES
                .iter()
                .map(|(num_g1, num_g2)| ContributionJson::initial(*num_g1, *num_g2))
                .collect(),
        }
    }

    #[cfg(feature = "schema-validation")]
    pub fn from_json(json: &str) -> Result<Self, ContributionsError> {
        let json: Value =
            serde_json::from_str(json).map_err(|_| ContributionsError::InvalidSchema())?;

        let validation = ScopedSchema::new(
            &json_schema::Scope::new(),
            &CONTRIBUTION_SCHEMA.lock().unwrap(),
        )
        .validate(&json);

        if !validation.is_strictly_valid() {
            for error in validation.errors {
                error!("{}", error);
            }
            for missing in validation.missing {
                error!("Missing {}", missing);
            }
            error!("Initial contribution is json-schema invalid.");
            return Err(ContributionsError::InvalidSchema());
        }
        info!("Initial contribution is json-schema valid.");

        serde_json::from_value::<Self>(json).map_err(|e| ContributionsError::InvalidSchema())
    }

    #[cfg(not(feature = "schema-validation"))]
    pub fn from_json(json: &str) -> Result<Self, ContributionsError> {
        let json: Value =
            serde_json::from_str(json).map_err(|_| ContributionsError::InvalidSchema())?;

        serde_json::from_value::<Self>(json).map_err(|e| ContributionsError::InvalidSchema())
    }

    pub fn parse(&self) -> Result<Vec<Contribution>, ContributionsError> {
        if self.sub_contributions.len() != crate::SIZES.len() {
            return Err(ContributionsError::InvalidContributionCount(
                4,
                self.sub_contributions.len(),
            ));
        }
        self.sub_contributions
            .iter()
            .zip(crate::SIZES.iter())
            .map(|(c, (num_g1, num_g2))| {
                if c.num_g1_powers != *num_g1 {
                    return Err(ContributionError::UnexpectedNumG1Powers(
                        *num_g1,
                        c.num_g1_powers,
                    ));
                }
                if c.num_g2_powers != *num_g2 {
                    return Err(ContributionError::UnexpectedNumG1Powers(
                        *num_g1,
                        c.num_g1_powers,
                    ));
                }
                Ok(())
            })
            .enumerate()
            .try_for_each(|(i, result)| {
                result.map_err(|e| ContributionsError::InvalidContribution(i, e))
            })?;
        self.sub_contributions
            .par_iter()
            .enumerate()
            .map(|(i, c)| {
                c.parse()
                    .map_err(|e| ContributionsError::InvalidContribution(i, e))
            })
            .collect::<Result<Vec<_>, _>>()
    }
}

impl ContributionJson {
    pub fn initial(num_g1_powers: usize, num_g2_powers: usize) -> Self {
        Self {
            num_g1_powers,
            num_g2_powers,
            powers_of_tau: PowersOfTau::initial(num_g1_powers, num_g2_powers),
            pot_pubkey: None,
        }
    }

    pub fn parse(&self) -> Result<Contribution, ContributionError> {
        if self.powers_of_tau.g1_powers.len() != self.num_g1_powers {
            return Err(ContributionError::InconsistentNumG1Powers(
                self.num_g1_powers,
                self.powers_of_tau.g1_powers.len(),
            ));
        }
        if self.powers_of_tau.g2_powers.len() != self.num_g2_powers {
            return Err(ContributionError::InconsistentNumG2Powers(
                self.num_g2_powers,
                self.powers_of_tau.g2_powers.len(),
            ));
        }
        let g1_powers = self
            .powers_of_tau
            .g1_powers
            .par_iter()
            .enumerate()
            .map(|(i, hex)| {
                parse_g::<g1::Parameters>(hex).map_err(|e| ContributionError::InvalidG1Power(i, e))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let g2_powers = self
            .powers_of_tau
            .g2_powers
            .par_iter()
            .enumerate()
            .map(|(i, hex)| {
                parse_g::<g2::Parameters>(hex).map_err(|e| ContributionError::InvalidG2Power(i, e))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pubkey = if let Some(pubkey) = &self.pot_pubkey {
            parse_g::<g2::Parameters>(pubkey).map_err(ContributionError::InvalidPubKey)?
        } else {
            G2Affine::zero()
        };
        Ok(Contribution {
            pubkey,
            g1_powers,
            g2_powers,
        })
    }
}

impl PowersOfTau {
    pub fn initial(num_g1_powers: usize, num_g2_powers: usize) -> Self {
        Self {
            g1_powers: vec!["0x97f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb".to_string(); num_g1_powers],
            g2_powers: vec!["0x93e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8".to_string(); num_g2_powers],
        }
    }
}

impl Transcript {
    #[must_use]
    pub fn new(num_g1: usize, num_g2: usize) -> Self {
        Self {
            pubkeys:   vec![G2Affine::prime_subgroup_generator()],
            products:  vec![G1Affine::prime_subgroup_generator()],
            g1_powers: vec![G1Affine::prime_subgroup_generator(); num_g1],
            g2_powers: vec![G2Affine::prime_subgroup_generator(); num_g2],
        }
    }
}

impl Contribution {
    pub fn new(num_g1: usize, num_g2: usize) -> Self {
        Self {
            pubkey:    G2Affine::prime_subgroup_generator(),
            g1_powers: vec![G1Affine::prime_subgroup_generator(); num_g1],
            g2_powers: vec![G2Affine::prime_subgroup_generator(); num_g2],
        }
    }

    #[instrument(level = "info", skip_all, fields(n1=self.g1_powers.len(), n2=self.g2_powers.len()))]
    pub fn subgroup_check(&self) {
        assert!(self.pubkey.is_in_correct_subgroup_assuming_on_curve());
        self.g1_powers
            .par_iter()
            .for_each(|point| assert!(g1_subgroup_check(point)));
        self.g2_powers
            .par_iter()
            .for_each(|point| assert!(g2_subgroup_check(point)));
    }

    #[instrument(level = "info", skip_all)]
    pub fn add_tau(&mut self, tau: &Fr) {
        let n_tau = max(self.g1_powers.len(), self.g2_powers.len());
        let powers = Self::pow_table(&tau, n_tau);
        self.mul_g1(&powers[0..self.g1_powers.len()]);
        self.mul_g2(&powers[0..self.g2_powers.len()]);
        self.pubkey = self.pubkey.mul(*tau).into_affine();
    }

    #[instrument(level = "info", skip_all)]
    fn pow_table(tau: &Fr, n: usize) -> Zeroizing<Vec<Fr>> {
        let mut powers = Zeroizing::new(Vec::with_capacity(n));
        let mut pow_tau = Zeroizing::new(Fr::one());
        powers.push(*pow_tau);
        for _ in 1..n {
            *pow_tau *= *tau;
            powers.push(*pow_tau);
        }
        powers
    }

    #[instrument(level = "info", skip_all)]
    fn mul_g1(&mut self, scalars: &[Fr]) {
        let projective = self
            .g1_powers
            .par_iter()
            .zip(scalars.par_iter())
            .map(|(c, pow_tau)| g1_mul_glv(c, *pow_tau))
            .collect::<Vec<_>>();
        self.g1_powers = G1Projective::batch_normalization_into_affine(&projective[..]);
    }

    #[instrument(level = "info", skip_all)]
    fn mul_g2(&mut self, scalars: &[Fr]) {
        let projective = self
            .g2_powers
            .par_iter()
            .zip(scalars.par_iter())
            .map(|(c, pow_tau)| c.mul(*pow_tau))
            .collect::<Vec<_>>();
        self.g2_powers = G2Projective::batch_normalization_into_affine(&projective[..]);
    }

    #[instrument(level = "info", skip_all)]
    pub fn verify(&self, transcript: &Transcript) {
        assert_eq!(self.g1_powers.len(), transcript.g1_powers.len());
        assert_eq!(self.g2_powers.len(), transcript.g2_powers.len());
        self.verify_pubkey(transcript.products.last().unwrap());
        self.verify_g1();
        self.verify_g2();
    }

    #[instrument(level = "info", skip_all)]
    fn verify_pubkey(&self, prev_product: &G1Affine) {
        assert_eq!(
            Bls12_381::pairing(self.g1_powers[1], G2Affine::prime_subgroup_generator()),
            Bls12_381::pairing(*prev_product, self.pubkey)
        );
    }

    #[instrument(level = "info", skip_all)]
    fn verify_g1(&self) {
        let (factors, sum) = random_factors(self.g1_powers.len() - 1);
        let lhs_g1 = VariableBaseMSM::multi_scalar_mul(&self.g1_powers[1..], &factors[..]);
        let lhs_g2 = G2Affine::prime_subgroup_generator().mul(sum);
        let rhs_g1 =
            VariableBaseMSM::multi_scalar_mul(&self.g1_powers[..factors.len()], &factors[..]);
        let rhs_g2 = self.g2_powers[1].mul(sum);
        assert_eq!(
            Bls12_381::pairing(lhs_g1, lhs_g2),
            Bls12_381::pairing(rhs_g1, rhs_g2)
        );
    }

    #[instrument(level = "info", skip_all)]
    fn verify_g2(&self) {
        let (factors, sum) = random_factors(self.g2_powers.len());
        let lhs_g1 =
            VariableBaseMSM::multi_scalar_mul(&self.g1_powers[..factors.len()], &factors[..]);
        let lhs_g2 = G2Affine::prime_subgroup_generator().mul(sum);
        let rhs_g1 = G1Affine::prime_subgroup_generator().mul(sum);
        let rhs_g2 = VariableBaseMSM::multi_scalar_mul(&self.g2_powers[..], &factors[..]);
        assert_eq!(
            Bls12_381::pairing(lhs_g1, lhs_g2),
            Bls12_381::pairing(rhs_g1, rhs_g2)
        );
    }
}

// Convert from Contribution to ContributionJson
impl From<Contribution> for ContributionJson {
    fn from(contribution: Contribution) -> Self {
        Self {
            num_g1_powers: contribution.g1_powers.len(),
            num_g2_powers: contribution.g2_powers.len(),
            pot_pubkey:    Some(encode_p::<g2::Parameters>(contribution.pubkey)),
            powers_of_tau: PowersOfTau {
                g1_powers: contribution
                    .g1_powers
                    .into_par_iter()
                    .map(encode_p::<g1::Parameters>)
                    .collect::<Vec<_>>(),
                g2_powers: contribution
                    .g2_powers
                    .into_par_iter()
                    .map(encode_p::<g2::Parameters>)
                    .collect::<Vec<_>>(),
            },
        }
    }
}

fn random_factors(n: usize) -> (Vec<<Fr as PrimeField>::BigInt>, Fr) {
    let mut rng = rand::thread_rng();
    let mut sum = Fr::zero();
    let factors = iter::from_fn(|| {
        let r = Fr::rand(&mut rng);
        sum += r;
        Some(r.0)
    })
    .take(n)
    .collect::<Vec<_>>();
    (factors, sum)
}

#[cfg(test)]
pub mod test {
    use super::*;
    use ark_ff::UniformRand;

    #[test]
    fn verify() {
        let mut transcript = Transcript::new(32768, 65);
        let mut contrib = Contribution::new(32768, 65);
        contrib.verify(&transcript);
        let mut rng = rand::thread_rng();
        contrib.add_tau(&Fr::rand(&mut rng));
        contrib.verify(&transcript);
    }
}

#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench {
    use crate::bench::rand_fr;

    use super::*;
    use ark_ff::UniformRand;
    use criterion::{black_box, BatchSize, BenchmarkId, Criterion};

    pub fn group(criterion: &mut Criterion) {
        bench_pow_tau(criterion);
        bench_add_tau(criterion);
        bench_verify(criterion);
    }

    fn bench_pow_tau(criterion: &mut Criterion) {
        criterion.bench_function("contribution/pow_tau", move |bencher| {
            let mut rng = rand::thread_rng();
            let tau = Zeroizing::new(Fr::rand(&mut rng));
            bencher.iter(|| black_box(Contribution::pow_table(black_box(&tau), 32768)));
        });
    }

    fn bench_add_tau(criterion: &mut Criterion) {
        for size in crate::SIZES {
            criterion.bench_with_input(
                BenchmarkId::new("contribution/add_tau", format!("{:?}", size)),
                &size,
                move |bencher, (n1, n2)| {
                    let mut contrib = Contribution::new(*n1, *n2);
                    bencher.iter_batched(
                        || rand_fr(),
                        |tau| contrib.add_tau(&tau),
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }

    fn bench_verify(criterion: &mut Criterion) {
        for size in crate::SIZES {
            criterion.bench_with_input(
                BenchmarkId::new("contribution/verify", format!("{:?}", size)),
                &size,
                move |bencher, (n1, n2)| {
                    let mut transcript = Transcript::new(*n1, *n2);
                    let mut contrib = Contribution::new(*n1, *n2);
                    contrib.add_tau(&rand_fr());
                    bencher.iter(|| black_box(contrib.verify(&transcript)));
                },
            );
        }
    }
}
