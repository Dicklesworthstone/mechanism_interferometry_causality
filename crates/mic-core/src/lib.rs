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
    /// Returns the gauge-invariant square curvature after rejecting nonfinite inputs.
    pub fn curvature(self) -> Result<f64, CoreError> {
        finite("log_p0", self.log_p0)?;
        finite("log_pa", self.log_pa)?;
        finite("log_pb", self.log_pb)?;
        finite("log_pab", self.log_pab)?;
        Ok((self.log_pab - self.log_pa) + (self.log_p0 - self.log_pb))
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
        Ok((self.pab.ln() - self.pa.ln()) + (self.p0.ln() - self.pb.ln()))
    }

    /// Signed product-law residual `p_AB - p_A p_B / p_0`.
    ///
    /// For positive masses this is zero if and only if [`Self::curvature`] is zero.
    pub fn closure_residual(self) -> Result<f64, CoreError> {
        positive("p0", self.p0)?;
        positive("pa", self.pa)?;
        positive("pb", self.pb)?;
        positive("pab", self.pab)?;
        Ok(self.pab - self.pa * self.pb / self.p0)
    }
}

/// Authority of an algebraic selection-sensitivity interval.
///
/// Γ-bounds are intervals under a declared inclusion-interaction budget.
/// They do not identify `π` from selected rows and never become Ready.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityAuthority {
    /// Declared-bound algebra only.
    DiagnosticOnly,
}

/// Source law, inclusion, and regime normalizers at one state.
///
/// Selected densities are `p_s π_s / Z_s`. The identity is
/// `κ̃ = κ + Δ_AB log π - Δ_AB log Z`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SelectionTransport {
    /// Unselected source densities.
    pub source: DensitySquare,
    /// Inclusion probabilities `π`.
    pub inclusion: DensitySquare,
    /// Regime normalizers `Z_s = ∫ p_s π_s`.
    pub normalizers: DensitySquare,
}

impl SelectionTransport {
    /// Selected densities `p π / Z`.
    pub fn selected(self) -> Result<DensitySquare, CoreError> {
        // Every factor is validated, not only the divisor. `DensitySquare` has public
        // fields, so a caller can supply a negative source and a negative inclusion; the
        // product of two invalid values is positive, and the resulting square would pass
        // every downstream positivity check while describing nothing. Validating only the
        // normalizers catches division by zero and misses that entirely.
        positive("source p0", self.source.p0)?;
        positive("source pa", self.source.pa)?;
        positive("source pb", self.source.pb)?;
        positive("source pab", self.source.pab)?;
        // Inclusion is a probability: positive, and never above one. A value above one
        // would silently manufacture selected mass that no sampling process can produce.
        inclusion_probability("inclusion p0", self.inclusion.p0)?;
        inclusion_probability("inclusion pa", self.inclusion.pa)?;
        inclusion_probability("inclusion pb", self.inclusion.pb)?;
        inclusion_probability("inclusion pab", self.inclusion.pab)?;
        // Z_s = integral of p_s pi_s is an inclusion rate under a probability pi, so it
        // obeys the same bound.
        inclusion_probability("Z0", self.normalizers.p0)?;
        inclusion_probability("Za", self.normalizers.pa)?;
        inclusion_probability("Zb", self.normalizers.pb)?;
        inclusion_probability("Zab", self.normalizers.pab)?;
        let selected = DensitySquare {
            p0: self.source.p0 * self.inclusion.p0 / self.normalizers.p0,
            pa: self.source.pa * self.inclusion.pa / self.normalizers.pa,
            pb: self.source.pb * self.inclusion.pb / self.normalizers.pb,
            pab: self.source.pab * self.inclusion.pab / self.normalizers.pab,
        };
        positive("selected p0", selected.p0)?;
        positive("selected pa", selected.pa)?;
        positive("selected pb", selected.pb)?;
        positive("selected pab", selected.pab)?;
        Ok(selected)
    }

    /// Residual of `κ̃ - (κ + Δ_AB log π - Δ_AB log Z)`.
    pub fn identity_residual(self) -> Result<f64, CoreError> {
        let observed = self.selected()?.curvature()?;
        let source = self.source.curvature()?;
        let delta_pi = self.inclusion.curvature()?;
        let delta_z = self.normalizers.curvature()?;
        Ok(observed - (source + delta_pi - delta_z))
    }
}

