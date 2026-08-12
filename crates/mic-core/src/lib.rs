#![forbid(unsafe_code)]
//! Exact population algebra for mechanism interferometry.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced by exact algebra and numerical contract checks.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum CoreError {
    /// A numeric input was not finite.
    #[error("{name} must be finite, got {value}")]
    NonFinite {
        /// Name of the offending input.
        name: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// A density, ratio, or normalizing weight was not strictly positive.
    #[error("{name} must be strictly positive, got {value}")]
    NonPositive {
        /// Name of the offending input.
        name: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// A weight that may be zero was negative.
    #[error("{name} must be nonnegative, got {value}")]
    Negative {
        /// Name of the offending input.
        name: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// A vector expected to be non-empty was empty.
    #[error("{name} must not be empty")]
    Empty {
        /// Name of the offending input.
        name: &'static str,
    },
    /// Two vectors expected to have equal length did not.
    #[error("length mismatch: {left_name} has {left}, {right_name} has {right}")]
    LengthMismatch {
        /// Name of the first vector.
        left_name: &'static str,
        /// Length of the first vector.
        left: usize,
        /// Name of the second vector.
        right_name: &'static str,
        /// Length of the second vector.
        right: usize,
    },
    /// A normalization constant was nonfinite or not strictly positive.
    #[error("invalid normalization constant {0}")]
    InvalidNormalizer(f64),
}

/// Pairwise log-density inputs ordered as baseline, A, B, and AB.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LogDensitySquare {
    /// Log density at the reference corner.
    pub log_p0: f64,
    /// Log density under primitive A.
    pub log_pa: f64,
    /// Log density under primitive B.
    pub log_pb: f64,
    /// Log density under the joint corner AB.
    pub log_pab: f64,
}

impl LogDensitySquare {
    /// Returns the gauge-invariant square curvature.
    #[must_use]
    pub fn curvature(self) -> f64 {
        self.log_pab + self.log_p0 - self.log_pa - self.log_pb
    }
}

/// Density inputs ordered as baseline, A, B, and AB.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DensitySquare {
    /// Baseline density.
    pub p0: f64,
    /// Primitive-A density.
    pub pa: f64,
    /// Primitive-B density.
    pub pb: f64,
    /// Joint AB density.
    pub pab: f64,
}

impl DensitySquare {
    /// Computes curvature after validating common positive support.
    pub fn curvature(self) -> Result<f64, CoreError> {
        positive("p0", self.p0)?;
        positive("pa", self.pa)?;
        positive("pb", self.pb)?;
        positive("pab", self.pab)?;
        Ok((self.pab * self.p0 / (self.pa * self.pb)).ln())
    }
}

/// Primitive and joint ratios at one state point.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RatioSquare {
    /// Primitive-A ratio.
    pub ra: f64,
    /// Primitive-B ratio.
    pub rb: f64,
    /// Joint ratio.
    pub rab: f64,
}

impl RatioSquare {
    /// Computes `log(r_ab / (r_a r_b))`.
    pub fn curvature(self) -> Result<f64, CoreError> {
        positive("ra", self.ra)?;
        positive("rb", self.rb)?;
        positive("rab", self.rab)?;
        Ok((self.rab / (self.ra * self.rb)).ln())
    }

    /// Returns the pointwise curvature-balance integrand `r_a r_b (exp(kappa)-1)`.
    #[must_use]
    pub fn balance_integrand(self) -> f64 {
        self.rab - self.ra * self.rb
    }
}

/// Exact decomposition of coarse conditional coupling after adding state `W`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct NestedConservation {
    /// Coarse coupling `r_A^X r_B^X (exp(kappa_X)-1)`.
    pub coarse: f64,
    /// Conditional mean of refined residual coupling.
    pub mean_refined: f64,
    /// Conditional covariance of refined primitive ratios.
    pub refined_ratio_covariance: f64,
}

impl NestedConservation {
    /// Signed residual in the nested conservation identity.
    #[must_use]
    pub fn residual(self) -> f64 {
        self.coarse - self.mean_refined - self.refined_ratio_covariance
    }

    /// Returns true when the identity holds within absolute and relative tolerance.
    #[must_use]
    pub fn holds(self, atol: f64, rtol: f64) -> bool {
        if !atol.is_finite() || !rtol.is_finite() || atol < 0.0 || rtol < 0.0 {
            return false;
        }
        let scale = self
            .coarse
            .abs()
            .max(self.mean_refined.abs() + self.refined_ratio_covariance.abs());
        self.residual().abs() <= atol + rtol * scale
    }
}

/// Summary returned by self-normalizing nonnegative composition weights.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NormalizedWeights {
    /// Arithmetic mean of the raw weights under the supplied baseline sample.
    pub raw_normalizer: f64,
    /// Raw normalizer minus one, a moment-restriction diagnostic.
    pub normalizer_residual: f64,
    /// Weights divided by the raw normalizer.
    pub weights: Vec<f64>,
    /// Kish effective sample size of the normalized weights.
    pub effective_sample_size: f64,
}

