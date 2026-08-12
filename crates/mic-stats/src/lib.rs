#![forbid(unsafe_code)]
//! Reference inference primitives shared by localization, orientation, and curvature tests.

use mic_core::compensated_sum;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Statistical primitive errors.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum StatsError {
    /// A vector length mismatch.
    #[error("all input vectors must have equal nonzero length")]
    Shape,
    /// A probability or bandwidth was invalid.
    #[error("{name} must be finite and in the required range, got {value}")]
    Invalid {
        /// Name of the offending input.
        name: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// A matrix was ragged or had incompatible dimensions.
    #[error("feature matrix is ragged or incompatible")]
    MatrixShape,
    /// A required evidence identifier or fingerprint was malformed.
    #[error("{name} must be a nonempty audit id or sha256:<64 lowercase hex> fingerprint")]
    InvalidEvidenceReference {
        /// Name of the malformed reference.
        name: &'static str,
    },
}

/// Public authority grade attached to GCM design evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductDesignGrade {
    /// Corner masses were checked directly and have product pooled odds.
    ProductOddsVerified,
    /// Observations reference a completed product-design reweighting audit.
    ReweightedToProduct,
    /// Residual-product output has no certificate authority.
    DiagnosticOnly,
}

/// Auditable, self-validating design evidence attached to every GCM projection.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct ProductDesignEvidence {
    kind: ProductDesignEvidenceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "grade", rename_all = "snake_case")]
enum ProductDesignEvidenceKind {
    /// Corner masses were checked directly and have product pooled odds.
    ProductOddsVerified {
        /// Stable identifier of the completed sampling-odds audit.
        audit_id: String,
        /// SHA-256 fingerprint of the declared allocation contract audited.
        source_fingerprint: String,
        /// Positive masses in `00, 10, 01, 11` order.
        probabilities: [f64; 4],
        /// Sum of the supplied masses.
        probability_sum: f64,
        /// Verified pooled log odds ratio.
        log_odds_ratio: f64,
        /// Absolute tolerance used for the product-odds decision.
        tolerance: f64,
    },
    /// Observations were reweighted and a completed product-design audit exists.
    ReweightedToProduct {
        /// Stable identifier of the completed reweighting audit and diagnostics.
        audit_id: String,
        /// SHA-256 fingerprint of the completed reweighting-audit artifact.
        source_fingerprint: String,
    },
    /// A residual-product diagnostic with no certificate authority.
    DiagnosticOnly {
        /// Human-readable reason why product-design eligibility is not claimed.
        reason: String,
    },
}

impl<'de> Deserialize<'de> for ProductDesignEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let kind = ProductDesignEvidenceKind::deserialize(deserializer)?;
        match kind {
            ProductDesignEvidenceKind::ProductOddsVerified {
                audit_id,
                source_fingerprint,
                probabilities,
                probability_sum,
                log_odds_ratio,
                tolerance,
            } => {
                let verified = Self::from_sampling_odds_audit(
                    audit_id,
                    source_fingerprint,
                    probabilities,
                    tolerance,
                )
                .map_err(serde::de::Error::custom)?;
                let ProductDesignEvidenceKind::ProductOddsVerified {
                    probability_sum: recomputed_sum,
                    log_odds_ratio: recomputed_log_odds,
                    ..
                } = &verified.kind
                else {
                    unreachable!("corner-odds constructor returned the wrong evidence kind")
                };
                if probability_sum.to_bits() != recomputed_sum.to_bits()
                    || log_odds_ratio.to_bits() != recomputed_log_odds.to_bits()
                {
                    return Err(serde::de::Error::custom(
                        "serialized product-design diagnostics do not match recomputation",
                    ));
                }
                Ok(verified)
            }
            ProductDesignEvidenceKind::ReweightedToProduct {
                audit_id,
                source_fingerprint,
            } => Self::from_reweighting_audit(audit_id, source_fingerprint)
                .map_err(serde::de::Error::custom),
            ProductDesignEvidenceKind::DiagnosticOnly { reason } => {
                Self::diagnostic_only(reason).map_err(serde::de::Error::custom)
            }
        }
    }
}

