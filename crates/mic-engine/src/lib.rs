#![forbid(unsafe_code)]
//! Fail-closed preflight orchestration for mechanism-interferometry analyses.

use mic_audit::{EvidenceLedger, ExecutionMode, Finding, Severity, code};
use mic_data::{ExperimentManifest, InferenceTrack, ManifestError, SelectionContract, TableError};
use mic_design::{
    DesignAudit, DesignError, DesignPoint, SamplingOddsAudit, SquareFace, audit_design,
    audit_sampling_odds,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

mod survey;
mod tabular;
pub use mic_design::{CausalCompletionEvaluation, NextQueryPurpose, OrientationTestability};
pub use survey::{
    ClusterUnitBasis, ColumnRole, ColumnTriage, InterferometerProposal, SurveyAuthority,
    SurveyDesignInformationContent, SurveyPolicy, SurveyReport, run_unsupervised_survey,
};
pub use tabular::{
    CellCurvature, ColumnProjection, FaceRatioOverlapAudit, FourLawFaceAudit, FourLawPolicy,
    ProjectionSpec, TabularAuditReport, TabularInformationContent, TabularIngestSummary,
    run_tabular_audit, run_tabular_audit_with_selection_evidence,
};

/// Numerical and policy settings for preflight validation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PreflightPolicy {
    /// Numerical tolerance for design-rank calculations.
    pub rank_tolerance: f64,
    /// Allowed absolute departure of pooled log odds from zero in GCM mode.
    pub product_odds_tolerance: f64,
    /// Whether a declared selection model is accepted before its diagnostics are attached.
    pub accept_unvalidated_selection_model: bool,
    /// Maximum standard-error-scaled pairwise gap tolerated between estimator families.
    pub lens_gap_tolerance: f64,
    /// Minimum acceptable ratio of effective sample size to sample size for ratio weights.
    pub min_ess_ratio: f64,
}

impl Default for PreflightPolicy {
    fn default() -> Self {
        Self {
            rank_tolerance: 1e-10,
            product_odds_tolerance: 1e-10,
            accept_unvalidated_selection_model: false,
            lens_gap_tolerance: 3.0,
            min_ess_ratio: 0.1,
        }
    }
}

/// Product-odds result for one fully observed square face.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaceSamplingAudit {
    /// Base corner as a compact bit string.
    pub base: String,
    /// First varying coordinate.
    pub first: usize,
    /// Second varying coordinate.
    pub second: usize,
    /// Corner identifiers in order `00, 10, 01, 11`.
    pub corners: [String; 4],
    /// Product-odds result.
    pub sampling: SamplingOddsAudit,
}

/// Conservative preflight state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreflightStatus {
    /// Required contracts for the requested analysis are satisfied.
    Ready,
    /// The manifest is structurally valid but the run is exploratory.
    DiagnosticOnly,
    /// A load-bearing contract blocks the requested analysis.
    Blocked,
}

/// Authority class of a content-bound external selection premise.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionEvidenceClass {
    /// External sampling/enrollment records assert state-independent inclusion.
    ExternalSamplingRecord,
    /// A separately validated recovery model supports the declared selection model.
    ValidatedSelectionModel,
}

/// Wire receipt resolved by the engine before it can satisfy selection readiness.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectionEvidenceReceipt {
    /// Receipt schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Experiment identifier copied from the manifest.
    pub experiment_id: String,
    /// Canonical SHA-256 of the complete validated manifest.
    pub manifest_canonical_sha256: String,
    /// SHA-256 of the exact analyzed data bytes.
    pub data_content_sha256: String,
    /// Selection declaration this receipt is allowed to support.
    pub declaration: SelectionContract,
    /// External evidence authority class.
    pub evidence_class: SelectionEvidenceClass,
    /// SHA-256 of the separately supplied authority-source bytes.
    pub authority_source_sha256: String,
}

/// Opaque provenance token created only by content verification.
///
/// This token proves that the receipt, manifest, analyzed bytes, and cited
/// source agree byte-for-byte. It deliberately does **not** prove that the
/// caller-supplied source is an independently trusted scientific authority,
/// and therefore cannot by itself satisfy strict selection readiness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSelectionEvidence {
    receipt_id: String,
    receipt_sha256: String,
    experiment_id: String,
    manifest_canonical_sha256: String,
    data_content_sha256: String,
    declaration: SelectionContract,
    evidence_class: SelectionEvidenceClass,
    authority_source_sha256: String,
}

/// Resolves a selection receipt against exact manifest, data, and authority bytes.
pub fn resolve_selection_evidence(
    manifest: &ExperimentManifest,
    receipt_bytes: &[u8],
    analyzed_data_bytes: &[u8],
    authority_source_bytes: &[u8],
) -> Result<ValidatedSelectionEvidence, EngineError> {
    resolve_selection_evidence_hashes(
        manifest,
        receipt_bytes,
        &hex_sha256(analyzed_data_bytes),
        authority_source_bytes,
    )
}

fn resolve_selection_evidence_hashes(
    manifest: &ExperimentManifest,
    receipt_bytes: &[u8],
    data_content_sha256: &str,
    authority_source_bytes: &[u8],
) -> Result<ValidatedSelectionEvidence, EngineError> {
    manifest.validate()?;
    let receipt: SelectionEvidenceReceipt =
        serde_json::from_slice(receipt_bytes).map_err(EngineError::SelectionEvidenceParse)?;
    let expected_manifest = canonical_manifest_sha256(manifest)?;
    let authority_source_sha256 = hex_sha256(authority_source_bytes);
    let expected_class = match receipt.declaration {
        SelectionContract::StateIndependentWithinRegime => {
            SelectionEvidenceClass::ExternalSamplingRecord
        }
        SelectionContract::Modeled => SelectionEvidenceClass::ValidatedSelectionModel,
        SelectionContract::Unknown | SelectionContract::StateDependentUnmodeled => {
            return Err(EngineError::InvalidSelectionEvidence(
                "unknown or unmodeled state-dependent selection cannot be resolved ready".into(),
            ));
        }
    };
    if receipt.schema_version != "1.0.0"
        || receipt.receipt_id.trim().is_empty()
        || receipt.receipt_id.chars().count() > 1024
        || receipt.experiment_id.trim().is_empty()
        || receipt.experiment_id.chars().count() > 1024
        || receipt.experiment_id != manifest.experiment_id
        || receipt.manifest_canonical_sha256 != expected_manifest
        || receipt.data_content_sha256 != data_content_sha256
        || receipt.declaration != manifest.selection
        || receipt.evidence_class != expected_class
        || receipt.authority_source_sha256 != authority_source_sha256
        || authority_source_bytes.is_empty()
    {
        return Err(EngineError::InvalidSelectionEvidence(
            "receipt does not bind the manifest, analyzed data, declaration, evidence class, and nonempty authority source"
                .into(),
        ));
    }
    Ok(ValidatedSelectionEvidence {
        receipt_id: receipt.receipt_id,
        receipt_sha256: hex_sha256(receipt_bytes),
        experiment_id: receipt.experiment_id,
        manifest_canonical_sha256: receipt.manifest_canonical_sha256,
        data_content_sha256: data_content_sha256.to_string(),
        declaration: receipt.declaration,
        evidence_class: receipt.evidence_class,
        authority_source_sha256,
    })
}