/// Self-normalizes positive finite weights and preserves the raw normalizer diagnostic.
pub fn self_normalize(weights: &[f64]) -> Result<NormalizedWeights, CoreError> {
    if weights.is_empty() {
        return Err(CoreError::Empty { name: "weights" });
    }
    for &weight in weights {
        positive("weight", weight)?;
    }
    let normalizer = compensated_sum(weights) / weights.len() as f64;
    if !normalizer.is_finite() || normalizer <= 0.0 {
        return Err(CoreError::InvalidNormalizer(normalizer));
    }
    let normalized: Vec<f64> = weights.iter().map(|weight| weight / normalizer).collect();
    let sum = compensated_sum(&normalized);
    let sum_sq = compensated_sum(&normalized.iter().map(|x| x * x).collect::<Vec<_>>());
    let ess = if sum_sq > 0.0 {
        sum * sum / sum_sq
    } else {
        0.0
    };
    Ok(NormalizedWeights {
        raw_normalizer: normalizer,
        normalizer_residual: normalizer - 1.0,
        weights: normalized,
        effective_sample_size: ess,
    })
}

/// Computes a weighted mean with nonnegative finite weights.
pub fn weighted_mean(values: &[f64], weights: &[f64]) -> Result<f64, CoreError> {
    equal_len("values", values.len(), "weights", weights.len())?;
    if values.is_empty() {
        return Err(CoreError::Empty { name: "values" });
    }
    let mut numerator_terms = Vec::with_capacity(values.len());
    for (&value, &weight) in values.iter().zip(weights) {
        finite("value", value)?;
        finite("weight", weight)?;
        if weight < 0.0 {
            return Err(CoreError::Negative {
                name: "weight",
                value: weight,
            });
        }
        numerator_terms.push(value * weight);
    }
    let denominator = compensated_sum(weights);
    if denominator <= 0.0 {
        return Err(CoreError::InvalidNormalizer(denominator));
    }
    Ok(compensated_sum(&numerator_terms) / denominator)
}

/// Computes the population covariance with denominator `n`.
pub fn covariance(left: &[f64], right: &[f64]) -> Result<f64, CoreError> {
    equal_len("left", left.len(), "right", right.len())?;
    if left.is_empty() {
        return Err(CoreError::Empty { name: "left/right" });
    }
    let unit = vec![1.0; left.len()];
    let mean_left = weighted_mean(left, &unit)?;
    let mean_right = weighted_mean(right, &unit)?;
    let products: Vec<f64> = left
        .iter()
        .zip(right)
        .map(|(&x, &y)| (x - mean_left) * (y - mean_right))
        .collect();
    Ok(compensated_sum(&products) / products.len() as f64)
}

/// Computes `E[w (r_ab - r_a r_b)]` on an equally weighted baseline sample.
pub fn four_law_moment(
    witness: &[f64],
    ra: &[f64],
    rb: &[f64],
    rab: &[f64],
) -> Result<f64, CoreError> {
    equal_len("witness", witness.len(), "ra", ra.len())?;
    equal_len("witness", witness.len(), "rb", rb.len())?;
    equal_len("witness", witness.len(), "rab", rab.len())?;
    if witness.is_empty() {
        return Err(CoreError::Empty { name: "witness" });
    }
    let mut terms = Vec::with_capacity(witness.len());
    for (((&w, &a), &b), &ab) in witness.iter().zip(ra).zip(rb).zip(rab) {
        finite("witness", w)?;
        finite("ra", a)?;
        finite("rb", b)?;
        finite("rab", ab)?;
        terms.push(w * (ab - a * b));
    }
    Ok(compensated_sum(&terms) / terms.len() as f64)
}

/// Kahan compensated sum used by deterministic reference paths.
#[must_use]
pub fn compensated_sum(values: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for &value in values {
        let adjusted = value - correction;
        let next = sum + adjusted;
        correction = (next - sum) - adjusted;
        sum = next;
    }
    sum
}

fn finite(name: &'static str, value: f64) -> Result<(), CoreError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(CoreError::NonFinite { name, value })
    }
}

fn positive(name: &'static str, value: f64) -> Result<(), CoreError> {
    finite(name, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(CoreError::NonPositive { name, value })
    }
}

fn equal_len(
    left_name: &'static str,
    left: usize,
    right_name: &'static str,
    right: usize,
) -> Result<(), CoreError> {
    if left == right {
        Ok(())
    } else {
        Err(CoreError::LengthMismatch {
            left_name,
            left,
            right_name,
            right,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_and_ratio_curvature_agree() {
        let density = DensitySquare {
            p0: 0.2,
            pa: 0.3,
            pb: 0.4,
            pab: 0.72,
        };
        let ratio = RatioSquare {
            ra: 1.5,
            rb: 2.0,
            rab: 3.6,
        };
        let expected = (1.2_f64).ln();
        assert!((density.curvature().unwrap() - expected).abs() < 1e-14);
        assert!((ratio.curvature().unwrap() - expected).abs() < 1e-14);
    }

    #[test]
    fn self_normalization_preserves_raw_residual() {
        let result = self_normalize(&[0.5, 1.5, 2.0]).unwrap();
        assert!((result.raw_normalizer - 4.0 / 3.0).abs() < 1e-14);
        assert!((compensated_sum(&result.weights) / 3.0 - 1.0).abs() < 1e-14);
        assert!(result.effective_sample_size > 0.0);
    }

    #[test]
    fn latent_conservation_fixture() {
        let a = 0.3;
        let ra = [1.0 - a, 1.0 + a];
        let rb = [1.0 + a, 1.0 - a];
        assert!((covariance(&ra, &rb).unwrap() + a * a).abs() < 1e-14);
    }
}