impl ProductDesignEvidence {
    /// Records a completed sampling-odds audit and rechecks its product pooled odds.
    ///
    /// The audit identifier and source fingerprint are required because product-looking
    /// empirical corner counts do not establish a product assignment contract.
    pub fn from_sampling_odds_audit(
        audit_id: impl Into<String>,
        source_fingerprint: impl Into<String>,
        probabilities: [f64; 4],
        tolerance: f64,
    ) -> Result<Self, StatsError> {
        let audit_id = audit_id.into();
        let audit_id = validate_audit_id(&audit_id, "sampling-odds audit identifier")?;
        let source_fingerprint = source_fingerprint.into();
        let source_fingerprint =
            validate_sha256_fingerprint(&source_fingerprint, "sampling-odds source fingerprint")?;
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(StatsError::Invalid {
                name: "product-odds tolerance",
                value: tolerance,
            });
        }
        for probability in probabilities {
            if !probability.is_finite() || probability <= 0.0 {
                return Err(StatsError::Invalid {
                    name: "corner probability",
                    value: probability,
                });
            }
        }
        let probability_sum: f64 = probabilities.iter().sum();
        if !probability_sum.is_finite() {
            return Err(StatsError::Invalid {
                name: "corner probability sum",
                value: probability_sum,
            });
        }
        let [p00, p10, p01, p11] = probabilities;
        let log_odds_ratio = p11.ln() + p00.ln() - p10.ln() - p01.ln();
        if log_odds_ratio.abs() > tolerance {
            return Err(StatsError::Invalid {
                name: "non-product pooled log odds",
                value: log_odds_ratio,
            });
        }
        Ok(Self {
            kind: ProductDesignEvidenceKind::ProductOddsVerified {
                audit_id,
                source_fingerprint,
                probabilities,
                probability_sum,
                log_odds_ratio,
                tolerance,
            },
        })
    }

    /// Records a completed reweighting audit that established a product design.
    pub fn from_reweighting_audit(
        audit_id: impl Into<String>,
        source_fingerprint: impl Into<String>,
    ) -> Result<Self, StatsError> {
        let audit_id = audit_id.into();
        let audit_id = validate_audit_id(&audit_id, "reweighting audit identifier")?;
        let source_fingerprint = source_fingerprint.into();
        let source_fingerprint = validate_sha256_fingerprint(
            &source_fingerprint,
            "reweighting audit source fingerprint",
        )?;
        Ok(Self {
            kind: ProductDesignEvidenceKind::ReweightedToProduct {
                audit_id,
                source_fingerprint,
            },
        })
    }

    /// Marks an unverified residual-product projection as diagnostic-only.
    pub fn diagnostic_only(reason: impl Into<String>) -> Result<Self, StatsError> {
        let reason = reason.into();
        let reason = reason.trim().to_owned();
        if reason.is_empty() {
            return Err(StatsError::InvalidEvidenceReference {
                name: "diagnostic-only reason",
            });
        }
        Ok(Self {
            kind: ProductDesignEvidenceKind::DiagnosticOnly { reason },
        })
    }

    /// Serializable authority grade of this evidence object.
    #[must_use]
    pub const fn grade(&self) -> ProductDesignGrade {
        match &self.kind {
            ProductDesignEvidenceKind::ProductOddsVerified { .. } => {
                ProductDesignGrade::ProductOddsVerified
            }
            ProductDesignEvidenceKind::ReweightedToProduct { .. } => {
                ProductDesignGrade::ReweightedToProduct
            }
            ProductDesignEvidenceKind::DiagnosticOnly { .. } => ProductDesignGrade::DiagnosticOnly,
        }
    }

    /// Verified corner masses and tolerance, when a sampling-odds audit supplied them.
    #[must_use]
    pub const fn verified_corner_odds(&self) -> Option<([f64; 4], f64)> {
        match &self.kind {
            ProductDesignEvidenceKind::ProductOddsVerified {
                probabilities,
                tolerance,
                ..
            } => Some((*probabilities, *tolerance)),
            _ => None,
        }
    }

    /// Completed reweighting-audit identifier, when that route supplied eligibility.
    #[must_use]
    pub fn reweighting_audit_id(&self) -> Option<&str> {
        match &self.kind {
            ProductDesignEvidenceKind::ReweightedToProduct { audit_id, .. } => Some(audit_id),
            _ => None,
        }
    }

    /// Completed sampling-odds audit identifier, when direct design evidence supplied eligibility.
    #[must_use]
    pub fn sampling_odds_audit_id(&self) -> Option<&str> {
        match &self.kind {
            ProductDesignEvidenceKind::ProductOddsVerified { audit_id, .. } => Some(audit_id),
            _ => None,
        }
    }

    /// Fingerprint of the audited allocation or reweighting source artifact.
    #[must_use]
    pub fn source_fingerprint(&self) -> Option<&str> {
        match &self.kind {
            ProductDesignEvidenceKind::ProductOddsVerified {
                source_fingerprint, ..
            }
            | ProductDesignEvidenceKind::ReweightedToProduct {
                source_fingerprint, ..
            } => Some(source_fingerprint),
            ProductDesignEvidenceKind::DiagnosticOnly { .. } => None,
        }
    }

    /// Whether the evidence can support a certificate-grade GCM projection.
    #[must_use]
    pub const fn is_certificate_eligible(&self) -> bool {
        !matches!(self.grade(), ProductDesignGrade::DiagnosticOnly)
    }
}

fn validate_audit_id(value: &str, name: &'static str) -> Result<String, StatsError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(StatsError::InvalidEvidenceReference { name });
    }
    Ok(value)
}

fn validate_sha256_fingerprint(value: &str, name: &'static str) -> Result<String, StatsError> {
    let value = value.trim().to_owned();
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(StatsError::InvalidEvidenceReference { name });
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(StatsError::InvalidEvidenceReference { name });
    }
    Ok(value)
}

/// Studentized projected generalized covariance estimate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GcmEstimate {
    /// Mean weighted residual product.
    pub estimate: f64,
    /// Estimated standard error of the mean.
    pub standard_error: f64,
    /// Studentized statistic.
    pub z_score: f64,
    /// Number of observations.
    pub sample_size: usize,
    /// Product-design or diagnostic authority attached to this projection.
    pub design_evidence: ProductDesignEvidence,
}

/// Computes a cross-fitted weighted residual-product projection with explicit design evidence.
///
/// Product-odds evidence is recomputed by
/// [`ProductDesignEvidence::from_sampling_odds_audit`] and bound to the completed
/// audit plus its source artifact. Reweighted evidence has the same provenance
/// requirement. A diagnostic-only projection remains useful for debugging but
/// is serializably ineligible for certificate use.
pub fn gcm_projection(
    design_evidence: &ProductDesignEvidence,
    a: &[f64],
    b: &[f64],
    mean_a: &[f64],
    mean_b: &[f64],
    witness: &[f64],
) -> Result<GcmEstimate, StatsError> {
    let n = a.len();
    if n == 0
        || [b.len(), mean_a.len(), mean_b.len(), witness.len()]
            .iter()
            .any(|&m| m != n)
    {
        return Err(StatsError::Shape);
    }
    let terms: Vec<f64> = (0..n)
        .map(|i| witness[i] * (a[i] - mean_a[i]) * (b[i] - mean_b[i]))
        .collect();
    if terms.iter().any(|value| !value.is_finite()) {
        return Err(StatsError::Invalid {
            name: "residual product",
            value: f64::NAN,
        });
    }
    let estimate = compensated_sum(&terms) / n as f64;
    let centered_sq: Vec<f64> = terms
        .iter()
        .map(|value| (value - estimate).powi(2))
        .collect();
    let variance = if n > 1 {
        compensated_sum(&centered_sq) / (n - 1) as f64
    } else {
        0.0
    };
    let standard_error = (variance / n as f64).sqrt();
    let z_score = if standard_error > 0.0 {
        estimate / standard_error
    } else {
        0.0
    };
    Ok(GcmEstimate {
        estimate,
        standard_error,
        z_score,
        sample_size: n,
        design_evidence: design_evidence.clone(),
    })
}

