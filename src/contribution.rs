use crate::parse_g::{parse_g, ParseError};
use ark_bls12_381::{g1, g2, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{AffineCurve, ProjectiveCurve};
use ark_ff::{One, Zero};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{self};
use std::cmp::max;
use thiserror::Error;
use tracing::error;
use valico::json_schema::{Schema, Scope};
use zeroize::Zeroizing;

const SIZES: [(usize, usize); 4] = [(4096, 65), (8192, 65), (16384, 65), (32768, 65)];

// static SCHEMA: Lazy<Mutex<Schema>> = Lazy::new(|| {
//     // Load schema
//     let schema =
// serde_json::from_str(include_str!("../specs/contributionSchema.json")).
// unwrap();     let schema = valico::schema::compile(schema).unwrap();
//     schema
// });

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
            sub_contributions: SIZES
                .iter()
                .map(|(num_g1, num_g2)| ContributionJson::initial(*num_g1, *num_g2))
                .collect(),
        }
    }

    pub fn from_json(json: &str) -> Result<Self, ContributionsError> {
        // let json = serde_json::from_str(json)?;
        // let validation = schema.validate(&initial);
        // if !validation.is_strictly_valid() {
        //     for error in validation.errors {
        //         error!("{}", error);
        //     }
        //     for missing in validation.missing {
        //         error!("Missing {}", missing);
        //     }
        //     // TODO bail!("Initial contribution is not valid.");
        // }
        // info!("Initial contribution is json-schema valid.");
        // TODO:
        todo!()
    }

    pub fn parse(&self) -> Result<Vec<Contribution>, ContributionsError> {
        if self.sub_contributions.len() != SIZES.len() {
            return Err(ContributionsError::InvalidContributionCount(
                4,
                self.sub_contributions.len(),
            ));
        }
        self.sub_contributions
            .iter()
            .zip(SIZES.iter())
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
            .map(|(i, result)| result.map_err(|e| ContributionsError::InvalidContribution(i, e)))
            .collect::<Result<_, _>>()?;
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
            parse_g::<g2::Parameters>(pubkey).map_err(|e| ContributionError::InvalidPubKey(e))?
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

impl Contribution {
    pub fn new(num_g1: usize, num_g2: usize) -> Self {
        Self {
            pubkey:    G2Affine::prime_subgroup_generator(),
            g1_powers: vec![G1Affine::prime_subgroup_generator(); num_g1],
            g2_powers: vec![G2Affine::prime_subgroup_generator(); num_g2],
        }
    }

    pub fn pairing_checks(&self, previous: &Self) {}

    pub fn add_tau(&mut self, tau: &Fr) {
        let n_tau = max(self.g1_powers.len(), self.g2_powers.len());
        let powers = Self::pow_table(&tau, n_tau);
        self.mul_g1(&powers[0..self.g1_powers.len()]);
        self.mul_g2(&powers[0..self.g2_powers.len()]);
        self.pubkey = self.pubkey.mul(*tau).into_affine();
    }

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

    fn mul_g1(&mut self, scalars: &[Fr]) {
        let projective = self
            .g1_powers
            .par_iter()
            .zip(scalars.par_iter())
            .map(|(c, pow_tau)| c.mul(*pow_tau))
            .collect::<Vec<_>>();
        self.g1_powers = G1Projective::batch_normalization_into_affine(&projective[..]);
    }

    fn mul_g2(&mut self, scalars: &[Fr]) {
        let projective = self
            .g2_powers
            .par_iter()
            .zip(scalars.par_iter())
            .map(|(c, pow_tau)| c.mul(*pow_tau))
            .collect::<Vec<_>>();
        self.g2_powers = G2Projective::batch_normalization_into_affine(&projective[..]);
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use ark_bls12_381::{G1Affine, G2Affine};
    use ark_ec::AffineCurve;
    use proptest::proptest;
}

#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench {
    use super::*;
    use ark_bls12_381::{g1, g2};
    use ark_ff::UniformRand;
    use criterion::{black_box, BatchSize, Criterion};
    use proptest::{
        strategy::{Strategy, ValueTree},
        test_runner::TestRunner,
    };

    pub fn group(criterion: &mut Criterion) {
        bench_pow_tau(criterion);
        bench_add_tau(criterion);
    }

    fn bench_pow_tau(criterion: &mut Criterion) {
        criterion.bench_function("contribution/pow_tau", move |bencher| {
            let mut rng = rand::thread_rng();
            let tau = Zeroizing::new(Fr::rand(&mut rng));
            bencher.iter(|| black_box(Contribution::pow_table(black_box(&tau), 32768)));
        });
    }

    fn bench_add_tau(criterion: &mut Criterion) {
        criterion.bench_function("contribution/add_tau", move |bencher| {
            let mut contrib = Contribution::new(32768, 65);
            let mut rng = rand::thread_rng();
            bencher.iter_batched(
                || Zeroizing::new(Fr::rand(&mut rng)),
                |tau| contrib.add_tau(&tau),
                BatchSize::SmallInput,
            );
        });
    }
}
