#![forbid(unsafe_code)]
//! Model-independent posterior-odds reconstruction and hierarchical contracts.

pub mod multinomial;

pub use multinomial::{
    FitConfig, FitSummary, MultinomialFitError, MultinomialLinearModel, MultinomialSample,
    posterior_log_density_ratios,
};

use mic_design::audit_sampling_odds;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Four posterior probabilities ordered as `00, 10, 01, 11`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PosteriorSquare {
    /// Posterior probabilities.
    pub q: [f64; 4],
}

impl PosteriorSquare {
    /// Reconstructs density curvature by subtracting pooled design odds.
    pub fn density_curvature(self, rho: [f64; 4]) -> Result<f64, ModelError> {
        validate_probabilities(&self.q, "posterior")?;
        let total: f64 = self.q.iter().sum();
        if (total - 1.0).abs() > 1e-8 {
            return Err(ModelError::NotNormalized {
                name: "posterior",
                total,
            });
        }
        let sampling =
            audit_sampling_odds(rho, 0.0).map_err(|error| ModelError::Design(error.to_string()))?;
        let [q00, q10, q01, q11] = self.q;
        let conditional_log_or = (q11 * q00 / (q10 * q01)).ln();
        Ok(conditional_log_or - sampling.log_odds_ratio)
    }
}

/// Dense hierarchy values for one design corner and state point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HierarchicalLogit {
    /// Corner-independent offset.
    pub intercept: f64,
    /// Primitive mechanism potentials.
    pub main_effects: Vec<f64>,
    /// Pairwise interaction fields keyed by coordinate pair.
    pub pair_effects: Vec<PairEffect>,
}

/// One pairwise design interaction contribution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PairEffect {
    /// First coordinate.
    pub first: usize,
    /// Second coordinate.
    pub second: usize,
    /// State-dependent interaction value.
    pub value: f64,
}

impl HierarchicalLogit {
    /// Evaluates the logit contribution for one Boolean corner.
    pub fn evaluate(&self, bits: &[bool]) -> Result<f64, ModelError> {
        if bits.len() != self.main_effects.len() {
            return Err(ModelError::Dimension {
                expected: self.main_effects.len(),
                actual: bits.len(),
            });
        }
        let mut value = self.intercept;
        for (active, effect) in bits.iter().zip(&self.main_effects) {
            if *active {
                value += effect;
            }
        }
        for pair in &self.pair_effects {
            if pair.first >= bits.len() || pair.second >= bits.len() {
                return Err(ModelError::InvalidPair {
                    first: pair.first,
                    second: pair.second,
                });
            }
            if bits[pair.first] && bits[pair.second] {
                value += pair.value;
            }
        }
        Ok(value)
    }
}

/// Stable softmax for finite logits.
pub fn softmax(logits: &[f64]) -> Result<Vec<f64>, ModelError> {
    if logits.is_empty() || logits.iter().any(|value| !value.is_finite()) {
        return Err(ModelError::InvalidLogits);
    }
    let max = logits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exp: Vec<f64> = logits.iter().map(|value| (value - max).exp()).collect();
    let sum: f64 = exp.iter().sum();
    Ok(exp.into_iter().map(|value| value / sum).collect())
}

/// Model contract errors.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ModelError {
    /// Invalid probabilities.
    #[error("{0} probabilities must be finite and positive")]
    InvalidProbabilities(&'static str),
    /// Probabilities were not normalized.
    #[error("{name} probabilities sum to {total}, expected 1")]
    NotNormalized {
        /// Name of the probability vector.
        name: &'static str,
        /// Observed total.
        total: f64,
    },
    /// Design error.
    #[error("design error: {0}")]
    Design(String),
    /// Dimension mismatch.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    Dimension {
        /// Expected dimension.
        expected: usize,
        /// Observed dimension.
        actual: usize,
    },
    /// Invalid pair index.
    #[error("invalid pair ({first}, {second})")]
    InvalidPair {
        /// First pair index.
        first: usize,
        /// Second pair index.
        second: usize,
    },
    /// Empty or nonfinite logits.
    #[error("logits must be nonempty and finite")]
    InvalidLogits,
}

fn validate_probabilities(values: &[f64; 4], name: &'static str) -> Result<(), ModelError> {
    if values.iter().all(|value| value.is_finite() && *value > 0.0) {
        Ok(())
    } else {
        Err(ModelError::InvalidProbabilities(name))
    }
}

/// FrankenTorch integration marker.
#[cfg(feature = "franken")]
pub mod franken {
    /// Reviewed FrankenTorch revision.
    pub const REVISION: &str = "5a3a0e70a2854c08e42ae02d816a78b8f88d912d";

    /// Backend identity.
    #[must_use]
    pub const fn backend_name() -> &'static str {
        "FrankenTorch"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posterior_curvature_preserves_constant_component() {
        let q = PosteriorSquare {
            q: [0.2, 0.2, 0.2, 0.4],
        };
        let rho = [0.25; 4];
        assert!((q.density_curvature(rho).unwrap() - 2.0_f64.ln()).abs() < 1e-14);
    }

    #[test]
    fn arbitrary_sampling_odds_are_subtracted() {
        let rho = [0.1, 0.2, 0.3, 0.4];
        let q = PosteriorSquare { q: rho };
        assert!(q.density_curvature(rho).unwrap().abs() < 1e-14);
    }
}