/// Kish effective sample size for nonnegative weights.
pub fn effective_sample_size(weights: &[f64]) -> Result<f64, StatsError> {
    if weights.is_empty()
        || weights
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(StatsError::Invalid {
            name: "weight",
            value: f64::NAN,
        });
    }
    let sum = compensated_sum(weights);
    let squares: Vec<f64> = weights.iter().map(|value| value * value).collect();
    let sum_sq = compensated_sum(&squares);
    Ok(if sum_sq > 0.0 {
        sum * sum / sum_sq
    } else {
        0.0
    })
}

/// Unbiased squared maximum mean discrepancy using an RBF kernel.
pub fn mmd2_unbiased(
    left: &[Vec<f64>],
    right: &[Vec<f64>],
    bandwidth: f64,
) -> Result<f64, StatsError> {
    validate_matrix_pair(left, right)?;
    if !bandwidth.is_finite() || bandwidth <= 0.0 {
        return Err(StatsError::Invalid {
            name: "bandwidth",
            value: bandwidth,
        });
    }
    let n = left.len();
    let m = right.len();
    if n < 2 || m < 2 {
        return Err(StatsError::Shape);
    }
    let gamma = 1.0 / (2.0 * bandwidth * bandwidth);
    let mut xx = 0.0;
    for i in 0..n {
        for j in 0..n {
            if i != j {
                xx += (-gamma * squared_distance(&left[i], &left[j])).exp();
            }
        }
    }
    let mut yy = 0.0;
    for i in 0..m {
        for j in 0..m {
            if i != j {
                yy += (-gamma * squared_distance(&right[i], &right[j])).exp();
            }
        }
    }
    let mut xy = 0.0;
    for x in left {
        for y in right {
            xy += (-gamma * squared_distance(x, y)).exp();
        }
    }
    Ok(xx / (n * (n - 1)) as f64 + yy / (m * (m - 1)) as f64 - 2.0 * xy / (n * m) as f64)
}

/// Sample energy distance between two multivariate samples.
pub fn energy_distance(left: &[Vec<f64>], right: &[Vec<f64>]) -> Result<f64, StatsError> {
    validate_matrix_pair(left, right)?;
    let n = left.len();
    let m = right.len();
    if n == 0 || m == 0 {
        return Err(StatsError::Shape);
    }
    let mut xy = 0.0;
    for x in left {
        for y in right {
            xy += squared_distance(x, y).sqrt();
        }
    }
    let mut xx = 0.0;
    for x in left {
        for y in left {
            xx += squared_distance(x, y).sqrt();
        }
    }
    let mut yy = 0.0;
    for x in right {
        for y in right {
            yy += squared_distance(x, y).sqrt();
        }
    }
    Ok(2.0 * xy / (n * m) as f64 - xx / (n * n) as f64 - yy / (m * m) as f64)
}

/// Relative deletion discrepancy used by the equivalence state machine.
pub fn relative_discrepancy(deletion: f64, full: f64, stabilizer: f64) -> Result<f64, StatsError> {
    if !deletion.is_finite() || deletion < 0.0 {
        return Err(StatsError::Invalid {
            name: "deletion discrepancy",
            value: deletion,
        });
    }
    if !full.is_finite() || full < 0.0 {
        return Err(StatsError::Invalid {
            name: "full discrepancy",
            value: full,
        });
    }
    if !stabilizer.is_finite() || stabilizer <= 0.0 {
        return Err(StatsError::Invalid {
            name: "stabilizer",
            value: stabilizer,
        });
    }
    Ok(deletion / (full + stabilizer))
}

/// Small deterministic generator used only for reference bootstrap and fixtures.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Creates a deterministic generator.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64-bit value.
    #[must_use]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `0..upper` using rejection sampling.
    pub fn index(&mut self, upper: usize) -> Result<usize, StatsError> {
        if upper == 0 {
            return Err(StatsError::Shape);
        }
        let upper64 = upper as u64;
        let zone = u64::MAX - u64::MAX % upper64;
        loop {
            let value = self.next_u64();
            if value < zone {
                return usize::try_from(value % upper64).map_err(|_| StatsError::Shape);
            }
        }
    }
}

/// One candidate localized support scored on held-out data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateSupport {
    /// State-variable names in the candidate support.
    pub variables: Vec<String>,
    /// Held-out proper regime-prediction loss for a model fit on this support.
    pub holdout_loss: f64,
    /// Nonnegative learner-specific complexity used only to break support-cardinality ties.
    pub complexity: f64,
}

