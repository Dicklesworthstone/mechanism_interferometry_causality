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
    /// A probability-scaled quantity fell outside `(0, 1]`.
    #[error("{name} must lie in (0, 1], got {value}")]
    OutsideUnitInterval {
        /// Offending field.
        name: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// A content-bound evidence reference was malformed.
    #[error("evidence reference must be sha256:<64 lowercase hex>")]
    InvalidEvidenceReference,
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

/// Four regime-specific inclusion probabilities with `0 < π_s ≤ 1`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(try_from = "DensitySquare", into = "DensitySquare")]
pub struct InclusionProbabilitySquare(DensitySquare);

impl TryFrom<DensitySquare> for InclusionProbabilitySquare {
    type Error = CoreError;

    fn try_from(value: DensitySquare) -> Result<Self, Self::Error> {
        validate_probability_square(value)?;
        Ok(Self(value))
    }
}

impl From<InclusionProbabilitySquare> for DensitySquare {
    fn from(value: InclusionProbabilitySquare) -> Self {
        value.0
    }
}

impl InclusionProbabilitySquare {
    /// Returns the validated probability values.
    #[must_use]
    pub fn values(self) -> DensitySquare {
        self.0
    }
}

/// Four regime-specific inclusion rates with `0 < Z_s ≤ 1`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(try_from = "DensitySquare", into = "DensitySquare")]
pub struct InclusionRateSquare(DensitySquare);

impl TryFrom<DensitySquare> for InclusionRateSquare {
    type Error = CoreError;

    fn try_from(value: DensitySquare) -> Result<Self, Self::Error> {
        validate_probability_square(value)?;
        Ok(Self(value))
    }
}

impl From<InclusionRateSquare> for DensitySquare {
    fn from(value: InclusionRateSquare) -> Self {
        value.0
    }
}

impl InclusionRateSquare {
    /// Returns the validated inclusion-rate values.
    #[must_use]
    pub fn values(self) -> DensitySquare {
        self.0
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
    pub inclusion: InclusionProbabilitySquare,
    /// Regime normalizers `Z_s = ∫ p_s π_s`.
    pub normalizers: InclusionRateSquare,
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
        let inclusion = self.inclusion.values();
        // Z_s = integral of p_s pi_s is an inclusion rate under a probability pi, so it
        // obeys the same bound.
        let normalizers = self.normalizers.values();
        let selected = DensitySquare {
            p0: self.source.p0 * inclusion.p0 / normalizers.p0,
            pa: self.source.pa * inclusion.pa / normalizers.pa,
            pb: self.source.pb * inclusion.pb / normalizers.pb,
            pab: self.source.pab * inclusion.pab / normalizers.pab,
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
        let delta_pi = self.inclusion.values().curvature()?;
        let delta_z = self.normalizers.values().curvature()?;
        Ok(observed - (source + delta_pi - delta_z))
    }
}

/// Provenance semantics of the normalizer interaction used by sensitivity algebra.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum NormalizerContrastEvidence {
    /// Caller-declared value with no independently resolved evidence receipt.
    DeclaredUnverified {
        /// Declared `Δ_AB log Z`.
        delta_log_z: f64,
    },
    /// Inclusion rates plus an unresolved content-bound enrollment receipt.
    EnrollmentRatesReceiptUnresolved {
        /// Validated regime-specific enrollment or inclusion rates.
        rates: InclusionRateSquare,
        /// `sha256:<64 lowercase hex>` receipt commitment.
        receipt_sha256: String,
    },
    /// Selection-model contrast plus an unresolved content-bound model receipt.
    SelectionModelReceiptUnresolved {
        /// Model-derived `Δ_AB log Z`.
        delta_log_z: f64,
        /// `sha256:<64 lowercase hex>` model receipt commitment.
        receipt_sha256: String,
    },
}

impl NormalizerContrastEvidence {
    fn value(&self) -> Result<f64, CoreError> {
        match self {
            Self::DeclaredUnverified { delta_log_z } => Ok(*delta_log_z),
            Self::EnrollmentRatesReceiptUnresolved {
                rates,
                receipt_sha256,
            } => {
                validate_sha256_reference(receipt_sha256)?;
                rates.values().curvature()
            }
            Self::SelectionModelReceiptUnresolved {
                delta_log_z,
                receipt_sha256,
            } => {
                validate_sha256_reference(receipt_sha256)?;
                Ok(*delta_log_z)
            }
        }
    }
}

/// Interval for source curvature under a declared `|Δ_AB log π| ≤ Γ`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GammaSensitivity {
    /// Observed selected-law curvature `κ̃`.
    observed_kappa: f64,
    /// Declared inclusion-interaction budget.
    gamma: f64,
    /// Provenance semantics for the normalizer contrast.
    normalizer_contrast: NormalizerContrastEvidence,
    /// Lower endpoint of the source-κ interval.
    source_kappa_low: f64,
    /// Upper endpoint of the source-κ interval.
    source_kappa_high: f64,
    /// Smallest `|Δ_AB log π|` that would make source κ zero.
    min_abs_selection_interaction_to_null: f64,
    /// Always diagnostic. Never Ready.
    authority: SensitivityAuthority,
}

