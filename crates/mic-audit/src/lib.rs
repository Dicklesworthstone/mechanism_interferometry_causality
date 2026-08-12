#![forbid(unsafe_code)]
//! Evidence ledger and strict-mode policy for mechanism-interferometry runs.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

mod report;
pub use report::{NarrativeReport, render_narrative};

/// Execution policy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Any unresolved load-bearing assumption blocks a certificate.
    Strict,
    /// Analysis may continue, but affected outputs remain diagnostic.
    Exploratory,
}

/// Finding severity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational evidence.
    Info,
    /// A recoverable warning.
    Warning,
    /// A strict-mode blocking condition.
    Error,
}

/// One typed audit finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Finding {
    /// Stable machine-readable reason code.
    pub code: String,
    /// Human-readable finding.
    pub message: String,
    /// Severity.
    pub severity: Severity,
    /// Pipeline stage.
    pub stage: String,
    /// Optional structured context.
    pub context: BTreeMap<String, String>,
}

/// Final certificate status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CertificateStatus {
    /// All declared certificate gates passed.
    Passed,
    /// At least one population implication was statistically rejected.
    Failed,
    /// Evidence was insufficient or a load-bearing contract was unknown.
    Abstained,
    /// The run is exploratory and cannot issue a certificate.
    DiagnosticOnly,
}

/// Evidence state for a necessary population implication of modularity.
///
/// `Unresolved` includes absent, invalid, underpowered, or otherwise
/// indeterminate evidence. It is not a synonym for a population refutation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImplicationVerdict {
    /// The implication was established under its declared evidence contract.
    Established,
    /// The implication was refuted under its declared evidence contract.
    Refuted,
    /// The available evidence does not determine the implication.
    Unresolved,
}

/// Evidence state for deletion-based target orientation.
///
/// Orientation ambiguity cannot refute modularity, so this type intentionally
/// has no `Refuted` variant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrientationVerdict {
    /// Unique orientation and its single-target/deletion-faithfulness premises
    /// were independently established under the declared evidence contract.
    Established,
    /// The pass pattern or any required orientation premise is unresolved.
    Unresolved,
}

/// Complete set of load-bearing gates for a strict inferred-target certificate.
///
/// There is deliberately no `Default` implementation and no Boolean
/// conversion: every producer must state every gate explicitly.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CertificateGates {
    /// Intervention locality for the proposed target/mechanism assignment.
    locality: ImplicationVerdict,
    /// Conditional normalization of every primitive mechanism replacement.
    conditional_normalization: ImplicationVerdict,
    /// Vanishing density curvature on every required estimable contrast.
    square_flatness: ImplicationVerdict,
    /// Authority-level orientation after both unique deletion equivalence and
    /// the single-target/deletion-faithfulness premises are established.
    orientation: OrientationVerdict,
}

impl CertificateGates {
    /// Constructs a deliberately non-certifying gate set for diagnostic paths.
    #[must_use]
    pub const fn unresolved() -> Self {
        Self {
            locality: ImplicationVerdict::Unresolved,
            conditional_normalization: ImplicationVerdict::Unresolved,
            square_flatness: ImplicationVerdict::Unresolved,
            orientation: OrientationVerdict::Unresolved,
        }
    }

    /// Returns the locality gate summary.
    #[must_use]
    pub const fn locality(self) -> ImplicationVerdict {
        self.locality
    }

    /// Returns the conditional-normalization gate summary.
    #[must_use]
    pub const fn conditional_normalization(self) -> ImplicationVerdict {
        self.conditional_normalization
    }

    /// Returns the square-flatness gate summary.
    #[must_use]
    pub const fn square_flatness(self) -> ImplicationVerdict {
        self.square_flatness
    }

    /// Returns the authority-level orientation gate summary.
    #[must_use]
    pub const fn orientation(self) -> OrientationVerdict {
        self.orientation
    }

    fn has_refuted_implication(self) -> bool {
        [
            self.locality,
            self.conditional_normalization,
            self.square_flatness,
        ]
        .contains(&ImplicationVerdict::Refuted)
    }

    fn establishes_certificate(self) -> bool {
        self.locality == ImplicationVerdict::Established
            && self.conditional_normalization == ImplicationVerdict::Established
            && self.square_flatness == ImplicationVerdict::Established
            && self.orientation == OrientationVerdict::Established
    }
}

/// Append-only evidence ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceLedger {
    /// Schema version.
    pub schema_version: String,
    /// Execution policy.
    pub mode: ExecutionMode,
    /// Ordered findings.
    pub findings: Vec<Finding>,
    /// Arbitrary immutable provenance fields.
    pub provenance: BTreeMap<String, String>,
}

impl EvidenceLedger {
    /// Creates a new ledger.
    #[must_use]
    pub fn new(mode: ExecutionMode) -> Self {
        Self {
            schema_version: "1.0.0".into(),
            mode,
            findings: Vec::new(),
            provenance: BTreeMap::new(),
        }
    }