/// Parsimony-frontier summary of a completed localization ensemble.
///
/// The frontier is the set of candidates whose held-out loss is within
/// `loss_tolerance` of the best loss in the ensemble, ordered first by support
/// cardinality and then by learner-specific complexity.  Under ratio
/// faithfulness and adequate learner capacity, locality makes the true support
/// the smallest support carrying full regime information, so the
/// smallest-cardinality frontier member is the preferred localization
/// proposal.  Inclusion frequencies are descriptive frequencies over the
/// designed candidate ensemble, not probabilities, and any certificate-grade
/// conclusion about an adaptively selected support requires an outer held-out
/// confirmation sample.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsimonyFrontier {
    /// Best held-out loss over the completed ensemble.
    pub best_loss: f64,
    /// Absolute loss tolerance defining the frontier.
    pub loss_tolerance: f64,
    /// Indices ordered by cardinality, learner complexity, loss, then input index.
    pub frontier: Vec<usize>,
    /// Variables of the smallest-cardinality frontier member.
    pub minimal_support: Vec<String>,
    /// Fraction of frontier members containing each variable, normalized per variable.
    pub inclusion_frequencies: std::collections::BTreeMap<String, f64>,
}

/// Computes the parsimony frontier of a completed localization ensemble.
///
/// The frontier threshold is computed once over the full candidate set; it is
/// never updated incrementally while results accumulate, so the output is
/// invariant to the order in which candidates were produced.
pub fn parsimony_frontier(
    candidates: &[CandidateSupport],
    loss_tolerance: f64,
) -> Result<ParsimonyFrontier, StatsError> {
    if candidates.is_empty() {
        return Err(StatsError::Shape);
    }
    if !loss_tolerance.is_finite() || loss_tolerance < 0.0 {
        return Err(StatsError::Invalid {
            name: "loss tolerance",
            value: loss_tolerance,
        });
    }
    for candidate in candidates {
        if !candidate.holdout_loss.is_finite() {
            return Err(StatsError::Invalid {
                name: "holdout loss",
                value: candidate.holdout_loss,
            });
        }
        if !candidate.complexity.is_finite() || candidate.complexity < 0.0 {
            return Err(StatsError::Invalid {
                name: "complexity",
                value: candidate.complexity,
            });
        }
        if candidate
            .variables
            .iter()
            .any(|name| name.trim().is_empty())
        {
            return Err(StatsError::Invalid {
                name: "variable name",
                value: f64::NAN,
            });
        }
        let mut unique = std::collections::BTreeSet::new();
        if !candidate.variables.iter().all(|name| unique.insert(name)) {
            return Err(StatsError::Invalid {
                name: "duplicate variable",
                value: f64::NAN,
            });
        }
    }
    let best_loss = candidates
        .iter()
        .map(|candidate| candidate.holdout_loss)
        .fold(f64::INFINITY, f64::min);
    let threshold = best_loss + loss_tolerance;
    let mut frontier: Vec<usize> = (0..candidates.len())
        .filter(|&index| candidates[index].holdout_loss <= threshold)
        .collect();
    frontier.sort_by(|&left, &right| {
        candidates[left]
            .variables
            .len()
            .cmp(&candidates[right].variables.len())
            .then(
                candidates[left]
                    .complexity
                    .total_cmp(&candidates[right].complexity),
            )
            .then(
                candidates[left]
                    .holdout_loss
                    .total_cmp(&candidates[right].holdout_loss),
            )
            .then(left.cmp(&right))
    });
    let minimal_support = {
        let mut variables = candidates[frontier[0]].variables.clone();
        variables.sort();
        variables
    };
    let mut inclusion_frequencies = std::collections::BTreeMap::new();
    let frontier_size = frontier.len() as f64;
    for &index in &frontier {
        for name in &candidates[index].variables {
            *inclusion_frequencies.entry(name.clone()).or_insert(0.0) += 1.0;
        }
    }
    for value in inclusion_frequencies.values_mut() {
        *value /= frontier_size;
    }
    Ok(ParsimonyFrontier {
        best_loss,
        loss_tolerance,
        frontier,
        minimal_support,
        inclusion_frequencies,
    })
}

/// Tri-state result of one deletion-equivalence comparison.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EquivalenceStatus {
    /// The simultaneous upper bound is below the equivalence tolerance.
    CertifiedInvariant,
    /// The simultaneous lower bound is above the equivalence tolerance.
    CertifiedChanged,
    /// The simultaneous interval overlaps the equivalence boundary.
    Undetermined,
}

/// One classified deletion hypothesis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeletionEquivalence {
    /// Deleted coordinate name.
    pub variable: String,
    /// Point estimate of the relative deletion discrepancy.
    pub relative_discrepancy: f64,
    /// Simultaneous lower confidence bound.
    pub lower: f64,
    /// Simultaneous upper confidence bound.
    pub upper: f64,
    /// Equivalence tolerance the bounds were compared against.
    pub epsilon: f64,
    /// Tri-state classification.
    pub status: EquivalenceStatus,
}

/// Classifies one deletion from simultaneous bounds and a preregistered tolerance.
///
/// Failure to reject equality is never treated as evidence of invariance: a
/// deletion is certified invariant only when its entire simultaneous interval
/// lies below `epsilon`, certified changed only when the interval lies above,
/// and undetermined otherwise.
pub fn classify_deletion(
    variable: impl Into<String>,
    relative_discrepancy: f64,
    lower: f64,
    upper: f64,
    epsilon: f64,
) -> Result<DeletionEquivalence, StatsError> {
    let variable = variable.into();
    let variable = variable.trim().to_owned();
    if variable.is_empty() {
        return Err(StatsError::Invalid {
            name: "deletion variable",
            value: f64::NAN,
        });
    }
    for (name, value) in [
        ("relative discrepancy", relative_discrepancy),
        ("lower bound", lower),
        ("upper bound", upper),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(StatsError::Invalid { name, value });
        }
    }
    if !epsilon.is_finite() || epsilon <= 0.0 {
        return Err(StatsError::Invalid {
            name: "equivalence tolerance",
            value: epsilon,
        });
    }
    if lower > relative_discrepancy || relative_discrepancy > upper {
        return Err(StatsError::Invalid {
            name: "bound ordering",
            value: relative_discrepancy,
        });
    }
    let status = if upper < epsilon {
        EquivalenceStatus::CertifiedInvariant
    } else if lower > epsilon {
        EquivalenceStatus::CertifiedChanged
    } else {
        EquivalenceStatus::Undetermined
    };
    Ok(DeletionEquivalence {
        variable,
        relative_discrepancy,
        lower,
        upper,
        epsilon,
        status,
    })
}

