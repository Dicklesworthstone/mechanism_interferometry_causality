#![forbid(unsafe_code)]
//! Evidence ledger and strict-mode policy for mechanism-interferometry runs.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

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
    pub fn status(&self, certificate_conditions_passed: bool) -> CertificateStatus {
        if self.mode == ExecutionMode::Exploratory {
            return CertificateStatus::DiagnosticOnly;
        }
        if self.has_blocking_error() {
            return CertificateStatus::Abstained;
        }
        if certificate_conditions_passed {
            CertificateStatus::Passed
        } else {
            CertificateStatus::Failed
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_error_abstains() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        ledger.note(
            Severity::Error,
            "design",
            code::NON_PRODUCT_GCM,
            "not product",
        );
        assert_eq!(ledger.status(true), CertificateStatus::Abstained);
    }

    #[test]
    fn exploratory_never_issues_certificate() {
        let ledger = EvidenceLedger::new(ExecutionMode::Exploratory);
        assert_eq!(ledger.status(true), CertificateStatus::DiagnosticOnly);
    }
}
