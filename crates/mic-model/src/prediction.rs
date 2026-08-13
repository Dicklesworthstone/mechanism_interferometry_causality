#![forbid(unsafe_code)]
//! Exact finite-state held-out combination-law prediction.
//!
//! The `11` law is never used to construct the prediction. Baseline and the
//! two primitive laws produce raw product ratios; the diagnostic reports their
//! raw normalizer before constructing the valid normalized predicted law. A
//! held-out `11` law is used only for nonnegative discrepancy evaluation.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const SIMPLEX_TOLERANCE: f64 = 1e-10;

/// Leave-the-combination-arm-out finite-law diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FiniteLawPredictionDiagnostic {
    /// Number of finite states in the shared support.
    pub n_states: usize,
    /// Baseline units used to translate the population ESS fraction to units.
    pub n_baseline_units: u32,
    /// `Z = sum_x p_A(x) p_B(x) / p_0(x)`, before normalization.
    pub raw_normalizer: f64,
    /// Raw normalizer minus one.
    pub normalizer_residual: f64,
    /// Predicted normalized `11` probabilities on the declared state order.
    pub predicted_probabilities: Vec<f64>,
    /// Maximum log normalized importance ratio over states.
    pub max_log_importance_ratio: f64,
    /// Population analogue of ESS divided by baseline sample size, in `(0,1]`.
    pub asymptotic_ess_fraction: f64,
    /// Baseline-unit count times the asymptotic ESS fraction.
    pub asymptotic_effective_units: f64,
    /// Total variation between the prediction and untouched `11` law.
    pub heldout_total_variation: f64,
    /// Squared Hellinger distance between prediction and untouched `11` law.
    pub heldout_hellinger_squared: f64,
    /// Explicit data-separation fact.
    pub combination_used_for_prediction: bool,
    /// Explicit authority boundary.
    pub calibrated_test: bool,
}

/// Fail-closed finite-law prediction errors.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum FiniteLawPredictionError {
    /// Probability vectors did not share a nonempty dimension.
    #[error("finite laws must have one shared nonempty dimension")]
    DimensionMismatch,
    /// A law was not a strictly positive finite simplex.
    #[error("{law} must be a strictly positive finite probability simplex")]
    InvalidLaw {
        /// Name of the invalid law.
        law: &'static str,
    },
    /// No independent baseline units were declared.
    #[error("n_baseline_units must be positive")]
    EmptyBaselineUnits,
    /// The mandatory raw normalizer was not representable as positive finite `f64`.
    #[error("raw product normalizer is not representable as a positive finite value")]
    InvalidNormalizer,
}

/// Predicts the `11` law from `00`, `10`, and `01`, then evaluates it on an
/// untouched `11` law.
///
/// All inputs must use the same finite state ordering and strictly positive
/// common support. This is an exact finite-state reference diagnostic, not a
/// calibrated sample-level test.
pub fn predict_combination_law(
    baseline: &[f64],
    primitive_a: &[f64],
    primitive_b: &[f64],
    heldout_combination: &[f64],
    n_baseline_units: u32,
) -> Result<FiniteLawPredictionDiagnostic, FiniteLawPredictionError> {
    if baseline.is_empty()
        || primitive_a.len() != baseline.len()
        || primitive_b.len() != baseline.len()
        || heldout_combination.len() != baseline.len()
    {
        return Err(FiniteLawPredictionError::DimensionMismatch);
    }
    validate_law(baseline, "baseline")?;
    validate_law(primitive_a, "primitive_a")?;
    validate_law(primitive_b, "primitive_b")?;
    validate_law(heldout_combination, "heldout_combination")?;
    if n_baseline_units == 0 {
        return Err(FiniteLawPredictionError::EmptyBaselineUnits);
    }

    let log_contributions = baseline
        .iter()
        .zip(primitive_a)
        .zip(primitive_b)
        .map(|((p0, pa), pb)| pa.ln() + pb.ln() - p0.ln())
        .collect::<Vec<_>>();
    let log_normalizer = log_sum_exp(&log_contributions);
    let raw_normalizer = log_normalizer.exp();
    if !raw_normalizer.is_finite() || raw_normalizer <= 0.0 {
        return Err(FiniteLawPredictionError::InvalidNormalizer);
    }
    let predicted_probabilities = log_contributions
        .iter()
        .map(|value| (value - log_normalizer).exp())
        .collect::<Vec<_>>();
    let max_log_importance_ratio = predicted_probabilities
        .iter()
        .zip(baseline)
        .map(|(predicted, p0)| predicted.ln() - p0.ln())
        .fold(f64::NEG_INFINITY, f64::max);
    let log_second_moment = log_sum_exp(
        &predicted_probabilities
            .iter()
            .zip(baseline)
            .map(|(predicted, p0)| 2.0 * predicted.ln() - p0.ln())
            .collect::<Vec<_>>(),
    );
    let asymptotic_ess_fraction = (-log_second_moment).exp().min(1.0);
    let heldout_total_variation = predicted_probabilities
        .iter()
        .zip(heldout_combination)
        .map(|(predicted, observed)| (predicted - observed).abs())
        .sum::<f64>()
        / 2.0;
    let heldout_hellinger_squared = predicted_probabilities
        .iter()
        .zip(heldout_combination)
        .map(|(predicted, observed)| (predicted.sqrt() - observed.sqrt()).powi(2))
        .sum::<f64>()
        / 2.0;
    Ok(FiniteLawPredictionDiagnostic {
        n_states: baseline.len(),
        n_baseline_units,
        raw_normalizer,
        normalizer_residual: raw_normalizer - 1.0,
        predicted_probabilities,
        max_log_importance_ratio,
        asymptotic_ess_fraction,
        asymptotic_effective_units: f64::from(n_baseline_units) * asymptotic_ess_fraction,
        heldout_total_variation,
        heldout_hellinger_squared,
        combination_used_for_prediction: false,
        calibrated_test: false,
    })
}