impl GammaSensitivity {
    /// Lower endpoint under the declared unverified normalizer contrast.
    #[must_use]
    pub fn source_kappa_low(&self) -> f64 {
        self.source_kappa_low
    }

    /// Upper endpoint under the declared unverified normalizer contrast.
    #[must_use]
    pub fn source_kappa_high(&self) -> f64 {
        self.source_kappa_high
    }

    /// Minimum declared inclusion interaction needed to make source curvature zero.
    #[must_use]
    pub fn min_abs_selection_interaction_to_null(&self) -> f64 {
        self.min_abs_selection_interaction_to_null
    }

    /// Authority ceiling. Always diagnostic only.
    #[must_use]
    pub fn authority(&self) -> SensitivityAuthority {
        self.authority
    }
}

/// Maps an observed selected curvature through a declared Γ budget.
///
/// `κ = κ̃ - Δ_AB log π + Δ_AB log Z` with `|Δ_AB log π| ≤ gamma`.
pub fn gamma_sensitivity(
    observed_kappa: f64,
    gamma: f64,
    normalizer_contrast: NormalizerContrastEvidence,
) -> Result<GammaSensitivity, CoreError> {
    let delta_log_z = normalizer_contrast.value()?;
    finite("observed_kappa", observed_kappa)?;
    finite("gamma", gamma)?;
    finite("delta_log_z", delta_log_z)?;
    if gamma < 0.0 {
        return Err(CoreError::Negative {
            name: "gamma",
            value: gamma,
        });
    }
    let center = observed_kappa + delta_log_z;
    finite("observed_kappa_plus_delta_log_z", center)?;
    let source_kappa_low = center - gamma;
    let source_kappa_high = center + gamma;
    finite("source_kappa_low", source_kappa_low)?;
    finite("source_kappa_high", source_kappa_high)?;
    Ok(GammaSensitivity {
        observed_kappa,
        gamma,
        normalizer_contrast,
        source_kappa_low,
        source_kappa_high,
        min_abs_selection_interaction_to_null: center.abs(),
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
        return Err(CoreError::OutsideUnitInterval { name, value });
    }
    Ok(())
}

fn validate_probability_square(value: DensitySquare) -> Result<(), CoreError> {
    inclusion_probability("square p0", value.p0)?;
    inclusion_probability("square pa", value.pa)?;
    inclusion_probability("square pb", value.pb)?;
    inclusion_probability("square pab", value.pab)
}

fn validate_sha256_reference(value: &str) -> Result<(), CoreError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(CoreError::InvalidEvidenceReference);
    };
    if hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(CoreError::InvalidEvidenceReference)
    }
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

    fn constant_square(value: f64) -> DensitySquare {
        DensitySquare {
            p0: value,
            pa: value,
            pb: value,
            pab: value,
        }
    }

    fn inclusion(value: f64) -> InclusionProbabilitySquare {
        InclusionProbabilitySquare::try_from(constant_square(value)).unwrap()
    }

    fn rates(value: f64) -> InclusionRateSquare {
        InclusionRateSquare::try_from(constant_square(value)).unwrap()
    }

    fn declared_normalizer(delta_log_z: f64) -> NormalizerContrastEvidence {
        NormalizerContrastEvidence::DeclaredUnverified { delta_log_z }
    }

    #[test]
    fn two_invalid_factors_cannot_produce_a_valid_selected_law() {
        // `DensitySquare` has public fields, so nothing stops a caller supplying a
        // negative source. Before source validation, pairing it with an equally invalid
        // negative inclusion could produce a positive selected-law value. Inclusion is
        // now unrepresentable without probability validation, and source still fails.
        let transport = SelectionTransport {
            source: constant_square(-0.5),
            inclusion: inclusion(0.4),
            normalizers: rates(0.25),
        };
        let error = transport.selected().unwrap_err();
        assert!(matches!(error, CoreError::NonPositive { .. }));
    }

    #[test]
    fn an_inclusion_probability_above_one_is_refused() {
        // pi > 1 is not a large probability; a selected law built from one reports mass
        // no sampling process could have produced.
        let invalid = InclusionProbabilitySquare::try_from(DensitySquare {
            p0: 1.5,
            pa: 0.5,
            pb: 0.5,
            pab: 0.5,
        });
        assert!(invalid.is_err());
    }

    #[test]
    fn a_valid_selection_transport_still_succeeds() {
        let transport = SelectionTransport {
            source: constant_square(0.25),
            inclusion: inclusion(0.5),
            normalizers: rates(0.5),
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
            source: constant_square(0.25),
            inclusion: InclusionProbabilitySquare::try_from(DensitySquare {
                p0: 0.8,
                pa: 0.2,
                pb: 0.2,
                pab: 0.8,
            })
            .unwrap(),
            normalizers: rates(1.0),
        };
        assert!(transport.identity_residual().unwrap().abs() < 1e-14);
        let selected = transport.selected().unwrap();
        assert!(selected.curvature().unwrap().abs() > 1.0);
        let observed = selected.curvature().unwrap();
        let needed = gamma_sensitivity(observed, 0.0, declared_normalizer(0.0))
            .unwrap()
            .min_abs_selection_interaction_to_null();
        let interval = gamma_sensitivity(observed, needed, declared_normalizer(0.0)).unwrap();
        assert_eq!(interval.authority(), SensitivityAuthority::DiagnosticOnly);
        assert!(interval.source_kappa_low() <= 0.0);
        assert!(interval.source_kappa_high() >= 0.0);
        assert!(needed > 0.0);
    }

    #[test]
    fn selection_transport_rejects_invalid_factors_and_overflow() {
        let negative_pair = SelectionTransport {
            source: constant_square(-1.0),
            inclusion: inclusion(1.0),
            normalizers: rates(1.0),
        };
        assert!(negative_pair.selected().is_err());

        let invalid_probability = InclusionProbabilitySquare::try_from(DensitySquare {
            p0: 2.0,
            pa: 1.0,
            pb: 1.0,
            pab: 1.0,
        });
        assert!(invalid_probability.is_err());

        let overflow = SelectionTransport {
            source: DensitySquare {
                p0: f64::MAX,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            },
            inclusion: inclusion(1.0),
            normalizers: InclusionRateSquare::try_from(DensitySquare {
                p0: f64::MIN_POSITIVE,
                pa: 1.0,
                pb: 1.0,
                pab: 1.0,
            })
            .unwrap(),
        };
        assert!(overflow.selected().is_err());
    }

    #[test]
    fn gamma_sensitivity_never_claims_ready_and_rejects_negative_budget() {
        let interval = gamma_sensitivity(0.4, 0.1, declared_normalizer(0.0)).unwrap();
        assert_eq!(interval.authority(), SensitivityAuthority::DiagnosticOnly);
        assert!((interval.source_kappa_low() - 0.3).abs() < 1e-15);
        assert!((interval.source_kappa_high() - 0.5).abs() < 1e-15);
        assert!(matches!(
            gamma_sensitivity(0.1, -0.2, declared_normalizer(0.0)),
            Err(CoreError::Negative { name: "gamma", .. })
        ));
        assert!(matches!(
            gamma_sensitivity(f64::MAX, f64::MAX, declared_normalizer(0.0)),
            Err(CoreError::NonFinite {
                name: "source_kappa_high",
                ..
            })
        ));
    }

    #[test]
    fn enrollment_rates_derive_the_normalizer_contrast_but_remain_unresolved() {
        let rates = InclusionRateSquare::try_from(DensitySquare {
            p0: 0.8,
            pa: 0.4,
            pb: 0.2,
            pab: 0.4,
        })
        .unwrap();
        let evidence = NormalizerContrastEvidence::EnrollmentRatesReceiptUnresolved {
            rates,
            receipt_sha256:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        };
        let delta = rates.values().curvature().unwrap();
        let interval = gamma_sensitivity(0.5, 0.0, evidence).unwrap();
        assert!((interval.source_kappa_low() - (0.5 + delta)).abs() < 1e-14);
        assert_eq!(interval.authority(), SensitivityAuthority::DiagnosticOnly);

        let malformed = NormalizerContrastEvidence::SelectionModelReceiptUnresolved {
            delta_log_z: 0.0,
            receipt_sha256: "not-a-digest".into(),
        };
        assert_eq!(
            gamma_sensitivity(0.5, 0.1, malformed).unwrap_err(),
            CoreError::InvalidEvidenceReference
        );
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