/// Five-state orientation outcome of the deletion pass-count audit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum OrientationOutcome {
    /// Exactly one deletion is certified invariant and every competitor is certified changed.
    UniqueTarget {
        /// The oriented target coordinate.
        target: String,
    },
    /// No deletion is certified invariant and every deletion is certified changed.
    NoPass,
    /// More than one deletion is certified invariant.
    MultiplePasses {
        /// All certified-invariant coordinates.
        passes: Vec<String>,
    },
    /// The full-support intervention discrepancy is below the power threshold.
    Underpowered,
    /// At least one simultaneous interval overlaps the equivalence boundary.
    Undetermined {
        /// Coordinates whose classification is unresolved.
        unresolved: Vec<String>,
    },
}

/// Runs the pass-count state machine over classified deletions.
///
/// Precedence is conservative: an underpowered intervention abstains before any
/// counting; two certified passes are reported as multiple passes even if other
/// deletions are unresolved, because the ambiguity is already fatal; any
/// remaining unresolved deletion blocks orientation.  Only the unique-target
/// state orients a family.
pub fn orient_from_deletions(
    deletions: &[DeletionEquivalence],
    full_discrepancy: f64,
    min_full_discrepancy: f64,
) -> Result<OrientationOutcome, StatsError> {
    if deletions.is_empty() {
        return Err(StatsError::Shape);
    }
    let epsilon = deletions[0].epsilon;
    if deletions
        .iter()
        .any(|deletion| deletion.epsilon.to_bits() != epsilon.to_bits())
    {
        return Err(StatsError::Invalid {
            name: "mixed equivalence tolerances",
            value: f64::NAN,
        });
    }
    if !full_discrepancy.is_finite() || full_discrepancy < 0.0 {
        return Err(StatsError::Invalid {
            name: "full discrepancy",
            value: full_discrepancy,
        });
    }
    if !min_full_discrepancy.is_finite() || min_full_discrepancy < 0.0 {
        return Err(StatsError::Invalid {
            name: "minimum full discrepancy",
            value: min_full_discrepancy,
        });
    }
    let mut unique = std::collections::BTreeSet::new();
    if !deletions
        .iter()
        .all(|deletion| unique.insert(deletion.variable.as_str()))
    {
        return Err(StatsError::Invalid {
            name: "duplicate deletion variable",
            value: f64::NAN,
        });
    }
    if full_discrepancy < min_full_discrepancy {
        return Ok(OrientationOutcome::Underpowered);
    }
    let passes: Vec<String> = deletions
        .iter()
        .filter(|deletion| deletion.status == EquivalenceStatus::CertifiedInvariant)
        .map(|deletion| deletion.variable.clone())
        .collect();
    let unresolved: Vec<String> = deletions
        .iter()
        .filter(|deletion| deletion.status == EquivalenceStatus::Undetermined)
        .map(|deletion| deletion.variable.clone())
        .collect();
    if passes.len() > 1 {
        return Ok(OrientationOutcome::MultiplePasses { passes });
    }
    if !unresolved.is_empty() {
        return Ok(OrientationOutcome::Undetermined { unresolved });
    }
    match passes.into_iter().next() {
        Some(target) => Ok(OrientationOutcome::UniqueTarget { target }),
        None => Ok(OrientationOutcome::NoPass),
    }
}

/// Reference simultaneous multiplier bounds for a vector of cluster-mean statistics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimultaneousBounds {
    /// Per-statistic cluster means.
    pub means: Vec<f64>,
    /// Per-statistic standard errors of the cluster mean.
    pub standard_errors: Vec<f64>,
    /// Max-statistic critical value shared by every coordinate.
    pub critical_value: f64,
    /// Simultaneous lower bounds, floored at zero for nonnegative statistics.
    pub lower: Vec<f64>,
    /// Simultaneous upper bounds.
    pub upper: Vec<f64>,
    /// Number of multiplier replicates.
    pub replicates: usize,
    /// Nominal simultaneous coverage level in (0, 1).
    pub confidence: f64,
    /// Deterministic seed recorded for the evidence ledger.
    pub seed: u64,
}