/// Resolves a selection receipt and its authority source from bounded caller paths.
pub fn resolve_selection_evidence_from_files(
    manifest: &ExperimentManifest,
    receipt_path: impl AsRef<Path>,
    authority_source_path: impl AsRef<Path>,
    base_dir: Option<&Path>,
) -> Result<ValidatedSelectionEvidence, EngineError> {
    const MAX_EVIDENCE_BYTES: u64 = 8 * 1024 * 1024;
    let receipt_path = resolve_relative_path(receipt_path.as_ref(), base_dir);
    let authority_source_path = resolve_relative_path(authority_source_path.as_ref(), base_dir);
    let data_path = mic_data::resolve_data_path(&manifest.data.path, base_dir)?;
    let receipt_bytes = read_bounded_evidence(&receipt_path, MAX_EVIDENCE_BYTES)?;
    let authority_source_bytes = read_bounded_evidence(&authority_source_path, MAX_EVIDENCE_BYTES)?;
    let data_content_sha256 = sha256_file(&data_path)?;
    resolve_selection_evidence_hashes(
        manifest,
        &receipt_bytes,
        &data_content_sha256,
        &authority_source_bytes,
    )
}

/// Machine-readable preflight result.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreflightReport {
    /// Schema version.
    schema_version: String,
    /// Experiment identifier.
    experiment_id: String,
    /// SHA-256 of the validated manifest's canonical serialized value.
    manifest_canonical_sha256: String,
    /// Policy the status was computed under. Verdicts are not reproducible without it.
    policy: PreflightPolicy,
    /// Requested inference track.
    requested_track: InferenceTrack,
    /// Main-effects and lack-of-fit geometry.
    design: DesignAudit,
    /// Sampling-odds audits for every observed square.
    face_sampling: Vec<FaceSamplingAudit>,
    /// Whether four-law functionals are permitted by the declared selection contract.
    four_law_eligible: bool,
    /// Whether product-factorial inference has independently resolved design
    /// authority. Arithmetic checks of caller-declared quotas are retained in
    /// `face_sampling`, but cannot make this field true by themselves.
    product_factorial_eligible: bool,
    /// Conservative preflight state.
    status: PreflightStatus,
    /// Evidence and reason codes.
    ledger: EvidenceLedger,
}

impl PreflightReport {
    /// Returns the report schema version.
    #[must_use]
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    /// Returns the experiment identifier.
    #[must_use]
    pub fn experiment_id(&self) -> &str {
        &self.experiment_id
    }

    /// Returns the content binding for the validated manifest value.
    ///
    /// This is a hash of the canonical Rust serialization after validation,
    /// not a hash of the source file's whitespace or key ordering.
    #[must_use]
    pub fn manifest_canonical_sha256(&self) -> &str {
        &self.manifest_canonical_sha256
    }

    /// Returns the numerical policy bound into this report.
    #[must_use]
    pub const fn policy(&self) -> PreflightPolicy {
        self.policy
    }

    /// Returns the requested inference track.
    #[must_use]
    pub const fn requested_track(&self) -> &InferenceTrack {
        &self.requested_track
    }

    /// Returns the audited design geometry.
    #[must_use]
    pub const fn design(&self) -> &DesignAudit {
        &self.design
    }

    /// Returns the per-face sampling-odds audits.
    #[must_use]
    pub fn face_sampling(&self) -> &[FaceSamplingAudit] {
        &self.face_sampling
    }

    /// Returns whether the declared selection contract permits four-law diagnostics.
    #[must_use]
    pub const fn four_law_eligible(&self) -> bool {
        self.four_law_eligible
    }

    /// Returns whether product-factorial inference has resolved design authority.
    #[must_use]
    pub const fn product_factorial_eligible(&self) -> bool {
        self.product_factorial_eligible
    }

    /// Returns the internally derived preflight status.
    #[must_use]
    pub const fn status(&self) -> PreflightStatus {
        self.status
    }

    /// Returns the immutable evidence ledger bound to the preflight result.
    #[must_use]
    pub const fn ledger(&self) -> &EvidenceLedger {
        &self.ledger
    }
}

