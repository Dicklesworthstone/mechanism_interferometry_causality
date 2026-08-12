#![forbid(unsafe_code)]
//! Fail-closed preflight orchestration for mechanism-interferometry analyses.

use mic_audit::{EvidenceLedger, ExecutionMode, Finding, Severity, code};
use mic_data::{ExperimentManifest, InferenceTrack, ManifestError, SelectionContract};
use mic_design::{
    DesignAudit, DesignError, DesignPoint, SamplingOddsAudit, SquareFace, audit_design,
    audit_sampling_odds,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

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
}

impl Default for PreflightPolicy {
    fn default() -> Self {
        Self {
            rank_tolerance: 1e-10,
            product_odds_tolerance: 1e-10,
            accept_unvalidated_selection_model: false,
            lens_gap_tolerance: 3.0,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreflightReport {
    /// Schema version.
    pub schema_version: String,
    /// Experiment identifier.
    pub experiment_id: String,
    /// Requested inference track.
    pub requested_track: InferenceTrack,
    /// Main-effects and lack-of-fit geometry.
    pub design: DesignAudit,
    /// Sampling-odds audits for every observed square.
    pub face_sampling: Vec<FaceSamplingAudit>,
    /// Whether four-law functionals are permitted by the declared selection contract.
    pub four_law_eligible: bool,
    /// Whether all observed pair faces have product pooled odds.
    pub product_factorial_eligible: bool,
    /// Conservative preflight state.
    pub status: PreflightStatus,
    /// Evidence and reason codes.
    pub ledger: EvidenceLedger,
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

    let points: Vec<DesignPoint> = manifest
        .regimes
        .iter()
        .map(|regime| regime.design.clone())
        .collect();
    let design = audit_design(&points, policy.rank_tolerance)?;
    if design.lack_of_fit_dimension == 0 {
        ledger.note(
            Severity::Warning,
            "design",
            code::NO_TESTABLE_FLATNESS,
            "the observed design has no lack-of-fit degree of freedom beyond main effects",
        );
    } else if !design.squares_span_lack_of_fit {
        ledger.note(
            Severity::Warning,
            "design",
            code::NON_SQUARE_CONTRASTS_REQUIRED,
            "observed square contrasts do not span the full testable lack-of-fit space",
        );
    }

    let four_law_eligible = selection_gate(manifest.selection, &policy, &mut ledger);
    let proportions: BTreeMap<DesignPoint, f64> = manifest
        .regimes
        .iter()
        .map(|regime| (regime.design.clone(), regime.sampling_proportion))
        .collect();
    let face_sampling = audit_faces(
        &design.square_faces,
        &proportions,
        policy.product_odds_tolerance,
    )?;
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
    let status = if manifest.strict {
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
        .findings
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
        assert_eq!(ledger.findings.len(), 1);
        assert_eq!(ledger.findings[0].severity, Severity::Info);
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
                .findings
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
        assert!(ledger.findings.is_empty());
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
        assert!(ledger.findings.is_empty());
    }
}