    /// Adds a provenance field before report finalization.
    pub fn provenance(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.provenance.insert(key.into(), value.into());
    }

    /// Appends a finding.
    pub fn push(&mut self, finding: Finding) {
        self.findings.push(finding);
    }

    /// Adds a concise finding without context.
    pub fn note(
        &mut self,
        severity: Severity,
        stage: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.push(Finding {
            code: code.into(),
            message: message.into(),
            severity,
            stage: stage.into(),
            context: BTreeMap::new(),
        });
    }

    /// Returns true when an error blocks a strict run.
    #[must_use]
    pub fn has_blocking_error(&self) -> bool {
        self.mode == ExecutionMode::Strict
            && self
                .findings
                .iter()
                .any(|finding| finding.severity == Severity::Error)
    }

    /// Derives a conservative final status.
    #[must_use]
    pub fn status(&self, gates: &CertificateGates) -> CertificateStatus {
        if self.mode == ExecutionMode::Exploratory {
            return CertificateStatus::DiagnosticOnly;
        }
        if self.has_blocking_error() {
            return CertificateStatus::Abstained;
        }
        if gates.has_refuted_implication() {
            return CertificateStatus::Failed;
        }
        if gates.establishes_certificate() {
            CertificateStatus::Passed
        } else {
            CertificateStatus::Abstained
        }
    }

    /// Canonical JSON hash used to bind reports to evidence.
    pub fn sha256(&self) -> Result<String, AuditError> {
        use core::fmt::Write as _;
        let bytes = serde_json::to_vec(self)?;
        let digest = Sha256::digest(bytes);
        let mut encoded = String::with_capacity(digest.len() * 2);
        for byte in &digest {
            write!(&mut encoded, "{byte:02x}").expect("writing hex into a String cannot fail");
        }
        Ok(encoded)
    }
}

/// Evidence-ledger serialization failures.
#[derive(Debug, Error)]
pub enum AuditError {
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Common strict-mode reason codes.
pub mod code {
    /// Sampling odds are not product for a requested GCM test.
    pub const NON_PRODUCT_GCM: &str = "non_product_sampling_for_gcm";
    /// Within-regime selection may depend on state.
    pub const STATE_DEPENDENT_SELECTION: &str = "state_dependent_selection";
    /// Common support or effective sample size failed.
    pub const OVERLAP_FAILURE: &str = "overlap_failure";
    /// Orientation did not have one certified deletion pass.
    pub const ORIENTATION_UNRESOLVED: &str = "orientation_unresolved";
    /// A requested design contrast is aliased or unidentifiable.
    pub const DESIGN_ALIAS: &str = "design_contrast_aliased";
    /// Negative controls indicate implementation curvature.
    pub const NEGATIVE_CONTROL_CURVATURE: &str = "negative_control_curvature";
    /// A modeled selection process was declared but not yet validated.
    pub const SELECTION_MODEL_UNVALIDATED: &str = "selection_model_unvalidated";
    /// The observed design has no testable lack-of-fit restriction.
    pub const NO_TESTABLE_FLATNESS: &str = "no_testable_flatness";
    /// Square faces do not span every testable flatness restriction.
    pub const NON_SQUARE_CONTRASTS_REQUIRED: &str = "non_square_contrasts_required";
    /// Certified projections disagree across the declared estimator lens battery.
    pub const ESTIMATOR_FAMILY_DISAGREEMENT: &str = "estimator_family_disagreement";
    /// A cluster appears under more than one regime.
    pub const CLUSTER_SPANS_REGIMES: &str = "cluster_spans_regimes";
    /// A declared regime has no included clusters.
    pub const MISSING_REGIME_DATA: &str = "missing_regime_data";
    /// Histogram four-law common support is empty.
    pub const EMPTY_COMMON_SUPPORT: &str = "empty_common_support";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_established() -> CertificateGates {
        CertificateGates {
            locality: ImplicationVerdict::Established,
            conditional_normalization: ImplicationVerdict::Established,
            square_flatness: ImplicationVerdict::Established,
            orientation: OrientationVerdict::Established,
        }
    }

    #[test]
    fn strict_error_abstains() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        ledger.note(
            Severity::Error,
            "design",
            code::NON_PRODUCT_GCM,
            "not product",
        );
        assert_eq!(
            ledger.status(&all_established()),
            CertificateStatus::Abstained
        );
    }

    #[test]
    fn exploratory_never_issues_certificate() {
        let ledger = EvidenceLedger::new(ExecutionMode::Exploratory);
        assert_eq!(
            ledger.status(&all_established()),
            CertificateStatus::DiagnosticOnly
        );
    }