/// Preflight failures that prevent creation of a report.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Manifest validation failed.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// Design audit failed.
    #[error(transparent)]
    Design(#[from] DesignError),
    /// A face refers to a regime absent from the manifest.
    #[error("manifest is missing design corner {0}")]
    MissingCorner(String),
    /// A lens-battery input was structurally invalid.
    #[error("estimator lens battery is invalid: {0}")]
    InvalidLensBattery(String),
    /// A statistical primitive rejected its inputs.
    #[error(transparent)]
    Stats(#[from] mic_stats::StatsError),
    /// An overlap-audit input was structurally invalid.
    #[error("overlap audit is invalid: {0}")]
    InvalidOverlap(String),
    /// Tabular ingest failed.
    #[error(transparent)]
    Table(#[from] TableError),
    /// The histogram four-law projection rejected its inputs.
    #[error("tabular four-law audit is invalid: {0}")]
    InvalidTabular(String),
    /// A numerical policy value would loosen a load-bearing gate.
    #[error("preflight policy is invalid: {0}")]
    InvalidPolicy(String),
    /// The validated manifest could not be serialized for content binding.
    #[error("validated manifest could not be fingerprinted: {0}")]
    ManifestFingerprint(#[source] serde_json::Error),
    /// Selection-evidence JSON could not be parsed as the closed receipt type.
    #[error("selection evidence receipt is invalid JSON: {0}")]
    SelectionEvidenceParse(#[source] serde_json::Error),
    /// Selection evidence failed content or semantic binding.
    #[error("selection evidence is invalid: {0}")]
    InvalidSelectionEvidence(String),
    /// A bounded evidence input could not be read.
    #[error("selection evidence I/O failed: {0}")]
    SelectionEvidenceIo(#[source] std::io::Error),
}

impl PreflightPolicy {
    /// Product-odds tolerance after the hard ceiling.
    ///
    /// Callers may tighten [`Self::product_odds_tolerance`] but cannot loosen it
    /// past [`mic_stats::ProductDesignEvidence::MAX_PRODUCT_ODDS_TOLERANCE`].
    /// A large finite slack (for example `0.5`) would otherwise stamp a 2:3
    /// odds ratio as product and defeat AGENTS.md rule 2.
    pub fn bounded_product_odds_tolerance(&self) -> Result<f64, EngineError> {
        let ceiling = mic_stats::ProductDesignEvidence::MAX_PRODUCT_ODDS_TOLERANCE;
        let requested = self.product_odds_tolerance;
        if !requested.is_finite() || !(0.0..=ceiling).contains(&requested) {
            return Err(EngineError::InvalidPolicy(format!(
                "product_odds_tolerance must lie in [0, {ceiling}], got {requested}"
            )));
        }
        Ok(requested)
    }
}

/// Serializable result of the deletion-orientation audit.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OrientationAudit {
    /// Classified deletions in input order.
    deletions: Vec<mic_stats::DeletionEquivalence>,
    /// Full-support intervention discrepancy.
    full_discrepancy: f64,
    /// Power threshold below which the audit abstains as underpowered.
    min_full_discrepancy: f64,
    /// Five-state pass-count outcome.
    outcome: mic_stats::OrientationOutcome,
}

impl OrientationAudit {
    /// Returns the numerical pass-pattern outcome. It is not causal authority.
    #[must_use]
    pub const fn outcome(&self) -> &mic_stats::OrientationOutcome {
        &self.outcome
    }
}

/// Runs the pass-count state machine and records the verdict in the ledger.
///
/// A unique numerical pass is recorded as an informational proposal only;
/// this function has no inputs establishing the single-target and
/// deletion-faithfulness premises required for causal orientation. Every other state is recorded as a blocking error
/// with reason code [`code::ORIENTATION_UNRESOLVED`], so strict runs abstain
/// rather than forcing an orientation.  The multiple-pass state additionally
/// signals that an active same-target disambiguation tilt should be proposed.
pub fn audit_orientation(
    deletions: &[mic_stats::DeletionEquivalence],
    full_discrepancy: f64,
    min_full_discrepancy: f64,
    stage: &str,
    ledger: &mut EvidenceLedger,
) -> Result<OrientationAudit, EngineError> {
    let outcome =
        mic_stats::orient_from_deletions(deletions, full_discrepancy, min_full_discrepancy)?;
    let mut context = BTreeMap::new();
    context.insert("deletion_count".into(), deletions.len().to_string());
    context.insert("full_discrepancy".into(), format!("{full_discrepancy:.6}"));
    match &outcome {
        mic_stats::OrientationOutcome::UniquePassPattern { variable } => {
            context.insert("variable".into(), variable.clone());
            ledger.push(finding_with_context(
                Severity::Info,
                stage,
                code::info::ORIENTATION_UNIQUE_PASS_PATTERN,
                "exactly one deletion passes the numerical equivalence audit; causal orientation premises remain unresolved",
                context,
            ));
        }
        mic_stats::OrientationOutcome::NoPass => {
            context.insert("state".into(), "no_pass".into());
            ledger.push(finding_with_context(
                Severity::Error,
                stage,
                code::ORIENTATION_UNRESOLVED,
                "no deletion is certified invariant; suspect descendant contamination, multi-target primitives, selection, or implementation mismatch",
                context,
            ));
        }
        mic_stats::OrientationOutcome::MultiplePasses { passes } => {
            context.insert("state".into(), "multiple_passes".into());
            context.insert("passes".into(), passes.join(","));
            ledger.push(finding_with_context(
                Severity::Error,
                stage,
                code::ORIENTATION_UNRESOLVED,
                "multiple deletions are certified invariant; propose an asymmetric same-target tilt to disambiguate",
                context,
            ));
        }
        mic_stats::OrientationOutcome::Underpowered => {
            context.insert("state".into(), "underpowered".into());
            context.insert(
                "min_full_discrepancy".into(),
                format!("{min_full_discrepancy:.6}"),
            );
            ledger.push(finding_with_context(
                Severity::Error,
                stage,
                code::ORIENTATION_UNRESOLVED,
                "the intervention discrepancy is below the power threshold; an undetectable intervention cannot orient a family",
                context,
            ));
        }
        mic_stats::OrientationOutcome::Undetermined { unresolved } => {
            context.insert("state".into(), "undetermined".into());
            context.insert("unresolved".into(), unresolved.join(","));
            ledger.push(finding_with_context(
                Severity::Error,
                stage,
                code::ORIENTATION_UNRESOLVED,
                "simultaneous intervals overlap the equivalence boundary; collect more data or widen the design",
                context,
            ));
        }
    }
    Ok(OrientationAudit {
        deletions: deletions.to_vec(),
        full_discrepancy,
        min_full_discrepancy,
        outcome,
    })
}

/// Serializable result of the ratio-weight overlap audit.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OverlapAudit {
    /// Kish effective sample size of the ratio weights.
    effective_sample_size: f64,
    /// Number of observations carrying weights.
    sample_size: usize,
    /// Effective sample size divided by sample size.
    ess_ratio: f64,
    /// Policy floor the ratio was compared against.
    minimum_ratio: f64,
    /// Whether the overlap is adequate under the policy.
    adequate: bool,
}

impl OverlapAudit {
    /// Kish effective sample size of the supplied ratio weights.
    #[must_use]
    pub fn effective_sample_size(&self) -> f64 {
        self.effective_sample_size
    }

    /// Number of supplied ratio weights.
    #[must_use]
    pub fn sample_size(&self) -> usize {
        self.sample_size
    }

    /// Kish effective sample size divided by sample size.
    #[must_use]
    pub fn ess_ratio(&self) -> f64 {
        self.ess_ratio
    }

    /// Frozen adequacy threshold.
    #[must_use]
    pub fn minimum_ratio(&self) -> f64 {
        self.minimum_ratio
    }

    /// Whether the descriptive overlap threshold was met.
    #[must_use]
    pub fn adequate(&self) -> bool {
        self.adequate
    }
}

/// Audits common-support overlap through the effective sample size of ratio weights.
///
/// Density-ratio weights collapse toward a few observations exactly when the
/// regimes separate, which is also when ratio estimation is least reliable, so
/// an inadequate effective-sample-size ratio is recorded as a blocking error
/// with reason code [`code::OVERLAP_FAILURE`].
pub fn audit_overlap(
    ratio_weights: &[f64],
    policy: &PreflightPolicy,
    stage: &str,
    ledger: &mut EvidenceLedger,
) -> Result<OverlapAudit, EngineError> {
    if !policy.min_ess_ratio.is_finite()
        || policy.min_ess_ratio <= 0.0
        || policy.min_ess_ratio > 1.0
    {
        return Err(EngineError::InvalidOverlap(format!(
            "min_ess_ratio must lie in (0, 1], got {}",
            policy.min_ess_ratio
        )));
    }
    let effective = mic_stats::effective_sample_size(ratio_weights)?;
    let sample_size = ratio_weights.len();
    let ess_ratio = effective / sample_size as f64;
    let adequate = ess_ratio >= policy.min_ess_ratio;
    let mut context = BTreeMap::new();
    context.insert("effective_sample_size".into(), format!("{effective:.6}"));
    context.insert("sample_size".into(), sample_size.to_string());
    context.insert("ess_ratio".into(), format!("{ess_ratio:.6}"));
    context.insert(
        "minimum_ratio".into(),
        format!("{:.6}", policy.min_ess_ratio),
    );
    if adequate {
        ledger.push(finding_with_context(
            Severity::Info,
            stage,
            code::info::OVERLAP_ADEQUATE,
            "ratio-weight effective sample size meets the policy floor",
            context,
        ));
    } else {
        ledger.push(finding_with_context(
            Severity::Error,
            stage,
            code::OVERLAP_FAILURE,
            "ratio-weight effective sample size is below the policy floor; overlap is inadequate for reliable ratio functionals",
            context,
        ));
    }
    Ok(OverlapAudit {
        effective_sample_size: effective,
        sample_size,
        ess_ratio,
        minimum_ratio: policy.min_ess_ratio,
        adequate,
    })
}

/// One estimator family's projection of the same population estimand.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LensEstimate {
    /// Declared model-family label, for example `linear`, `kernel`, or `boosted_tree`.
    pub family: String,
    /// Cross-fitted point estimate of the shared estimand.
    pub estimate: f64,
    /// Estimated standard error of the point estimate; must be finite and strictly positive.
    pub standard_error: f64,
}

/// Sensitivity audit across a battery of deliberately dissimilar estimator families.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LensBatteryAudit {
    /// The estimates in input order.
    estimates: Vec<LensEstimate>,
    /// Largest standard-error-scaled pairwise gap observed across the battery.
    max_scaled_gap: f64,
    /// The pair of family labels achieving the largest scaled gap.
    worst_pair: [String; 2],
    /// Tolerance applied to the scaled gaps.
    tolerance: f64,
    /// Whether every scaled gap is within tolerance.
    agrees: bool,
}

impl LensBatteryAudit {
    /// Validated input estimates in declared order.
    #[must_use]
    pub fn estimates(&self) -> &[LensEstimate] {
        &self.estimates
    }

    /// Largest standard-error-scaled pairwise gap.
    #[must_use]
    pub fn max_scaled_gap(&self) -> f64 {
        self.max_scaled_gap
    }

    /// Family pair producing the largest scaled gap.
    #[must_use]
    pub fn worst_pair(&self) -> &[String; 2] {
        &self.worst_pair
    }

    /// Frozen sensitivity tolerance.
    #[must_use]
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Whether every descriptive scaled gap lies within tolerance.
    #[must_use]
    pub fn agrees(&self) -> bool {
        self.agrees
    }
}

/// Audits learner sensitivity of one estimand across estimator families.
///
/// The population curvature functionals are gauge invariant, so materially different
/// answers from different nuisance families indicate estimator artifacts rather than
/// evidence about the system.  The scaled gap divides each pairwise difference by the
/// root sum of squared standard errors; the families normally share data and folds, so
/// this is a preregistered robustness heuristic, not a calibrated joint test statistic.
/// Disagreement is recorded as a blocking error finding with code
/// [`code::ESTIMATOR_FAMILY_DISAGREEMENT`] because a learner-dependent projection must
/// not certify.  Agreement is recorded as an informational finding only: consensus
/// among families is not evidence of validity and cannot repair a violated sampling
/// contract.  Standard errors must be finite and strictly positive; degenerate scales
/// fail closed as [`EngineError::InvalidLensBattery`] so every reported metric stays
/// finite and serializable.
pub fn audit_lens_battery(
    estimates: &[LensEstimate],
    policy: &PreflightPolicy,
    stage: &str,
    ledger: &mut EvidenceLedger,
) -> Result<LensBatteryAudit, EngineError> {
    if estimates.len() < 2 {
        return Err(EngineError::InvalidLensBattery(
            "at least two estimator families are required for a sensitivity audit".into(),
        ));
    }
    if !policy.lens_gap_tolerance.is_finite() || policy.lens_gap_tolerance <= 0.0 {
        return Err(EngineError::InvalidLensBattery(format!(
            "lens_gap_tolerance must be finite and positive, got {}",
            policy.lens_gap_tolerance
        )));
    }
    let mut labels = BTreeSet::new();
    for lens in estimates {
        if lens.family.trim().is_empty() {
            return Err(EngineError::InvalidLensBattery(
                "family label is empty".into(),
            ));
        }
        if !labels.insert(lens.family.clone()) {
            return Err(EngineError::InvalidLensBattery(format!(
                "family label {} appears more than once",
                lens.family
            )));
        }
        if !lens.estimate.is_finite() {
            return Err(EngineError::InvalidLensBattery(format!(
                "estimate for family {} is not finite",
                lens.family
            )));
        }
        if !lens.standard_error.is_finite() || lens.standard_error <= 0.0 {
            return Err(EngineError::InvalidLensBattery(format!(
                "standard error for family {} must be finite and strictly positive",
                lens.family
            )));
        }
    }
    let mut max_scaled_gap = 0.0_f64;
    let mut worst_pair = [estimates[0].family.clone(), estimates[1].family.clone()];
    for i in 0..estimates.len() {
        for j in (i + 1)..estimates.len() {
            let gap = (estimates[i].estimate - estimates[j].estimate).abs();
            let scale = estimates[i]
                .standard_error
                .hypot(estimates[j].standard_error);
            let scaled_gap = gap / scale;
            if scaled_gap > max_scaled_gap {
                max_scaled_gap = scaled_gap;
                worst_pair = [estimates[i].family.clone(), estimates[j].family.clone()];
            }
        }
    }
    let agrees = max_scaled_gap <= policy.lens_gap_tolerance;
    let mut context = BTreeMap::new();
    context.insert(
        "families".into(),
        labels.iter().cloned().collect::<Vec<_>>().join(","),
    );
    context.insert("max_scaled_gap".into(), format!("{max_scaled_gap:.6}"));
    context.insert(
        "tolerance".into(),
        format!("{:.6}", policy.lens_gap_tolerance),
    );
    context.insert("worst_pair".into(), worst_pair.join(","));
    if agrees {
        ledger.push(finding_with_context(
            Severity::Info,
            stage,
            code::info::ESTIMATOR_FAMILY_AGREEMENT,
            "estimator families agree within the preregistered sensitivity tolerance; agreement is diagnostic, not certifying",
            context,
        ));
    } else {
        ledger.push(finding_with_context(
            Severity::Error,
            stage,
            code::ESTIMATOR_FAMILY_DISAGREEMENT,
            "estimator families disagree beyond tolerance; the projection is learner-dependent and cannot be certified",
            context,
        ));
    }
    Ok(LensBatteryAudit {
        estimates: estimates.to_vec(),
        max_scaled_gap,
        worst_pair,
        tolerance: policy.lens_gap_tolerance,
        agrees,
    })
}

/// Validates the manifest, design geometry, selection contract, and pooled sampling odds.
pub fn run_preflight(
    manifest: &ExperimentManifest,
    policy: PreflightPolicy,
) -> Result<PreflightReport, EngineError> {
    run_preflight_inner(manifest, policy, None)
}

/// Runs preflight with an opaque, content-verified selection-provenance token.
pub fn run_preflight_with_selection_evidence(
    manifest: &ExperimentManifest,
    policy: PreflightPolicy,
    selection_evidence: &ValidatedSelectionEvidence,
) -> Result<PreflightReport, EngineError> {
    run_preflight_inner(manifest, policy, Some(selection_evidence))
}

fn run_preflight_inner(
    manifest: &ExperimentManifest,
    policy: PreflightPolicy,
    selection_evidence: Option<&ValidatedSelectionEvidence>,
) -> Result<PreflightReport, EngineError> {
    manifest.validate()?;
    let manifest_canonical_sha256 = canonical_manifest_sha256(manifest)?;
    let mode = if manifest.strict {
        ExecutionMode::Strict
    } else {
        ExecutionMode::Exploratory
    };
    let mut ledger = EvidenceLedger::new(mode);
    ledger.provenance("experiment_id", &manifest.experiment_id);
    ledger.provenance(
        "manifest_canonical_sha256",
        manifest_canonical_sha256.clone(),
    );
    ledger.provenance("schema_version", &manifest.schema_version);
    ledger.provenance("requested_track", format!("{:?}", manifest.inference_track));
    let product_odds_tolerance = policy.bounded_product_odds_tolerance()?;
    ledger.provenance("product_odds_tolerance", product_odds_tolerance.to_string());
    ledger.provenance(
        "product_odds_tolerance_ceiling",
        mic_stats::ProductDesignEvidence::MAX_PRODUCT_ODDS_TOLERANCE.to_string(),
    );
    ledger.provenance("rank_tolerance", policy.rank_tolerance.to_string());
    ledger.provenance("lens_gap_tolerance", policy.lens_gap_tolerance.to_string());
    ledger.provenance("min_ess_ratio", policy.min_ess_ratio.to_string());
    ledger.provenance(
        "accept_unvalidated_selection_model",
        policy.accept_unvalidated_selection_model.to_string(),
    );

    let points: Vec<DesignPoint> = manifest
        .regimes
        .iter()
        .map(|regime| regime.design.clone())
        .collect();
    let design = audit_design(&points, policy.rank_tolerance)?;
    record_design_geometry(manifest.inference_track, &design, &mut ledger);

    let selection_ok = selection_gate(
        manifest,
        &manifest_canonical_sha256,
        selection_evidence,
        &policy,
        &mut ledger,
    );
    let four_law_geometry_ok = !design.square_faces.is_empty();
    let four_law_eligible = selection_ok && four_law_geometry_ok;
    let proportions: BTreeMap<DesignPoint, f64> = manifest
        .regimes
        .iter()
        .map(|regime| (regime.design.clone(), regime.sampling_proportion))
        .collect();
    let face_sampling = audit_faces(&design.square_faces, &proportions, product_odds_tolerance)?;
    let product_odds_arithmetic_passed =
        !face_sampling.is_empty() && face_sampling.iter().all(|face| face.sampling.is_product);
    // Caller-declared sampling proportions are arithmetic inputs, not a
    // resolved randomization/allocation receipt. No trusted product-design
    // resolver exists in this engine yet, so fail closed.
    let product_factorial_eligible = false;

    record_product_design_authority(
        manifest.inference_track,
        product_odds_arithmetic_passed,
        &mut ledger,
    );

    let requested_eligible = match manifest.inference_track {
        InferenceTrack::FourLaw => four_law_eligible,
        InferenceTrack::ProductFactorial => product_factorial_eligible,
        InferenceTrack::Both => four_law_eligible && product_factorial_eligible,
    };
    let unvalidated_selection_override = matches!(
        manifest.selection,
        SelectionContract::StateIndependentWithinRegime | SelectionContract::Modeled
    ) && policy.accept_unvalidated_selection_model
        && selection_evidence.is_none();
    let status = if manifest.strict && (ledger.has_blocking_error() || !requested_eligible) {
        PreflightStatus::Blocked
    } else if unvalidated_selection_override || !manifest.strict {
        PreflightStatus::DiagnosticOnly
    } else {
        PreflightStatus::Ready
    };

    Ok(PreflightReport {
        schema_version: "1.2.0".into(),
        experiment_id: manifest.experiment_id.clone(),
        manifest_canonical_sha256,
        policy,
        requested_track: manifest.inference_track,
        design,
        face_sampling,
        four_law_eligible,
        product_factorial_eligible,
        status,
        ledger,
    })
}

fn record_product_design_authority(
    track: InferenceTrack,
    product_odds_arithmetic_passed: bool,
    ledger: &mut EvidenceLedger,
) {
    let requires_product = matches!(
        track,
        InferenceTrack::ProductFactorial | InferenceTrack::Both
    );
    if requires_product && !product_odds_arithmetic_passed {
        ledger.note(
            Severity::Error,
            "sampling",
            code::NON_PRODUCT_GCM,
            "product-factorial inference was requested, but at least one required face lacks product pooled odds or no complete face is observed",
        );
    } else if requires_product {
        ledger.note(
            Severity::Error,
            "sampling",
            code::PRODUCT_DESIGN_AUTHORITY_UNRESOLVED,
            "caller-declared quotas pass the product-odds arithmetic check, but no independently trusted allocation or explicit reweighting authority is resolved",
        );
    }
}

fn canonical_manifest_sha256(manifest: &ExperimentManifest) -> Result<String, EngineError> {
    use core::fmt::Write as _;

    let bytes = serde_json::to_vec(manifest).map_err(EngineError::ManifestFingerprint)?;
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing hex into a String cannot fail");
    }
    Ok(encoded)
}

