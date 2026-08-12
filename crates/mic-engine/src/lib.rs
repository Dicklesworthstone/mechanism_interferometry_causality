#![forbid(unsafe_code)]
//! Fail-closed preflight orchestration for mechanism-interferometry analyses.

use mic_audit::{EvidenceLedger, ExecutionMode, Finding, Severity, code};
use mic_data::{ExperimentManifest, InferenceTrack, ManifestError, SelectionContract, TableError};
use mic_design::{
    DesignAudit, DesignError, DesignPoint, SamplingOddsAudit, SquareFace, audit_design,
    audit_sampling_odds,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

mod survey;
mod tabular;
pub use survey::{
    ClusterUnitBasis, ColumnRole, ColumnTriage, InterferometerProposal, SurveyAuthority,
    SurveyPolicy, SurveyReport, run_unsupervised_survey,
};
pub use tabular::{
    CellCurvature, ColumnProjection, FourLawFaceAudit, FourLawPolicy, ProjectionSpec,
    TabularAuditReport, TabularIngestSummary, run_tabular_audit,
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

/// Machine-readable preflight result.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreflightReport {
    /// Schema version.
    schema_version: String,
    /// Experiment identifier.
    experiment_id: String,
    /// Requested inference track.
    requested_track: InferenceTrack,
    /// Main-effects and lack-of-fit geometry.
    design: DesignAudit,
    /// Sampling-odds audits for every observed square.
    face_sampling: Vec<FaceSamplingAudit>,
    /// Whether four-law functionals are permitted by the declared selection contract.
    four_law_eligible: bool,
    /// Whether all observed pair faces have product pooled odds.
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

    /// Returns whether every observed square passed the product-odds audit.
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrientationAudit {
    /// Classified deletions in input order.
    pub deletions: Vec<mic_stats::DeletionEquivalence>,
    /// Full-support intervention discrepancy.
    pub full_discrepancy: f64,
    /// Power threshold below which the audit abstains as underpowered.
    pub min_full_discrepancy: f64,
    /// Five-state pass-count outcome.
    pub outcome: mic_stats::OrientationOutcome,
}

/// Runs the pass-count state machine and records the verdict in the ledger.
///
/// Only the unique-target state orients a family and is recorded as an
/// informational finding.  Every other state is recorded as a blocking error
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
        mic_stats::OrientationOutcome::UniqueTarget { target } => {
            context.insert("target".into(), target.clone());
            ledger.push(finding_with_context(
                Severity::Info,
                stage,
                "orientation_unique_target",
                "exactly one deletion is certified invariant and every competitor is certified changed",
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverlapAudit {
    /// Kish effective sample size of the ratio weights.
    pub effective_sample_size: f64,
    /// Number of observations carrying weights.
    pub sample_size: usize,
    /// Effective sample size divided by sample size.
    pub ess_ratio: f64,
    /// Policy floor the ratio was compared against.
    pub minimum_ratio: f64,
    /// Whether the overlap is adequate under the policy.
    pub adequate: bool,
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
            "overlap_adequate",
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LensBatteryAudit {
    /// The estimates in input order.
    pub estimates: Vec<LensEstimate>,
    /// Largest standard-error-scaled pairwise gap observed across the battery.
    pub max_scaled_gap: f64,
    /// The pair of family labels achieving the largest scaled gap.
    pub worst_pair: [String; 2],
    /// Tolerance applied to the scaled gaps.
    pub tolerance: f64,
    /// Whether every scaled gap is within tolerance.
    pub agrees: bool,
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
            "estimator_family_agreement",
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
    manifest.validate()?;
    let mode = if manifest.strict {
        ExecutionMode::Strict
    } else {
        ExecutionMode::Exploratory
    };
    let mut ledger = EvidenceLedger::new(mode);
    ledger.provenance("experiment_id", &manifest.experiment_id);
    ledger.provenance("schema_version", &manifest.schema_version);
    ledger.provenance("requested_track", format!("{:?}", manifest.inference_track));
    let product_odds_tolerance = policy.bounded_product_odds_tolerance()?;
    ledger.provenance("product_odds_tolerance", product_odds_tolerance.to_string());
    ledger.provenance(
        "product_odds_tolerance_ceiling",
        mic_stats::ProductDesignEvidence::MAX_PRODUCT_ODDS_TOLERANCE.to_string(),
    );

    let points: Vec<DesignPoint> = manifest
        .regimes
        .iter()
        .map(|regime| regime.design.clone())
        .collect();
    let design = audit_design(&points, policy.rank_tolerance)?;
    record_design_geometry(manifest.inference_track, &design, &mut ledger);

    let selection_ok = selection_gate(manifest.selection, &policy, &mut ledger);
    let four_law_geometry_ok = !design.square_faces.is_empty();
    let four_law_eligible = selection_ok && four_law_geometry_ok;
    let proportions: BTreeMap<DesignPoint, f64> = manifest
        .regimes
        .iter()
        .map(|regime| (regime.design.clone(), regime.sampling_proportion))
        .collect();
    let face_sampling = audit_faces(&design.square_faces, &proportions, product_odds_tolerance)?;
    let product_factorial_eligible = !face_sampling.is_empty()
        && face_sampling.iter().all(|face| face.sampling.is_product)
        && four_law_eligible;

    let requires_product = matches!(
        manifest.inference_track,
        InferenceTrack::ProductFactorial | InferenceTrack::Both
    );
    if requires_product && !product_factorial_eligible {
        ledger.note(
            Severity::Error,
            "sampling",
            code::NON_PRODUCT_GCM,
            "product-factorial inference was requested, but at least one required face lacks product pooled odds or no complete face is observed",
        );
    }

    let requested_eligible = match manifest.inference_track {
        InferenceTrack::FourLaw => four_law_eligible,
        InferenceTrack::ProductFactorial => product_factorial_eligible,
        InferenceTrack::Both => four_law_eligible && product_factorial_eligible,
    };
    let unvalidated_selection_override = matches!(manifest.selection, SelectionContract::Modeled)
        && policy.accept_unvalidated_selection_model;
    let status = if unvalidated_selection_override {
        PreflightStatus::DiagnosticOnly
    } else if manifest.strict {
        if ledger.has_blocking_error() || !requested_eligible {
            PreflightStatus::Blocked
        } else {
            PreflightStatus::Ready
        }
    } else {
        PreflightStatus::DiagnosticOnly
    };

    Ok(PreflightReport {
        schema_version: "1.0.0".into(),
        experiment_id: manifest.experiment_id.clone(),
        requested_track: manifest.inference_track,
        design,
        face_sampling,
        four_law_eligible,
        product_factorial_eligible,
        status,
        ledger,
    })
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
    selection: SelectionContract,
    policy: &PreflightPolicy,
    ledger: &mut EvidenceLedger,
) -> bool {
    match selection {
        SelectionContract::StateIndependentWithinRegime => true,
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
    fn product_design_is_ready() {
        let report = run_preflight(&manifest([0.25; 4], true), PreflightPolicy::default()).unwrap();
        assert_eq!(report.status, PreflightStatus::Ready);
        assert!(report.product_factorial_eligible);
        assert!(blocking_codes(&report).is_empty());
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
        let policy = PreflightPolicy {
            accept_unvalidated_selection_model: true,
            ..PreflightPolicy::default()
        };
        let report = run_preflight(&modeled, policy).unwrap();
        assert_eq!(report.status, PreflightStatus::DiagnosticOnly);
        assert!(
            report
                .ledger
                .findings()
                .iter()
                .any(|finding| finding.code == code::SELECTION_MODEL_UNVALIDATED)
        );
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
    fn unique_target_orientation_is_informational() {
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let deletions = vec![
            mic_stats::classify_deletion("t", 0.01, 0.0, 0.02, 0.05).unwrap(),
            mic_stats::classify_deletion("p", 0.9, 0.6, 1.2, 0.05).unwrap(),
        ];
        let audit = audit_orientation(&deletions, 1.0, 0.1, "orientation", &mut ledger).unwrap();
        assert_eq!(
            audit.outcome,
            mic_stats::OrientationOutcome::UniqueTarget { target: "t".into() }
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
            audit.outcome,
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