/// Deterministic Rademacher multiplier bootstrap for reference simultaneous mean bounds.
///
/// `contributions` has one row per cluster and one column per statistic; the
/// randomization unit must be the row unit.  Each replicate flips cluster signs
/// with Rademacher multipliers drawn from a seeded [`SplitMix64`], the max over
/// standard-error-scaled coordinates forms the replicate statistic, and the
/// empirical `confidence` quantile becomes one shared critical value, so the
/// bounds are simultaneous across coordinates.  This reference primitive covers
/// mean-type discrepancy vectors; it is not a finite-sample coverage guarantee,
/// and degenerate U-statistic corrections remain a Packet 2 item. Lower bounds
/// are floored at zero because the intended consumers are nonnegative discrepancies.
pub fn simultaneous_mean_bounds(
    contributions: &[Vec<f64>],
    replicates: usize,
    confidence: f64,
    seed: u64,
) -> Result<SimultaneousBounds, StatsError> {
    if contributions.len() < 2 {
        return Err(StatsError::Shape);
    }
    let width = contributions[0].len();
    if width == 0
        || contributions
            .iter()
            .any(|row| row.len() != width || row.iter().any(|value| !value.is_finite()))
    {
        return Err(StatsError::MatrixShape);
    }
    if replicates == 0 {
        return Err(StatsError::Invalid {
            name: "replicates",
            value: 0.0,
        });
    }
    if !confidence.is_finite() || confidence <= 0.0 || confidence >= 1.0 {
        return Err(StatsError::Invalid {
            name: "confidence",
            value: confidence,
        });
    }
    let clusters = contributions.len();
    let count = clusters as f64;
    let means: Vec<f64> = (0..width)
        .map(|column| {
            let column_values: Vec<f64> = contributions.iter().map(|row| row[column]).collect();
            compensated_sum(&column_values) / count
        })
        .collect();
    let centered: Vec<Vec<f64>> = contributions
        .iter()
        .map(|row| {
            row.iter()
                .zip(&means)
                .map(|(&value, &mean)| value - mean)
                .collect()
        })
        .collect();
    let standard_errors: Vec<f64> = (0..width)
        .map(|column| {
            let squares: Vec<f64> = centered
                .iter()
                .map(|row| row[column] * row[column])
                .collect();
            (compensated_sum(&squares) / (count - 1.0) / count).sqrt()
        })
        .collect();
    let mut max_statistics =
        multiplier_max_statistics(&centered, &standard_errors, replicates, seed);
    max_statistics.sort_by(f64::total_cmp);
    let critical_value = max_statistics[quantile_rank(confidence, replicates)];
    let lower: Vec<f64> = means
        .iter()
        .zip(&standard_errors)
        .map(|(&mean, &se)| (mean - critical_value * se).max(0.0))
        .collect();
    let upper: Vec<f64> = means
        .iter()
        .zip(&standard_errors)
        .map(|(&mean, &se)| mean + critical_value * se)
        .collect();
    Ok(SimultaneousBounds {
        means,
        standard_errors,
        critical_value,
        lower,
        upper,
        replicates,
        confidence,
        seed,
    })
}

/// One max-over-coordinates Rademacher multiplier statistic per replicate.
fn multiplier_max_statistics(
    centered: &[Vec<f64>],
    standard_errors: &[f64],
    replicates: usize,
    seed: u64,
) -> Vec<f64> {
    let clusters = centered.len();
    let count = clusters as f64;
    let mut generator = SplitMix64::new(seed);
    let mut max_statistics = Vec::with_capacity(replicates);
    for _ in 0..replicates {
        let multipliers: Vec<f64> = (0..clusters)
            .map(|_| {
                if generator.next_u64() & 1 == 1 {
                    1.0
                } else {
                    -1.0
                }
            })
            .collect();
        let mut replicate_max = 0.0_f64;
        for (column, &standard_error) in standard_errors.iter().enumerate() {
            if standard_error <= 0.0 {
                continue;
            }
            let terms: Vec<f64> = centered
                .iter()
                .zip(&multipliers)
                .map(|(row, &multiplier)| multiplier * row[column])
                .collect();
            let perturbed = compensated_sum(&terms) / count;
            replicate_max = replicate_max.max(perturbed.abs() / standard_error);
        }
        max_statistics.push(replicate_max);
    }
    max_statistics
}

/// Zero-based index of the empirical `confidence` quantile among `replicates` sorted values.
fn quantile_rank(confidence: f64, replicates: usize) -> usize {
    // The ceiling is bounded above by `replicates`, so the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rank = (confidence * replicates as f64).ceil() as usize;
    rank.clamp(1, replicates) - 1
}

fn validate_matrix_pair(left: &[Vec<f64>], right: &[Vec<f64>]) -> Result<(), StatsError> {
    if left.is_empty() || right.is_empty() {
        return Err(StatsError::Shape);
    }
    let dimension = left[0].len();
    if dimension == 0
        || left
            .iter()
            .chain(right)
            .any(|row| row.len() != dimension || row.iter().any(|value| !value.is_finite()))
    {
        return Err(StatsError::MatrixShape);
    }
    Ok(())
}

fn squared_distance(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(&x, &y)| (x - y).powi(2)).sum()
}