/// Interval for source curvature under a declared `|Δ_AB log π| ≤ Γ`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GammaSensitivity {
    /// Observed selected-law curvature `κ̃`.
    pub observed_kappa: f64,
    /// Declared inclusion-interaction budget.
    pub gamma: f64,
    /// `Δ_AB log Z`, treated as known (cancels in x-contrasts).
    pub delta_log_z: f64,
    /// Lower endpoint of the source-κ interval.
    pub source_kappa_low: f64,
    /// Upper endpoint of the source-κ interval.
    pub source_kappa_high: f64,
    /// Smallest `|Δ_AB log π|` that would make source κ zero.
    pub min_abs_selection_interaction_to_null: f64,
    /// Always diagnostic. Never Ready.
    pub authority: SensitivityAuthority,
}

/// Maps an observed selected curvature through a declared Γ budget.
///
/// `κ = κ̃ - Δ_AB log π + Δ_AB log Z` with `|Δ_AB log π| ≤ gamma`.
pub fn gamma_sensitivity(
    observed_kappa: f64,
    gamma: f64,
    delta_log_z: f64,
) -> Result<GammaSensitivity, CoreError> {
    finite("observed_kappa", observed_kappa)?;
    finite("gamma", gamma)?;
    finite("delta_log_z", delta_log_z)?;
    if gamma < 0.0 {
        return Err(CoreError::Negative {
            name: "gamma",
            value: gamma,
        });
    }
    Ok(GammaSensitivity {
        observed_kappa,
        gamma,
        delta_log_z,
        source_kappa_low: observed_kappa - gamma + delta_log_z,
        source_kappa_high: observed_kappa + gamma + delta_log_z,
        min_abs_selection_interaction_to_null: (observed_kappa + delta_log_z).abs(),
        authority: SensitivityAuthority::DiagnosticOnly,
    })
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
        Ok((self.rab.ln() - self.ra.ln()) - self.rb.ln())
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

/// Validates an inclusion probability or inclusion rate in `(0, 1]`.
///
/// Kept distinct from `positive` because the upper bound is the part that carries
/// meaning here: `pi > 1` is not a large probability, it is not a probability, and a
/// selected law built from one reports mass that no sampling process could have
/// produced.
fn inclusion_probability(name: &'static str, value: f64) -> Result<(), CoreError> {
    positive(name, value)?;
    if value > 1.0 {
        return Err(CoreError::InvalidNormalizer(value));
    }
    Ok(())
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
    fn two_invalid_factors_cannot_produce_a_valid_selected_law() {
        // `DensitySquare` has public fields, so nothing stops a caller supplying a
        // negative source and a negative inclusion. Their product is positive, so before
        // the factors were validated the resulting square passed every downstream
        // positivity check while describing nothing at all.
        let transport = SelectionTransport {
            source: DensitySquare {
                p0: -0.5,
                pa: -0.5,
                pb: -0.5,
                pab: -0.5,
            },
            inclusion: DensitySquare {
                p0: -0.4,
                pa: -0.4,
                pb: -0.4,
                pab: -0.4,
            },
            normalizers: DensitySquare {
                p0: 0.25,
                pa: 0.25,
                pb: 0.25,
                pab: 0.25,
            },
        };
        let error = transport.selected().unwrap_err();
        assert!(matches!(error, CoreError::NonPositive { .. }));
    }

    #[test]
    fn an_inclusion_probability_above_one_is_refused() {
        // pi > 1 is not a large probability; a selected law built from one reports mass
        // no sampling process could have produced.
        let transport = SelectionTransport {
            source: DensitySquare {
                p0: 0.25,
                pa: 0.25,
                pb: 0.25,
                pab: 0.25,
            },
            inclusion: DensitySquare {
                p0: 1.5,
                pa: 0.5,
                pb: 0.5,
                pab: 0.5,
            },
            normalizers: DensitySquare {
                p0: 0.25,
                pa: 0.25,
                pb: 0.25,
                pab: 0.25,
            },
        };
        assert!(transport.selected().is_err());
    }

    #[test]
    fn a_valid_selection_transport_still_succeeds() {
        let transport = SelectionTransport {
            source: DensitySquare {
                p0: 0.25,
                pa: 0.25,
                pb: 0.25,
                pab: 0.25,
            },
            inclusion: DensitySquare {
                p0: 0.5,
                pa: 0.5,
                pb: 0.5,
                pab: 0.5,
            },
            normalizers: DensitySquare {
                p0: 0.5,
                pa: 0.5,
                pb: 0.5,
                pab: 0.5,
            },
        };
        let selected = transport.selected().expect("valid inputs must still pass");
        assert!((selected.p0 - 0.25).abs() < 1e-12);
    }

    #[test]
    fn closure_residual_vanishes_iff_curvature_vanishes() {
        let flat = DensitySquare {
            p0: 0.25,
            pa: 0.25,
            pb: 0.25,
            pab: 0.25,
        };
        assert_eq!(flat.curvature().unwrap(), 0.0);
        assert!(flat.closure_residual().unwrap().abs() < 1e-15);

        let curved = DensitySquare {
            p0: 0.4,
            pa: 0.1,
            pb: 0.1,
            pab: 0.4,
        };
        assert!(curved.curvature().unwrap().abs() > 1.0);
        assert!(curved.closure_residual().unwrap() > 0.0);
    }

    #[test]
    fn selection_identity_holds_to_machine_precision() {
        let transport = SelectionTransport {
            source: DensitySquare {
                p0: 0.25,
                pa: 0.25,
                pb: 0.25,
                pab: 0.25,
            },
            inclusion: DensitySquare {
                p0: 0.8,
                pa: 0.2,
                pb: 0.2,
                pab: 0.8,
            },
            normalizers: DensitySquare {
                p0: 1.0,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            },
        };
        assert!(transport.identity_residual().unwrap().abs() < 1e-14);
        let selected = transport.selected().unwrap();
        assert!(selected.curvature().unwrap().abs() > 1.0);
        let observed = selected.curvature().unwrap();
        let needed = gamma_sensitivity(observed, 0.0, 0.0)
            .unwrap()
            .min_abs_selection_interaction_to_null;
        let interval = gamma_sensitivity(observed, needed, 0.0).unwrap();
        assert_eq!(interval.authority, SensitivityAuthority::DiagnosticOnly);
        assert!(interval.source_kappa_low <= 0.0);
        assert!(interval.source_kappa_high >= 0.0);
        assert!(needed > 0.0);
    }

    #[test]
    fn selection_transport_rejects_invalid_factors_and_overflow() {
        let negative_pair = SelectionTransport {
            source: DensitySquare {
                p0: -1.0,
                pa: -1.0,
                pb: -1.0,
                pab: -1.0,
            },
            inclusion: DensitySquare {
                p0: -1.0,
                pa: -1.0,
                pb: -1.0,
                pab: -1.0,
            },
            normalizers: DensitySquare {
                p0: 1.0,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            },
        };
        assert!(negative_pair.selected().is_err());

        let invalid_probability = SelectionTransport {
            source: DensitySquare {
                p0: 1.0,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            },
            inclusion: DensitySquare {
                p0: 2.0,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            },
            normalizers: DensitySquare {
                p0: 1.0,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            },
        };
        assert!(invalid_probability.selected().is_err());

        let overflow = SelectionTransport {
            source: DensitySquare {
                p0: f64::MAX,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            },
            inclusion: DensitySquare {
                p0: 1.0,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            },
            normalizers: DensitySquare {
                p0: f64::MIN_POSITIVE,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            },
        };
        assert!(overflow.selected().is_err());
    }

    #[test]
    fn gamma_sensitivity_never_claims_ready_and_rejects_negative_budget() {
        let interval = gamma_sensitivity(0.4, 0.1, 0.0).unwrap();
        assert_eq!(interval.authority, SensitivityAuthority::DiagnosticOnly);
        assert!((interval.source_kappa_low - 0.3).abs() < 1e-15);
        assert!((interval.source_kappa_high - 0.5).abs() < 1e-15);
        assert!(matches!(
            gamma_sensitivity(0.1, -0.2, 0.0),
            Err(CoreError::Negative { name: "gamma", .. })
        ));
    }

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
    fn log_contrasts_avoid_density_underflow_and_ratio_overflow() {
        let density = DensitySquare {
            p0: 1e-300,
            pa: 1e-300,
            pb: 1e-300,
            pab: 1e-300,
        };
        assert_eq!(density.curvature().unwrap(), 0.0);

        let ratio = RatioSquare {
            ra: 1e200,
            rb: 1e200,
            rab: 1e300,
        };
        let expected = (1e-100_f64).ln();
        assert!((ratio.curvature().unwrap() - expected).abs() < 1e-12);
    }

    #[test]
    fn log_density_curvature_rejects_nonfinite_inputs() {
        let valid = LogDensitySquare {
            log_p0: -3.0,
            log_pa: -2.0,
            log_pb: -4.0,
            log_pab: -3.0,
        };
        assert_eq!(valid.curvature().unwrap(), 0.0);

        let invalid = LogDensitySquare {
            log_pab: f64::INFINITY,
            ..valid
        };
        assert!(matches!(
            invalid.curvature(),
            Err(CoreError::NonFinite {
                name: "log_pab",
                ..
            })
        ));
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