fn hex_sha256(bytes: &[u8]) -> String {
    use core::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing hex into a String cannot fail");
    }
    encoded
}

fn resolve_relative_path(path: &Path, base_dir: Option<&Path>) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base) = base_dir {
        base.join(path)
    } else {
        path.to_path_buf()
    }
}

fn read_bounded_evidence(path: &Path, maximum: u64) -> Result<Vec<u8>, EngineError> {
    use std::io::Read as _;

    let file = std::fs::File::open(path).map_err(EngineError::SelectionEvidenceIo)?;
    let length = file
        .metadata()
        .map_err(EngineError::SelectionEvidenceIo)?
        .len();
    if length > maximum {
        return Err(EngineError::InvalidSelectionEvidence(format!(
            "{} exceeds the {maximum}-byte evidence limit",
            path.display()
        )));
    }
    let capacity = usize::try_from(length.min(maximum)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(EngineError::SelectionEvidenceIo)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(EngineError::InvalidSelectionEvidence(format!(
            "{} grew beyond the {maximum}-byte evidence limit",
            path.display()
        )));
    }
    Ok(bytes)
}

fn sha256_file(path: &Path) -> Result<String, EngineError> {
    use core::fmt::Write as _;
    use std::io::Read as _;

    let mut file = std::fs::File::open(path).map_err(EngineError::SelectionEvidenceIo)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(EngineError::SelectionEvidenceIo)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing hex into a String cannot fail");
    }
    Ok(encoded)
}