/// Feature-gated marker proving the Franken numerical adapters were selected.
#[cfg(feature = "franken")]
pub mod franken {
    /// Returns the pinned integration family used by this build.
    #[must_use]
    pub const fn backend_name() -> &'static str {
        "FrankenNumPy + FrankenSciPy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_FINGERPRINT: &str =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn gcm_zero_when_one_residual_is_zero() {
        let evidence = ProductDesignEvidence::from_sampling_odds_audit(
            "sampling-audit-1",
            SOURCE_FINGERPRINT,
            [0.25; 4],
            1e-12,
        )
        .unwrap();
        let estimate = gcm_projection(
            &evidence,
            &[0.0, 1.0, 0.0, 1.0],
            &[0.0, 0.0, 1.0, 1.0],
            &[0.0, 1.0, 0.0, 1.0],
            &[0.5; 4],
            &[1.0; 4],
        )
        .unwrap();
        assert_eq!(estimate.estimate, 0.0);
        assert!(estimate.design_evidence.is_certificate_eligible());
    }

    #[test]
    fn nonproduct_odds_cannot_create_verified_gcm_evidence() {
        let error = ProductDesignEvidence::from_sampling_odds_audit(
            "sampling-audit-1",
            SOURCE_FINGERPRINT,
            [0.1, 0.2, 0.3, 0.4],
            1e-12,
        )
        .unwrap_err();
        assert!(matches!(error, StatsError::Invalid { .. }));
        assert!(
            ProductDesignEvidence::from_sampling_odds_audit(
                "sampling-audit-1",
                SOURCE_FINGERPRINT,
                [f64::MAX; 4],
                1e-12,
            )
            .is_err(),
            "nonfinite recorded diagnostics must fail closed"
        );
    }

    #[test]
    fn product_looking_odds_require_audited_provenance() {
        assert!(matches!(
            ProductDesignEvidence::from_sampling_odds_audit(
                "",
                SOURCE_FINGERPRINT,
                [0.25; 4],
                1e-12,
            )
            .unwrap_err(),
            StatsError::InvalidEvidenceReference { .. }
        ));
        assert!(matches!(
            ProductDesignEvidence::from_sampling_odds_audit(
                "sampling-audit-1",
                "empirical-counts",
                [0.25; 4],
                1e-12,
            )
            .unwrap_err(),
            StatsError::InvalidEvidenceReference { .. }
        ));
    }

    #[test]
    fn diagnostic_gcm_is_serializably_ineligible() {
        let evidence =
            ProductDesignEvidence::diagnostic_only("sampling audit unavailable").unwrap();
        let estimate = gcm_projection(
            &evidence,
            &[0.0, 1.0],
            &[0.0, 1.0],
            &[0.5, 0.5],
            &[0.5, 0.5],
            &[1.0, 1.0],
        )
        .unwrap();
        assert!(!estimate.design_evidence.is_certificate_eligible());
        let encoded = serde_json::to_string(&estimate).unwrap();
        assert!(encoded.contains("diagnostic_only"));
    }

    #[test]
    fn completed_reweighting_audit_is_certificate_eligible() {
        let evidence =
            ProductDesignEvidence::from_reweighting_audit(" weights-audit-17 ", SOURCE_FINGERPRINT)
                .unwrap();
        assert!(evidence.is_certificate_eligible());
        assert_eq!(evidence.grade(), ProductDesignGrade::ReweightedToProduct);
        assert_eq!(evidence.reweighting_audit_id(), Some("weights-audit-17"));
        assert_eq!(evidence.source_fingerprint(), Some(SOURCE_FINGERPRINT));
    }

    #[test]
    fn deserialization_revalidates_product_odds_and_diagnostics() {
        let forged_provenance = r#"{
            "grade":"product_odds_verified",
            "audit_id":"sampling-audit-1",
            "source_fingerprint":"empirical-counts",
            "probabilities":[0.25,0.25,0.25,0.25],
            "probability_sum":1.0,
            "log_odds_ratio":0.0,
            "tolerance":1e-12
        }"#;
        assert!(serde_json::from_str::<ProductDesignEvidence>(forged_provenance).is_err());

        let forged_nonproduct = r#"{
            "grade":"product_odds_verified",
            "audit_id":"sampling-audit-1",
            "source_fingerprint":"sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "probabilities":[0.1,0.2,0.3,0.4],
            "probability_sum":1.0,
            "log_odds_ratio":0.0,
            "tolerance":1e-12
        }"#;
        assert!(serde_json::from_str::<ProductDesignEvidence>(forged_nonproduct).is_err());

        let evidence = ProductDesignEvidence::from_sampling_odds_audit(
            "sampling-audit-1",
            SOURCE_FINGERPRINT,
            [0.25; 4],
            1e-12,
        )
        .unwrap();
        let encoded = serde_json::to_string(&evidence).unwrap();
        let decoded: ProductDesignEvidence = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, evidence);
        assert_eq!(decoded.sampling_odds_audit_id(), Some("sampling-audit-1"));
    }

    #[test]
    fn identical_samples_have_zero_energy_distance() {
        let sample = vec![vec![0.0], vec![1.0], vec![2.0]];
        assert!(energy_distance(&sample, &sample).unwrap().abs() < 1e-14);
    }

    #[test]
    fn deterministic_rng_repeats() {
        let mut left = SplitMix64::new(7);
        let mut right = SplitMix64::new(7);
        for _ in 0..32 {
            assert_eq!(left.next_u64(), right.next_u64());
        }
    }

    fn candidate(variables: &[&str], holdout_loss: f64, complexity: f64) -> CandidateSupport {
        CandidateSupport {
            variables: variables.iter().map(|name| (*name).into()).collect(),
            holdout_loss,
            complexity,
        }
    }

    #[test]
    fn frontier_prefers_smallest_support_within_tolerance() {
        let candidates = vec![
            candidate(&["t", "p1", "p2", "z"], 0.100, 4.0),
            candidate(&["t", "p1", "p2"], 0.101, 3.0),
            candidate(&["t", "p1"], 0.150, 2.0),
            candidate(&["z"], 0.900, 1.0),
        ];
        let frontier = parsimony_frontier(&candidates, 0.005).unwrap();
        assert_eq!(frontier.best_loss, 0.100);
        assert_eq!(frontier.frontier, vec![1, 0]);
        assert_eq!(
            frontier.minimal_support,
            vec!["p1".to_string(), "p2".into(), "t".into()]
        );
        assert_eq!(frontier.inclusion_frequencies["t"], 1.0);
        assert_eq!(frontier.inclusion_frequencies["z"], 0.5);
    }

    #[test]
    fn frontier_is_invariant_to_candidate_order() {
        let forward = vec![
            candidate(&["t", "p1"], 0.10, 2.0),
            candidate(&["t", "p1", "z"], 0.10, 3.0),
            candidate(&["z"], 0.50, 1.0),
        ];
        let reversed: Vec<CandidateSupport> = forward.iter().rev().cloned().collect();
        let left = parsimony_frontier(&forward, 0.01).unwrap();
        let right = parsimony_frontier(&reversed, 0.01).unwrap();
        assert_eq!(left.minimal_support, right.minimal_support);
        assert_eq!(left.inclusion_frequencies, right.inclusion_frequencies);
        assert_eq!(left.best_loss, right.best_loss);
    }

    #[test]
    fn frontier_frequencies_are_descriptive_per_variable_rates() {
        let candidates = vec![
            candidate(&["a", "b"], 0.1, 2.0),
            candidate(&["a", "c"], 0.1, 2.0),
            candidate(&["a"], 0.1, 1.0),
        ];
        let frontier = parsimony_frontier(&candidates, 0.0).unwrap();
        assert_eq!(frontier.inclusion_frequencies["a"], 1.0);
        assert!(
            frontier
                .inclusion_frequencies
                .values()
                .all(|&value| (0.0..=1.0).contains(&value))
        );
        assert_eq!(frontier.minimal_support, vec!["a".to_string()]);
    }

    #[test]
    fn support_cardinality_precedes_learner_complexity() {
        let candidates = vec![
            candidate(&["a", "b"], 0.1, 0.0),
            candidate(&["a"], 0.1, 100.0),
        ];
        let frontier = parsimony_frontier(&candidates, 0.0).unwrap();
        assert_eq!(frontier.frontier, vec![1, 0]);
        assert_eq!(frontier.minimal_support, vec!["a".to_string()]);
    }

    #[test]
    fn frontier_rejects_duplicate_variables() {
        let error = parsimony_frontier(&[candidate(&["a", "a"], 0.1, 1.0)], 0.0).unwrap_err();
        assert!(matches!(error, StatsError::Invalid { .. }));
    }

    #[test]
    fn deletion_classification_uses_bounds_not_point() {
        let invariant = classify_deletion("t", 0.01, 0.0, 0.04, 0.05).unwrap();
        assert_eq!(invariant.status, EquivalenceStatus::CertifiedInvariant);
        let changed = classify_deletion("p", 0.40, 0.20, 0.60, 0.05).unwrap();
        assert_eq!(changed.status, EquivalenceStatus::CertifiedChanged);
        let undetermined = classify_deletion("z", 0.05, 0.01, 0.20, 0.05).unwrap();
        assert_eq!(undetermined.status, EquivalenceStatus::Undetermined);
        let error = classify_deletion("bad", 0.5, 0.6, 0.7, 0.05).unwrap_err();
        assert!(matches!(error, StatsError::Invalid { .. }));
    }

    #[test]
    fn orientation_state_machine_matches_formal_spec() {
        let invariant = |name: &str| classify_deletion(name, 0.01, 0.0, 0.02, 0.05).unwrap();
        let changed = |name: &str| classify_deletion(name, 0.9, 0.6, 1.2, 0.05).unwrap();
        let unresolved = |name: &str| classify_deletion(name, 0.05, 0.01, 0.2, 0.05).unwrap();

        let unique =
            orient_from_deletions(&[invariant("t"), changed("p1"), changed("p2")], 1.0, 0.1)
                .unwrap();
        assert_eq!(
            unique,
            OrientationOutcome::UniqueTarget { target: "t".into() }
        );

        let parity = orient_from_deletions(&[invariant("P"), invariant("T")], 1.0, 0.1).unwrap();
        assert_eq!(
            parity,
            OrientationOutcome::MultiplePasses {
                passes: vec!["P".into(), "T".into()]
            }
        );

        let none = orient_from_deletions(&[changed("a"), changed("b")], 1.0, 0.1).unwrap();
        assert_eq!(none, OrientationOutcome::NoPass);

        let weak = orient_from_deletions(&[invariant("t"), changed("p")], 0.05, 0.1).unwrap();
        assert_eq!(weak, OrientationOutcome::Underpowered);

        let blocked = orient_from_deletions(&[invariant("t"), unresolved("p")], 1.0, 0.1).unwrap();
        assert_eq!(
            blocked,
            OrientationOutcome::Undetermined {
                unresolved: vec!["p".into()]
            }
        );
    }

    #[test]
    fn orientation_rejects_row_specific_equivalence_tolerances() {
        let deletions = [
            classify_deletion(" target ", 0.01, 0.0, 0.02, 0.05).unwrap(),
            classify_deletion("parent", 0.09, 0.06, 0.12, 0.10).unwrap(),
        ];
        assert_eq!(deletions[0].variable, "target");
        let error = orient_from_deletions(&deletions, 1.0, 0.1).unwrap_err();
        assert!(matches!(error, StatsError::Invalid { .. }));
    }

    #[test]
    fn multiplier_bounds_are_deterministic_and_cover_means() {
        let contributions: Vec<Vec<f64>> = (0..16)
            .map(|index| {
                let x = f64::from(index);
                vec![0.1 + 0.01 * x, 0.5 - 0.02 * x]
            })
            .collect();
        let first = simultaneous_mean_bounds(&contributions, 500, 0.95, 20_260_812).unwrap();
        let second = simultaneous_mean_bounds(&contributions, 500, 0.95, 20_260_812).unwrap();
        assert_eq!(first, second);
        for column in 0..2 {
            assert!(first.lower[column] <= first.means[column]);
            assert!(first.means[column] <= first.upper[column]);
        }
        assert!(first.critical_value > 0.0);
        let different_seed = simultaneous_mean_bounds(&contributions, 500, 0.95, 7).unwrap();
        assert_eq!(different_seed.means, first.means);
    }

    #[test]
    fn multiplier_bounds_handle_constant_columns() {
        let contributions = vec![vec![0.3, 1.0], vec![0.3, 2.0], vec![0.3, 3.0]];
        let bounds = simultaneous_mean_bounds(&contributions, 200, 0.9, 5).unwrap();
        assert_eq!(bounds.standard_errors[0], 0.0);
        assert_eq!(bounds.lower[0], bounds.means[0]);
        assert_eq!(bounds.upper[0], bounds.means[0]);
    }
}