    #[test]
    fn unresolved_or_partial_evidence_abstains() {
        let ledger = EvidenceLedger::new(ExecutionMode::Strict);
        assert_eq!(
            ledger.status(&CertificateGates::unresolved()),
            CertificateStatus::Abstained
        );

        let candidates = [
            CertificateGates {
                locality: ImplicationVerdict::Established,
                ..CertificateGates::unresolved()
            },
            CertificateGates {
                conditional_normalization: ImplicationVerdict::Established,
                ..CertificateGates::unresolved()
            },
            CertificateGates {
                square_flatness: ImplicationVerdict::Established,
                ..CertificateGates::unresolved()
            },
            CertificateGates {
                orientation: OrientationVerdict::Established,
                ..CertificateGates::unresolved()
            },
            CertificateGates {
                locality: ImplicationVerdict::Established,
                conditional_normalization: ImplicationVerdict::Established,
                square_flatness: ImplicationVerdict::Established,
                orientation: OrientationVerdict::Unresolved,
            },
        ];
        for gates in candidates {
            assert_eq!(ledger.status(&gates), CertificateStatus::Abstained);
        }

        for missing in 0..3 {
            let mut gates = all_established();
            match missing {
                0 => gates.locality = ImplicationVerdict::Unresolved,
                1 => gates.conditional_normalization = ImplicationVerdict::Unresolved,
                2 => gates.square_flatness = ImplicationVerdict::Unresolved,
                _ => unreachable!(),
            }
            assert_eq!(ledger.status(&gates), CertificateStatus::Abstained);
        }
    }

    #[test]
    fn only_complete_established_evidence_passes() {
        let ledger = EvidenceLedger::new(ExecutionMode::Strict);
        assert_eq!(ledger.status(&all_established()), CertificateStatus::Passed);
    }

    #[test]
    fn valid_implication_refutation_fails_even_when_another_gate_is_unresolved() {
        let ledger = EvidenceLedger::new(ExecutionMode::Strict);
        for refuted in 0..3 {
            let mut gates = all_established();
            match refuted {
                0 => gates.locality = ImplicationVerdict::Refuted,
                1 => gates.conditional_normalization = ImplicationVerdict::Refuted,
                2 => gates.square_flatness = ImplicationVerdict::Refuted,
                _ => unreachable!(),
            }
            assert_eq!(ledger.status(&gates), CertificateStatus::Failed);
        }

        let gates = CertificateGates {
            locality: ImplicationVerdict::Refuted,
            ..CertificateGates::unresolved()
        };
        assert_eq!(ledger.status(&gates), CertificateStatus::Failed);
    }

    #[test]
    fn invalid_evidence_contract_precedes_a_purported_refutation() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        ledger.note(Severity::Error, "selection", "invalid_contract", "invalid");
        let gates = CertificateGates {
            locality: ImplicationVerdict::Refuted,
            ..CertificateGates::unresolved()
        };
        assert_eq!(ledger.status(&gates), CertificateStatus::Abstained);
        assert_eq!(
            ledger.status(&all_established()),
            CertificateStatus::Abstained
        );
    }

    #[test]
    fn exploratory_refutation_remains_diagnostic_only() {
        let ledger = EvidenceLedger::new(ExecutionMode::Exploratory);
        let gates = CertificateGates {
            locality: ImplicationVerdict::Refuted,
            ..CertificateGates::unresolved()
        };
        assert_eq!(ledger.status(&gates), CertificateStatus::DiagnosticOnly);
    }

    #[test]
    fn gate_json_serializes_the_complete_closed_summary() {
        let encoded = serde_json::to_string(&all_established()).unwrap();
        let document: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        let object = document.as_object().unwrap();
        assert_eq!(object.len(), 4);
        assert_eq!(object["locality"], "established");
        assert_eq!(object["conditional_normalization"], "established");
        assert_eq!(object["square_flatness"], "established");
        assert_eq!(object["orientation"], "established");
    }

    #[test]
    fn every_gate_state_derives_a_status_without_deserialization_authority() {
        for locality in [
            ImplicationVerdict::Established,
            ImplicationVerdict::Refuted,
            ImplicationVerdict::Unresolved,
        ] {
            for conditional_normalization in [
                ImplicationVerdict::Established,
                ImplicationVerdict::Refuted,
                ImplicationVerdict::Unresolved,
            ] {
                for square_flatness in [
                    ImplicationVerdict::Established,
                    ImplicationVerdict::Refuted,
                    ImplicationVerdict::Unresolved,
                ] {
                    for orientation in [
                        OrientationVerdict::Established,
                        OrientationVerdict::Unresolved,
                    ] {
                        let gates = CertificateGates {
                            locality,
                            conditional_normalization,
                            square_flatness,
                            orientation,
                        };
                        let _ = ledger_status_for_truth_table(gates);
                    }
                }
            }
        }
    }

    fn ledger_status_for_truth_table(gates: CertificateGates) -> CertificateStatus {
        EvidenceLedger::new(ExecutionMode::Strict).status(&gates)
    }
}