fn record_design_geometry(
    track: InferenceTrack,
    design: &DesignAudit,
    ledger: &mut EvidenceLedger,
) {
    ledger.provenance("design_corner_count", design.corner_count.to_string());
    ledger.provenance("square_face_count", design.square_faces.len().to_string());
    let four_law_requests = matches!(track, InferenceTrack::FourLaw | InferenceTrack::Both);
    // Certificate route: a Warning here used to leave one-factor FourLaw Ready.
    let geometry_severity = if four_law_requests {
        Severity::Error
    } else {
        Severity::Warning
    };
    if design.lack_of_fit_dimension == 0 {
        ledger.note(
            geometry_severity,
            "design",
            code::NO_TESTABLE_FLATNESS,
            "the observed design has no lack-of-fit degree of freedom beyond main effects; square flatness is not defined",
        );
    } else if !design.squares_span_lack_of_fit {
        ledger.note(
            geometry_severity,
            "design",
            code::NON_SQUARE_CONTRASTS_REQUIRED,
            "observed square contrasts do not span the full testable lack-of-fit space; a four-law certificate would leave untested directions",
        );
    }
}

fn selection_gate(
    manifest: &ExperimentManifest,
    manifest_canonical_sha256: &str,
    evidence: Option<&ValidatedSelectionEvidence>,
    policy: &PreflightPolicy,
    ledger: &mut EvidenceLedger,
) -> bool {
    if let Some(evidence) = evidence {
        if evidence.experiment_id != manifest.experiment_id
            || evidence.manifest_canonical_sha256 != manifest_canonical_sha256
            || evidence.declaration != manifest.selection
        {
            ledger.note(
                Severity::Error,
                "selection",
                code::SELECTION_MODEL_UNVALIDATED,
                "content-verified selection provenance is scoped to a different experiment, manifest, or declaration",
            );
            return false;
        }
        ledger.provenance("selection_evidence_receipt_id", &evidence.receipt_id);
        ledger.provenance(
            "selection_evidence_receipt_sha256",
            &evidence.receipt_sha256,
        );
        ledger.provenance(
            "selection_evidence_data_sha256",
            &evidence.data_content_sha256,
        );
        ledger.provenance(
            "selection_evidence_authority_source_sha256",
            &evidence.authority_source_sha256,
        );
        ledger.provenance(
            "selection_evidence_class",
            format!("{:?}", evidence.evidence_class),
        );
        ledger.provenance(
            "selection_evidence_authority",
            "content_verified_provenance_only",
        );
        ledger.note(
            Severity::Error,
            "selection",
            code::SELECTION_MODEL_UNVALIDATED,
            "selection receipt bytes and cited source are content-bound, but the source is caller-supplied and no independent trust authority has validated the scientific selection premise",
        );
        return false;
    }
    match manifest.selection {
        SelectionContract::StateIndependentWithinRegime
            if policy.accept_unvalidated_selection_model =>
        {
            ledger.note(
                Severity::Warning,
                "selection",
                code::SELECTION_MODEL_UNVALIDATED,
                "state-independent inclusion is only caller-declared; policy permits diagnostic use, but strict readiness requires separately resolved selection evidence",
            );
            true
        }
        SelectionContract::StateIndependentWithinRegime => {
            ledger.note(
                Severity::Error,
                "selection",
                code::SELECTION_MODEL_UNVALIDATED,
                "state-independent inclusion is only caller-declared; no validated selection evidence is attached",
            );
            false
        }
        SelectionContract::Modeled if policy.accept_unvalidated_selection_model => {
            ledger.note(
                Severity::Warning,
                "selection",
                code::SELECTION_MODEL_UNVALIDATED,
                "a modeled selection process is accepted by policy but still requires diagnostic evidence",
            );
            true
        }
        SelectionContract::Modeled => {
            ledger.note(
                Severity::Error,
                "selection",
                code::SELECTION_MODEL_UNVALIDATED,
                "a selection model was declared but no validated selection evidence is attached",
            );
            false
        }
        SelectionContract::Unknown => {
            ledger.note(
                Severity::Error,
                "selection",
                code::STATE_DEPENDENT_SELECTION,
                "within-regime state dependence of inclusion is unknown",
            );
            false
        }
        SelectionContract::StateDependentUnmodeled => {
            ledger.note(
                Severity::Error,
                "selection",
                code::STATE_DEPENDENT_SELECTION,
                "inclusion depends on state within regime and is not modeled",
            );
            false
        }
    }
}