fn validate_law(probabilities: &[f64], law: &'static str) -> Result<(), FiniteLawPredictionError> {
    if probabilities
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        || (probabilities.iter().sum::<f64>() - 1.0).abs() > SIMPLEX_TOLERANCE
    {
        return Err(FiniteLawPredictionError::InvalidLaw { law });
    }
    Ok(())
}

fn log_sum_exp(values: &[f64]) -> f64 {
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    maximum
        + values
            .iter()
            .map(|value| (value - maximum).exp())
            .sum::<f64>()
            .ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heldout_prediction_reports_raw_normalizer_and_zero_discrepancy() {
        let diagnostic =
            predict_combination_law(&[0.5, 0.5], &[0.6, 0.4], &[0.4, 0.6], &[0.5, 0.5], 40)
                .unwrap();
        assert!((diagnostic.raw_normalizer - 0.96).abs() < 1e-14);
        assert!((diagnostic.normalizer_residual + 0.04).abs() < 1e-14);
        assert!(diagnostic.heldout_total_variation < 1e-14);
        assert!(diagnostic.heldout_hellinger_squared < 1e-14);
        assert!((diagnostic.asymptotic_ess_fraction - 1.0).abs() < 1e-14);
        assert!(!diagnostic.combination_used_for_prediction);
        assert!(!diagnostic.calibrated_test);
    }

    #[test]
    fn overlap_concentration_reduces_effective_units() {
        let diagnostic =
            predict_combination_law(&[0.5, 0.5], &[0.9, 0.1], &[0.9, 0.1], &[0.8, 0.2], 100)
                .unwrap();
        assert!((diagnostic.raw_normalizer - 1.64).abs() < 1e-14);
        assert!(diagnostic.max_log_importance_ratio > 0.0);
        assert!(diagnostic.asymptotic_ess_fraction > 0.5);
        assert!(diagnostic.asymptotic_ess_fraction < 0.53);
        assert!(diagnostic.asymptotic_effective_units < 53.0);
        assert!(diagnostic.heldout_total_variation > 0.0);
    }

    #[test]
    fn invalid_support_simplex_and_unit_count_fail_closed() {
        assert_eq!(
            predict_combination_law(&[1.0, 0.0], &[0.5, 0.5], &[0.5, 0.5], &[0.5, 0.5], 1)
                .unwrap_err(),
            FiniteLawPredictionError::InvalidLaw { law: "baseline" }
        );
        assert_eq!(
            predict_combination_law(&[0.5, 0.5], &[0.5, 0.5], &[0.5, 0.5], &[0.5, 0.5], 0)
                .unwrap_err(),
            FiniteLawPredictionError::EmptyBaselineUnits
        );
        assert_eq!(
            predict_combination_law(&[1.0], &[1.0], &[1.0, 0.0], &[1.0], 1).unwrap_err(),
            FiniteLawPredictionError::DimensionMismatch
        );
    }
}
