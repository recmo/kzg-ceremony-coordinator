use crate::parse_g::{parse_g, ParseError};
use ark_bls12_381::{g1, g2};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SIZES: [(usize, usize); 4] = [(4096, 65), (8192, 65), (16384, 65), (32768, 65)];

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributions {
    pub sub_contributions: Vec<Contribution>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contribution {
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

impl Contributions {
    pub fn initial() -> Self {
        Self {
            sub_contributions: SIZES
                .iter()
                .map(|(num_g1, num_g2)| Contribution::initial(*num_g1, *num_g2))
                .collect(),
        }
    }

    pub fn validate(&self) -> Result<(), ContributionsError> {
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
            .iter()
            .enumerate()
            .map(|(i, c)| {
                c.validate()
                    .map_err(|e| ContributionsError::InvalidContribution(i, e))
            })
            .collect::<Result<_, _>>()?;
        Ok(())
    }
}

impl Contribution {
    pub fn initial(num_g1_powers: usize, num_g2_powers: usize) -> Self {
        Self {
            num_g1_powers,
            num_g2_powers,
            powers_of_tau: PowersOfTau::initial(num_g1_powers, num_g2_powers),
            pot_pubkey: None,
        }
    }

    pub fn validate(&self) -> Result<(), ContributionError> {
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
        self.powers_of_tau
            .g1_powers
            .iter()
            .enumerate()
            .map(|(i, hex)| {
                parse_g::<g1::Parameters>(hex)
                    .map_err(|e| ContributionError::InvalidG1Power(i, e))
                    .map(|_| ())
            })
            .collect::<Result<_, _>>()?;
        self.powers_of_tau
            .g2_powers
            .iter()
            .enumerate()
            .map(|(i, hex)| {
                parse_g::<g2::Parameters>(hex)
                    .map_err(|e| ContributionError::InvalidG2Power(i, e))
                    .map(|_| ())
            })
            .collect::<Result<_, _>>()?;
        if let Some(pubkey) = &self.pot_pubkey {
            parse_g::<g2::Parameters>(pubkey).map_err(|e| ContributionError::InvalidPubKey(e))?;
        }
        Ok(())
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

impl Default for Contributions {
    fn default() -> Self {
        Self::initial()
    }
}