fn audit_faces(
    faces: &[SquareFace],
    proportions: &BTreeMap<DesignPoint, f64>,
    tolerance: f64,
) -> Result<Vec<FaceSamplingAudit>, EngineError> {
    let mut output = Vec::with_capacity(faces.len());
    for face in faces {
        let corners = face.corners();
        let mut values = [0.0; 4];
        let mut labels = [String::new(), String::new(), String::new(), String::new()];
        for (index, corner) in corners.iter().enumerate() {
            let label = corner.bit_string();
            labels[index].clone_from(&label);
            values[index] = *proportions
                .get(corner)
                .ok_or(EngineError::MissingCorner(label))?;
        }
        output.push(FaceSamplingAudit {
            base: face.base.bit_string(),
            first: face.first,
            second: face.second,
            corners: labels,
            sampling: audit_sampling_odds(values, tolerance)?,
        });
    }
    Ok(output)
}

/// Returns the set of reason codes that block a strict preflight.
#[must_use]
pub fn blocking_codes(report: &PreflightReport) -> BTreeSet<String> {
    report
        .ledger
        .findings()
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .map(|finding| finding.code.clone())
        .collect()
}

/// Builds a structured finding with context for downstream adapters.
#[must_use]
pub fn finding_with_context(
    severity: Severity,
    stage: impl Into<String>,
    code_value: impl Into<String>,
    message: impl Into<String>,
    context: BTreeMap<String, String>,
) -> Finding {
    Finding {
        code: code_value.into(),
        message: message.into(),
        severity,
        stage: stage.into(),
        context,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mic_data::{DataSource, RegimeSpec};
    use std::path::PathBuf;

    fn manifest(probabilities: [f64; 4], strict: bool) -> ExperimentManifest {
        let labels = ["00", "10", "01", "11"];
        ExperimentManifest {
            schema_version: "1.0.0".into(),
            experiment_id: "preflight-test".into(),
            strict,
            inference_track: InferenceTrack::Both,
            selection: SelectionContract::StateIndependentWithinRegime,
            cluster_column: "cluster".into(),
            regime_column: "regime".into(),
            state_columns: vec!["x".into()],
            candidate_state_blocks: Vec::new(),
            regimes: labels
                .iter()
                .zip(probabilities)
                .map(|(label, sampling_proportion)| RegimeSpec {
                    id: (*label).into(),
                    design: DesignPoint::parse(label).unwrap(),
                    sampling_proportion,
                    perturbations: Vec::new(),
                })
                .collect(),
            data: DataSource {
                format: "synthetic".into(),
                path: PathBuf::from("none"),
            },
            seed: 7,
        }
    }

    #[test]
    fn selection_declaration_does_not_mint_readiness() {
        let report = run_preflight(&manifest([0.25; 4], true), PreflightPolicy::default()).unwrap();
        assert_eq!(report.status, PreflightStatus::Blocked);
        assert!(!report.four_law_eligible);
        assert!(!report.product_factorial_eligible);
        assert!(blocking_codes(&report).contains(code::SELECTION_MODEL_UNVALIDATED));
    }

    #[test]
    fn declared_selection_override_is_diagnostic_never_ready() {
        let mut diagnostic_manifest = manifest([0.25; 4], true);
        diagnostic_manifest.inference_track = InferenceTrack::FourLaw;
        let policy = PreflightPolicy {
            accept_unvalidated_selection_model: true,
            ..PreflightPolicy::default()
        };
        let report = run_preflight(&diagnostic_manifest, policy).unwrap();
        assert_eq!(report.status, PreflightStatus::DiagnosticOnly);
        assert!(report.four_law_eligible);
        assert!(!report.product_factorial_eligible);
        assert!(report.ledger.findings().iter().any(|finding| {
            finding.code == code::SELECTION_MODEL_UNVALIDATED
                && finding.severity == Severity::Warning
        }));
    }

    #[test]
    fn content_bound_selection_provenance_cannot_self_mint_strict_readiness() {
        let manifest = manifest([0.25; 4], true);
        let data = b"exact analyzed table bytes";
        let authority = b"external sampling log asserting state-independent inclusion";
        let receipt = serde_json::to_vec(&serde_json::json!({
            "schema_version": "1.0.0",
            "receipt_id": "selection-receipt-001",
            "experiment_id": manifest.experiment_id.clone(),
            "manifest_canonical_sha256": canonical_manifest_sha256(&manifest).unwrap(),
            "data_content_sha256": hex_sha256(data),
            "declaration": "state_independent_within_regime",
            "evidence_class": "external_sampling_record",
            "authority_source_sha256": hex_sha256(authority),
        }))
        .unwrap();
        let evidence = resolve_selection_evidence(&manifest, &receipt, data, authority).unwrap();
        let report =
            run_preflight_with_selection_evidence(&manifest, PreflightPolicy::default(), &evidence)
                .unwrap();
        assert_eq!(report.status, PreflightStatus::Blocked);
        assert!(!report.four_law_eligible);
        assert!(!report.product_factorial_eligible);
        assert!(blocking_codes(&report).contains(code::SELECTION_MODEL_UNVALIDATED));
        assert_eq!(
            report
                .ledger
                .provenance_fields()
                .get("selection_evidence_receipt_id")
                .map(String::as_str),
            Some("selection-receipt-001")
        );
        assert_eq!(
            report
                .ledger
                .provenance_fields()
                .get("selection_evidence_authority")
                .map(String::as_str),
            Some("content_verified_provenance_only")
        );
        assert!(matches!(
            resolve_selection_evidence(&manifest, &receipt, b"mutated data", authority),
            Err(EngineError::InvalidSelectionEvidence(_))
        ));
    }

    #[test]
    fn selection_provenance_token_is_scoped_to_exact_manifest_and_experiment() {
        let original = manifest([0.25; 4], true);
        let data = b"exact analyzed table bytes";
        let authority = b"caller supplied sampling record";
        let receipt = serde_json::to_vec(&serde_json::json!({
            "schema_version": "1.0.0",
            "receipt_id": "selection-receipt-scoped",
            "experiment_id": original.experiment_id.clone(),
            "manifest_canonical_sha256": canonical_manifest_sha256(&original).unwrap(),
            "data_content_sha256": hex_sha256(data),
            "declaration": "state_independent_within_regime",
            "evidence_class": "external_sampling_record",
            "authority_source_sha256": hex_sha256(authority),
        }))
        .unwrap();
        let evidence = resolve_selection_evidence(&original, &receipt, data, authority).unwrap();

        let mut replay_target = original.clone();
        replay_target.experiment_id = "different-experiment".into();
        let report = run_preflight_with_selection_evidence(
            &replay_target,
            PreflightPolicy::default(),
            &evidence,
        )
        .unwrap();
        assert_eq!(report.status(), PreflightStatus::Blocked);
        assert!(blocking_codes(&report).contains(code::SELECTION_MODEL_UNVALIDATED));
        assert!(
            report
                .ledger()
                .findings()
                .iter()
                .any(|finding| { finding.message.contains("scoped to a different experiment") })
        );
    }

    #[test]
    fn preflight_is_content_bound_to_the_complete_validated_manifest() {
        let original = manifest([0.25; 4], true);
        let report = run_preflight(&original, PreflightPolicy::default()).unwrap();
        let fingerprint = report.manifest_canonical_sha256();
        assert_eq!(fingerprint.len(), 64);
        assert!(fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(
            report
                .ledger()
                .provenance_fields()
                .get("manifest_canonical_sha256")
                .map(String::as_str),
            Some(fingerprint)
        );

        let mut changed = original;
        changed.seed += 1;
        let changed_report = run_preflight(&changed, PreflightPolicy::default()).unwrap();
        assert_ne!(
            report.manifest_canonical_sha256(),
            changed_report.manifest_canonical_sha256()
        );
    }

    #[test]
    fn nonproduct_design_blocks_gcm() {
        let report = run_preflight(
            &manifest([0.1, 0.2, 0.3, 0.4], true),
            PreflightPolicy::default(),
        )
        .unwrap();
        assert_eq!(report.status, PreflightStatus::Blocked);
        assert!(blocking_codes(&report).contains(code::NON_PRODUCT_GCM));
    }

    #[test]
    fn exploratory_manifest_never_reports_ready() {
        let report =
            run_preflight(&manifest([0.25; 4], false), PreflightPolicy::default()).unwrap();
        assert_eq!(report.status, PreflightStatus::DiagnosticOnly);
    }

    #[test]
    fn unvalidated_selection_override_is_diagnostic_never_ready() {
        let mut modeled = manifest([0.25; 4], true);
        modeled.selection = SelectionContract::Modeled;
        modeled.inference_track = InferenceTrack::FourLaw;
        let policy = PreflightPolicy {
            accept_unvalidated_selection_model: true,
            ..PreflightPolicy::default()
        };
        let report = run_preflight(&modeled, policy).unwrap();
        assert_eq!(report.status, PreflightStatus::DiagnosticOnly);
        assert!(report.policy().accept_unvalidated_selection_model);
        assert_eq!(report.schema_version(), "1.2.0");
        assert!(
            report
                .ledger
                .findings()
                .iter()
                .any(|finding| finding.code == code::SELECTION_MODEL_UNVALIDATED)
        );
        let blocked = run_preflight(&modeled, PreflightPolicy::default()).unwrap();
        assert!(!blocked.policy().accept_unvalidated_selection_model);
        assert_ne!(report.policy(), blocked.policy());
        assert_eq!(report.status, PreflightStatus::DiagnosticOnly);
        assert_eq!(blocked.status, PreflightStatus::Blocked);
    }

    #[test]
    fn loose_product_odds_tolerance_is_rejected_not_clamped_open() {
        let policy = PreflightPolicy {
            product_odds_tolerance: 0.5,
            ..PreflightPolicy::default()
        };
        let error = run_preflight(&manifest([0.1, 0.2, 0.3, 0.4], true), policy).unwrap_err();
        assert!(matches!(error, EngineError::InvalidPolicy(_)));
    }

    #[test]
    fn one_factor_four_law_is_blocked_not_ready() {
        let mut one_factor = manifest([0.5, 0.5, 0.0, 0.0], true);
        one_factor.inference_track = InferenceTrack::FourLaw;
        one_factor.regimes.truncate(2);
        one_factor.regimes[0].id = "0".into();
        one_factor.regimes[0].design = DesignPoint::parse("0").unwrap();
        one_factor.regimes[0].sampling_proportion = 0.5;
        one_factor.regimes[1].id = "1".into();
        one_factor.regimes[1].design = DesignPoint::parse("1").unwrap();
        one_factor.regimes[1].sampling_proportion = 0.5;
        let report = run_preflight(&one_factor, PreflightPolicy::default()).unwrap();
        assert_eq!(report.status, PreflightStatus::Blocked);
        assert!(!report.four_law_eligible);
        assert!(blocking_codes(&report).contains(code::NO_TESTABLE_FLATNESS));
        assert!(report.design.square_faces.is_empty());
    }

    #[test]
    fn no_square_lack_of_fit_blocks_four_law_certificate_route() {
        let labels = ["001", "010", "011", "100", "101", "110"];
        let hole = ExperimentManifest {
            schema_version: "1.0.0".into(),
            experiment_id: "antipodal-hole".into(),
            strict: true,
            inference_track: InferenceTrack::FourLaw,
            selection: SelectionContract::StateIndependentWithinRegime,
            cluster_column: "cluster".into(),
            regime_column: "regime".into(),
            state_columns: vec!["x".into()],
            candidate_state_blocks: Vec::new(),
            regimes: labels
                .iter()
                .map(|label| RegimeSpec {
                    id: (*label).into(),
                    design: DesignPoint::parse(label).unwrap(),
                    sampling_proportion: 1.0 / 6.0,
                    perturbations: Vec::new(),
                })
                .collect(),
            data: DataSource {
                format: "synthetic".into(),
                path: PathBuf::from("none"),
            },
            seed: 7,
        };
        let report = run_preflight(&hole, PreflightPolicy::default()).unwrap();
        assert_eq!(report.status, PreflightStatus::Blocked);
        assert!(!report.four_law_eligible);
        assert!(blocking_codes(&report).contains(code::NON_SQUARE_CONTRASTS_REQUIRED));
    }

    fn lens(family: &str, estimate: f64, standard_error: f64) -> LensEstimate {
        LensEstimate {
            family: family.into(),
            estimate,
            standard_error,
        }
    }

    #[test]
    fn agreeing_lens_battery_keeps_strict_run_unblocked() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let audit = audit_lens_battery(
            &[
                lens("linear", 0.10, 0.05),
                lens("kernel", 0.12, 0.05),
                lens("boosted_tree", 0.08, 0.06),
            ],
            &PreflightPolicy::default(),
            "curvature",
            &mut ledger,
        )
        .unwrap();
        assert!(audit.agrees);
        assert!(!ledger.has_blocking_error());
        assert_eq!(ledger.findings().len(), 1);
        assert_eq!(ledger.findings()[0].severity, Severity::Info);
    }

    #[test]
    fn disagreeing_lens_battery_blocks_strict_run() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let audit = audit_lens_battery(
            &[lens("linear", 0.10, 0.01), lens("kernel", 0.90, 0.01)],
            &PreflightPolicy::default(),
            "curvature",
            &mut ledger,
        )
        .unwrap();
        assert!(!audit.agrees);
        assert_eq!(
            audit.worst_pair,
            ["linear".to_string(), "kernel".to_string()]
        );
        assert!(ledger.has_blocking_error());
        assert!(
            ledger
                .findings()
                .iter()
                .any(|finding| finding.code == code::ESTIMATOR_FAMILY_DISAGREEMENT)
        );
    }

    #[test]
    fn zero_standard_error_fails_closed() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let error = audit_lens_battery(
            &[lens("linear", 0.0, 0.0), lens("kernel", 0.1, 0.0)],
            &PreflightPolicy::default(),
            "curvature",
            &mut ledger,
        )
        .unwrap_err();
        assert!(matches!(error, EngineError::InvalidLensBattery(_)));
        assert!(ledger.findings().is_empty());
    }

    #[test]
    fn unique_pass_pattern_is_informational_and_not_causal_authority() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let deletions = vec![
            mic_stats::classify_deletion("t", 0.01, 0.0, 0.02, 0.05).unwrap(),
            mic_stats::classify_deletion("p", 0.9, 0.6, 1.2, 0.05).unwrap(),
        ];
        let audit = audit_orientation(&deletions, 1.0, 0.1, "orientation", &mut ledger).unwrap();
        assert_eq!(
            audit.outcome(),
            &mic_stats::OrientationOutcome::UniquePassPattern {
                variable: "t".into()
            }
        );
        assert!(!ledger.has_blocking_error());
    }

    #[test]
    fn parity_multiple_passes_blocks_strict_run() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let deletions = vec![
            mic_stats::classify_deletion("P", 0.01, 0.0, 0.02, 0.05).unwrap(),
            mic_stats::classify_deletion("T", 0.01, 0.0, 0.02, 0.05).unwrap(),
        ];
        let audit = audit_orientation(&deletions, 1.0, 0.1, "orientation", &mut ledger).unwrap();
        assert!(matches!(
            audit.outcome(),
            mic_stats::OrientationOutcome::MultiplePasses { .. }
        ));
        assert!(ledger.has_blocking_error());
        assert!(
            ledger
                .findings()
                .iter()
                .any(|finding| finding.code == code::ORIENTATION_UNRESOLVED)
        );
        assert_eq!(
            ledger.status(&mic_audit::CertificateGates::unresolved()),
            mic_audit::CertificateStatus::Abstained
        );
    }

    #[test]
    fn concentrated_weights_fail_overlap_gate() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let mut weights = vec![0.001; 100];
        weights[0] = 1000.0;
        let audit = audit_overlap(
            &weights,
            &PreflightPolicy::default(),
            "overlap",
            &mut ledger,
        )
        .unwrap();
        assert!(!audit.adequate);
        assert!(audit.ess_ratio < 0.1);
        assert!(
            ledger
                .findings()
                .iter()
                .any(|finding| finding.code == code::OVERLAP_FAILURE)
        );
    }

    #[test]
    fn uniform_weights_pass_overlap_gate() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let audit = audit_overlap(
            &vec![1.0; 64],
            &PreflightPolicy::default(),
            "overlap",
            &mut ledger,
        )
        .unwrap();
        assert!(audit.adequate);
        assert!((audit.ess_ratio - 1.0).abs() < 1e-12);
        assert!(!ledger.has_blocking_error());
    }

    #[test]
    fn lens_audit_serializes_to_finite_json() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let audit = audit_lens_battery(
            &[lens("linear", 0.10, 0.01), lens("kernel", 0.90, 0.01)],
            &PreflightPolicy::default(),
            "curvature",
            &mut ledger,
        )
        .unwrap();
        let encoded = serde_json::to_string(&audit).unwrap();
        assert!(encoded.contains("max_scaled_gap"));
    }

    #[test]
    fn single_family_battery_is_rejected() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let error = audit_lens_battery(
            &[lens("linear", 0.0, 0.1)],
            &PreflightPolicy::default(),
            "curvature",
            &mut ledger,
        )
        .unwrap_err();
        assert!(matches!(error, EngineError::InvalidLensBattery(_)));
        assert!(ledger.findings().is_empty());
    }
}
