#![forbid(unsafe_code)]
//! Transport-nuisance refit stability conditional on frozen features.
//!
//! Replicates sample clusters without replacement within each arm, retain each
//! selected unit intact, refit every primitive nuisance model, and rescore a
//! separately sampled combination set. The empirical quantiles are descriptive
//! stability summaries, not calibrated confidence intervals or equivalence
//! tests.

use std::{collections::BTreeMap, fmt::Write as _};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CombinationConfirmationSample, FittedCombinationPredictionReport, FittedTransportAuthority,
    FittedTransportError, FrozenFeatureContract, PrimitiveArm, PrimitiveTransportConfig,
    PrimitiveTransportSample, freeze_primitive_transport, score_combination_confirmation,
};

const MIN_REFITS: usize = 20;
const MAX_REFITS: usize = 1_000;
const MIN_COMPLETION_FRACTION: f64 = 0.8;
const MAX_REFIT_WORK_UNITS: usize = 1_000_000_000;
const MAX_REFIT_ENERGY_WORK_UNITS: usize = 1_000_000_000;
const MAX_REFIT_IDENTIFIER_BYTE_WORK: usize = 1_000_000_000;
const MAX_EXACT_DISTANCE_ROWS: usize = 4_096;

/// Closed configuration for deterministic cluster subsampling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransportRefitConfig {
    /// Seed used only for the outer refit plan.
    pub seed: u64,
    /// Number of complete nuisance-refit attempts.
    pub n_refits: usize,
    /// Fraction of clusters retained independently within every arm.
    pub retain_fraction: f64,
}

/// Non-certificate state of the empirical refit distribution.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransportRefitStatus {
    /// Every requested transport-nuisance refit completed.
    Complete,
    /// Some, but not all, requested refits completed.
    Partial,
    /// No refit completed or fewer than two distinct joint plans succeeded.
    Insufficient,
}

/// Why empirical refit quantiles are present or suppressed.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefitQuantileStatus {
    /// Every reported quantile conditions on the subset of successful refits.
    ReportedConditionalOnSuccessfulRefits,
    /// Fewer than two distinct successful plans informed this metric.
    SuppressedDegenerateResampling,
    /// Too few requested refits completed to meet the descriptive reporting floor.
    SuppressedCompletionBelowFloor,
    /// At least one completed score omitted the exact-distance calculation.
    SuppressedIncompleteEnergyBudget,
}

/// Treatment of the feature transform during transport refits.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefitFeatureTransformTreatment {
    /// The externally supplied transform remains fixed in every refit.
    FrozenNotRefit,
}

/// One central empirical quantile summary.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EmpiricalRefitQuantiles {
    /// Number of finite completed values summarized.
    pub n_values: usize,
    /// Empirical 2.5th percentile.
    pub q025: f64,
    /// Empirical median.
    pub q500: f64,
    /// Empirical 97.5th percentile.
    pub q975: f64,
}

/// Realized fixed cluster counts in every deterministic subsample.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealizedRefitCounts {
    /// `(retained, total)` baseline clusters.
    pub baseline: (usize, usize),
    /// `(retained, total)` first-arm clusters.
    pub first: (usize, usize),
    /// `(retained, total)` second-arm clusters.
    pub second: (usize, usize),
    /// `(retained, total)` combination clusters.
    pub combination: (usize, usize),
}

/// Seed-specific inclusion coverage for one cluster stratum.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RefitStratumCoverage {
    /// Number of distinct clusters available in the stratum.
    pub total_clusters: usize,
    /// Number selected at least once across the relevant plans.
    pub ever_selected_clusters: usize,
    /// Fraction of available clusters selected at least once.
    pub coverage_fraction: f64,
    /// Minimum number of selections over all available clusters, including zero.
    pub min_inclusions: usize,
    /// Maximum number of selections over all available clusters.
    pub max_inclusions: usize,
}

/// Seed-specific inclusion coverage in all four arms.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RefitSelectionCoverage {
    /// Baseline-arm coverage.
    pub baseline: RefitStratumCoverage,
    /// First-arm coverage.
    pub first: RefitStratumCoverage,
    /// Second-arm coverage.
    pub second: RefitStratumCoverage,
    /// Combination-arm coverage.
    pub combination: RefitStratumCoverage,
}

/// Seed-specific inclusion coverage for the three primitive arms.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrimitiveRefitSelectionCoverage {
    /// Baseline-arm coverage.
    pub baseline: RefitStratumCoverage,
    /// First-arm coverage.
    pub first: RefitStratumCoverage,
    /// Second-arm coverage.
    pub second: RefitStratumCoverage,
}

/// Diversity of attempted and successful deterministic subset plans.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RefitPlanDiversity {
    /// Distinct primitive selections attempted.
    pub attempted_primitive: usize,
    /// Distinct combination selections attempted.
    pub attempted_combination: usize,
    /// Distinct joint selections attempted.
    pub attempted_joint: usize,
    /// Distinct primitive selections whose Stage-A fit succeeded.
    pub successful_primitive: usize,
    /// Distinct combination selections whose complete score succeeded.
    pub successful_combination: usize,
    /// Distinct joint selections whose complete score succeeded.
    pub successful_joint: usize,
}

/// Metric-specific eligibility for descriptive empirical quantiles.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RefitMetricQuantileStatus {
    /// Raw-normalizer and cluster-ESS quantiles need varied successful Stage A.
    pub primitive_metrics: RefitQuantileStatus,
    /// Proper-score quantiles need varied successful joint plans.
    pub proper_score: RefitQuantileStatus,
    /// Energy quantiles additionally require uniform exact-distance computation.
    pub energy: RefitQuantileStatus,
}

/// Pipeline stage at which one refit failed.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransportRefitFailureStage {
    /// Primitive-only nuisance fitting or transport construction failed.
    PrimitiveFit,
    /// Separately supplied combination scoring failed.
    CombinationScore,
}

/// One retained failure rather than a silently dropped replicate.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TransportRefitFailure {
    /// Zero-based deterministic refit index.
    pub refit: usize,
    /// Pipeline stage that returned the error.
    pub stage: TransportRefitFailureStage,
    /// Exact typed error display from that stage.
    pub reason: String,
}

/// Immutable descriptive output of transport-nuisance cluster refits.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransportRefitReport {
    authority: FittedTransportAuthority,
    certificate_eligible: bool,
    calibrated_test: bool,
    method: &'static str,
    status: TransportRefitStatus,
    config: TransportRefitConfig,
    resample_plan_sha256: String,
    point: FittedCombinationPredictionReport,
    n_requested: usize,
    n_primitive_completed: usize,
    n_completed: usize,
    primitive_completion_fraction: f64,
    completion_fraction: f64,
    failures: Vec<TransportRefitFailure>,
    realized_counts: RealizedRefitCounts,
    plan_diversity: RefitPlanDiversity,
    attempted_selection_coverage: RefitSelectionCoverage,
    primitive_metric_success_selection_coverage: PrimitiveRefitSelectionCoverage,
    joint_metric_success_selection_coverage: RefitSelectionCoverage,
    metric_quantile_status: RefitMetricQuantileStatus,
    raw_normalizer_residual: Option<EmpiricalRefitQuantiles>,
    proper_score_gain: Option<EmpiricalRefitQuantiles>,
    effective_baseline_clusters: Option<EmpiricalRefitQuantiles>,
    energy_n_completed: usize,
    energy_omitted_by_budget: usize,
    predicted_vs_heldout_energy: Option<EmpiricalRefitQuantiles>,
    feature_transform_treatment: RefitFeatureTransformTreatment,
    interpretation: &'static str,
    order_statistic_convention: &'static str,
}

impl TransportRefitReport {
    /// Number of completed transport-nuisance refits.
    #[must_use]
    pub const fn n_completed(&self) -> usize {
        self.n_completed
    }

    /// Empirical refit quantiles of the raw-normalizer residual.
    #[must_use]
    pub const fn raw_normalizer_residual(&self) -> Option<EmpiricalRefitQuantiles> {
        self.raw_normalizer_residual
    }

    /// Empirical refit quantiles of held-out proper-score gain.
    #[must_use]
    pub const fn proper_score_gain(&self) -> Option<EmpiricalRefitQuantiles> {
        self.proper_score_gain
    }

    /// Content binding for the complete deterministic resample plan.
    #[must_use]
    pub fn resample_plan_sha256(&self) -> &str {
        &self.resample_plan_sha256
    }
}

/// Fail-closed refit configuration or point-estimate errors.
#[derive(Debug, Error)]
pub enum TransportRefitError {
    /// Refit count is outside the fixed resource and quantile floor.
    #[error("n_refits must be in {MIN_REFITS}..={MAX_REFITS}")]
    InvalidRefitCount,
    /// Retained fraction is not finite and strictly between zero and one.
    #[error("retain_fraction must be finite and lie strictly between zero and one")]
    InvalidRetainFraction,
    /// Point estimate could not be constructed.
    #[error(transparent)]
    PointEstimate(#[from] FittedTransportError),
    /// An input count exceeded a fixed-width fingerprint field.
    #[error("refit input exceeds the supported fixed-width fingerprint range")]
    FingerprintOverflow,
    /// Requested rows, folds, iterations, features, and refits exceed budget.
    #[error("transport refit request exceeds the fixed aggregate work budget")]
    WorkBudgetExceeded,
}

/// Validates the outer refit contract and its primitive-only work budget.
///
/// The CLI uses this before opening the separately supplied combination file,
/// so an adversarial fit configuration cannot consume unbounded work before
/// the stage boundary is established. The complete function below performs a
/// second, stricter budget check after Stage B becomes available.
pub fn validate_primitive_refit_request(
    primitive: &[PrimitiveTransportSample],
    transport: PrimitiveTransportConfig,
    refits: TransportRefitConfig,
) -> Result<(), TransportRefitError> {
    validate_refit_config(refits)?;
    validate_work_budget(primitive, &[], transport, refits)
}

/// Runs the point diagnostic and deterministic stratified cluster refits.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn refit_transport_uncertainty(
    primitive: &[PrimitiveTransportSample],
    confirmation: &[CombinationConfirmationSample],
    sampling_proportions: [f64; 3],
    declared_independent_unit: &str,
    feature_contract: &FrozenFeatureContract,
    transport_config: PrimitiveTransportConfig,
    refit_config: TransportRefitConfig,
) -> Result<TransportRefitReport, TransportRefitError> {
    validate_refit_config(refit_config)?;
    validate_work_budget(primitive, confirmation, transport_config, refit_config)?;
    let point_frozen = freeze_primitive_transport(
        primitive,
        sampling_proportions,
        declared_independent_unit,
        feature_contract.clone(),
        transport_config,
    )?;
    let point = score_combination_confirmation(
        &point_frozen,
        declared_independent_unit,
        feature_contract,
        confirmation,
    )?;

    let refits = collect_refits(
        primitive,
        confirmation,
        sampling_proportions,
        declared_independent_unit,
        feature_contract,
        transport_config,
        refit_config,
    )?;
    let n_primitive_completed = refits.raw_residuals.len();
    let n_completed = refits.score_gains.len();
    #[allow(clippy::cast_precision_loss)]
    let primitive_completion_fraction = n_primitive_completed as f64 / refit_config.n_refits as f64;
    #[allow(clippy::cast_precision_loss)]
    let completion_fraction = n_completed as f64 / refit_config.n_refits as f64;
    let primitive_quantile_status = refit_quantile_status(
        refits.plan_diversity.successful_primitive,
        primitive_completion_fraction,
    );
    let score_quantile_status =
        refit_quantile_status(refits.plan_diversity.successful_joint, completion_fraction);
    let primitive_quantiles_reported = matches!(
        primitive_quantile_status,
        RefitQuantileStatus::ReportedConditionalOnSuccessfulRefits
    );
    let score_quantiles_reported = matches!(
        score_quantile_status,
        RefitQuantileStatus::ReportedConditionalOnSuccessfulRefits
    );
    let status = if n_completed == 0 || refits.plan_diversity.successful_joint < 2 {
        TransportRefitStatus::Insufficient
    } else if n_completed == refit_config.n_refits {
        TransportRefitStatus::Complete
    } else {
        TransportRefitStatus::Partial
    };
    let energy_n_completed = refits.energies.len();
    let energy_omitted_by_budget = n_completed.saturating_sub(energy_n_completed);
    let energy_quantile_status =
        energy_refit_quantile_status(score_quantile_status, refits.energies.len(), n_completed);
    let energy_quantiles = matches!(
        energy_quantile_status,
        RefitQuantileStatus::ReportedConditionalOnSuccessfulRefits
    )
    .then(|| empirical_quantiles(refits.energies.clone()))
    .flatten();
    Ok(TransportRefitReport {
        authority: FittedTransportAuthority::DiagnosticOnly,
        certificate_eligible: false,
        calibrated_test: false,
        method: "transport_nuisance_refits_conditional_on_frozen_features",
        status,
        config: refit_config,
        resample_plan_sha256: refits.plan_sha256,
        point,
        n_requested: refit_config.n_refits,
        n_primitive_completed,
        n_completed,
        primitive_completion_fraction,
        completion_fraction,
        failures: refits.failures,
        realized_counts: refits.realized_counts,
        plan_diversity: refits.plan_diversity,
        attempted_selection_coverage: refits.attempted_selection_coverage,
        primitive_metric_success_selection_coverage: refits
            .primitive_metric_success_selection_coverage,
        joint_metric_success_selection_coverage: refits.joint_metric_success_selection_coverage,
        metric_quantile_status: RefitMetricQuantileStatus {
            primitive_metrics: primitive_quantile_status,
            proper_score: score_quantile_status,
            energy: energy_quantile_status,
        },
        raw_normalizer_residual: primitive_quantiles_reported
            .then(|| empirical_quantiles(refits.raw_residuals))
            .flatten(),
        proper_score_gain: score_quantiles_reported
            .then(|| empirical_quantiles(refits.score_gains))
            .flatten(),
        effective_baseline_clusters: primitive_quantiles_reported
            .then(|| empirical_quantiles(refits.cluster_ess))
            .flatten(),
        energy_n_completed,
        energy_omitted_by_budget,
        predicted_vs_heldout_energy: energy_quantiles,
        feature_transform_treatment: RefitFeatureTransformTreatment::FrozenNotRefit,
        interpretation: "seed-specific subset-ensemble sensitivity conditional on frozen features, realized cluster counts, metric-specific successful refits, and reported inclusion coverage; not a bootstrap sampling distribution, confidence interval, equivalence test, robustness guarantee, or causal certificate",
        order_statistic_convention: "indices round 0.025*(n-1), use the upper median, and round 0.975*(n-1); at n=20 the outer values are the observed minimum and maximum",
    })
}

fn refit_quantile_status(
    unique_successful_plans: usize,
    completion_fraction: f64,
) -> RefitQuantileStatus {
    if unique_successful_plans < 2 {
        RefitQuantileStatus::SuppressedDegenerateResampling
    } else if completion_fraction >= MIN_COMPLETION_FRACTION {
        RefitQuantileStatus::ReportedConditionalOnSuccessfulRefits
    } else {
        RefitQuantileStatus::SuppressedCompletionBelowFloor
    }
}

fn energy_refit_quantile_status(
    score_status: RefitQuantileStatus,
    n_energies: usize,
    n_completed: usize,
) -> RefitQuantileStatus {
    if !matches!(
        score_status,
        RefitQuantileStatus::ReportedConditionalOnSuccessfulRefits
    ) {
        score_status
    } else if n_energies == n_completed {
        RefitQuantileStatus::ReportedConditionalOnSuccessfulRefits
    } else {
        RefitQuantileStatus::SuppressedIncompleteEnergyBudget
    }
}

struct RefitCollection {
    plan_sha256: String,
    failures: Vec<TransportRefitFailure>,
    raw_residuals: Vec<f64>,
    score_gains: Vec<f64>,
    cluster_ess: Vec<f64>,
    energies: Vec<f64>,
    realized_counts: RealizedRefitCounts,
    plan_diversity: RefitPlanDiversity,
    attempted_selection_coverage: RefitSelectionCoverage,
    primitive_metric_success_selection_coverage: PrimitiveRefitSelectionCoverage,
    joint_metric_success_selection_coverage: RefitSelectionCoverage,
}

struct SelectionCoverageAccumulator {
    primitive: BTreeMap<PrimitiveArm, BTreeMap<String, usize>>,
    combination: BTreeMap<String, usize>,
}

impl SelectionCoverageAccumulator {
    fn new(primitive: &BTreeMap<PrimitiveArm, Vec<String>>, combination: &[String]) -> Self {
        Self {
            primitive: primitive
                .iter()
                .map(|(arm, ids)| (*arm, ids.iter().map(|id| (id.clone(), 0)).collect()))
                .collect(),
            combination: combination.iter().map(|id| (id.clone(), 0)).collect(),
        }
    }

    fn record_primitive(&mut self, selected: &BTreeMap<PrimitiveArm, Vec<String>>) {
        for (arm, ids) in selected {
            let counts = self
                .primitive
                .get_mut(arm)
                .expect("selection arm comes from the initialized cluster groups");
            for id in ids {
                *counts
                    .get_mut(id)
                    .expect("selected cluster comes from the initialized cluster groups") += 1;
            }
        }
    }

    fn record_combination(&mut self, selected: &[String]) {
        for id in selected {
            *self
                .combination
                .get_mut(id)
                .expect("selected cluster comes from the initialized combination groups") += 1;
        }
    }

    fn finish(self) -> RefitSelectionCoverage {
        let summary = |arm| {
            summarize_coverage(
                self.primitive
                    .get(&arm)
                    .expect("all primitive arms are present after Stage-A validation"),
            )
        };
        RefitSelectionCoverage {
            baseline: summary(PrimitiveArm::Baseline),
            first: summary(PrimitiveArm::First),
            second: summary(PrimitiveArm::Second),
            combination: summarize_coverage(&self.combination),
        }
    }

    fn finish_primitive(self) -> PrimitiveRefitSelectionCoverage {
        let summary = |arm| {
            summarize_coverage(
                self.primitive
                    .get(&arm)
                    .expect("all primitive arms are present after Stage-A validation"),
            )
        };
        PrimitiveRefitSelectionCoverage {
            baseline: summary(PrimitiveArm::Baseline),
            first: summary(PrimitiveArm::First),
            second: summary(PrimitiveArm::Second),
        }
    }
}

fn summarize_coverage(counts: &BTreeMap<String, usize>) -> RefitStratumCoverage {
    let total_clusters = counts.len();
    let ever_selected_clusters = counts.values().filter(|count| **count > 0).count();
    #[allow(clippy::cast_precision_loss)]
    let coverage_fraction = if total_clusters == 0 {
        0.0
    } else {
        ever_selected_clusters as f64 / total_clusters as f64
    };
    RefitStratumCoverage {
        total_clusters,
        ever_selected_clusters,
        coverage_fraction,
        min_inclusions: counts.values().copied().min().unwrap_or(0),
        max_inclusions: counts.values().copied().max().unwrap_or(0),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn collect_refits(
    primitive: &[PrimitiveTransportSample],
    confirmation: &[CombinationConfirmationSample],
    sampling_proportions: [f64; 3],
    declared_independent_unit: &str,
    feature_contract: &FrozenFeatureContract,
    transport_config: PrimitiveTransportConfig,
    refit_config: TransportRefitConfig,
) -> Result<RefitCollection, TransportRefitError> {
    let primitive_groups = primitive_cluster_groups(primitive);
    let confirmation_groups = confirmation_cluster_groups(confirmation);
    let mut plan_hasher = Sha256::new();
    plan_hasher.update(b"mic.transport_refit.plan.v1\0");
    hash_u64(&mut plan_hasher, refit_config.seed);
    hash_usize(&mut plan_hasher, refit_config.n_refits)?;
    hash_u64(&mut plan_hasher, refit_config.retain_fraction.to_bits());

    let realized_counts = realized_counts(
        &primitive_groups,
        &confirmation_groups,
        refit_config,
        transport_config.n_folds,
    );
    let mut failures = Vec::new();
    let mut raw_residuals = Vec::new();
    let mut score_gains = Vec::new();
    let mut cluster_ess = Vec::new();
    let mut energies = Vec::new();
    let mut attempted_primitive_plans = std::collections::BTreeSet::new();
    let mut attempted_combination_plans = std::collections::BTreeSet::new();
    let mut attempted_joint_plans = std::collections::BTreeSet::new();
    let mut successful_primitive_plans = std::collections::BTreeSet::new();
    let mut successful_combination_plans = std::collections::BTreeSet::new();
    let mut successful_joint_plans = std::collections::BTreeSet::new();
    let mut attempted_coverage =
        SelectionCoverageAccumulator::new(&primitive_groups, &confirmation_groups);
    let mut primitive_metric_success_coverage =
        SelectionCoverageAccumulator::new(&primitive_groups, &confirmation_groups);
    let mut joint_metric_success_coverage =
        SelectionCoverageAccumulator::new(&primitive_groups, &confirmation_groups);

    for refit in 0..refit_config.n_refits {
        let primitive_ids = select_primitive_clusters(
            &primitive_groups,
            refit_config,
            transport_config.n_folds,
            refit,
        );
        let confirmation_ids =
            select_confirmation_clusters(&confirmation_groups, refit_config, refit);
        let primitive_plan = primitive_selection_fingerprint(&primitive_ids)?;
        let combination_plan = combination_selection_fingerprint(&confirmation_ids)?;
        let joint_plan = selection_fingerprint(&primitive_ids, &confirmation_ids)?;
        attempted_primitive_plans.insert(primitive_plan.clone());
        attempted_combination_plans.insert(combination_plan.clone());
        attempted_joint_plans.insert(joint_plan.clone());
        attempted_coverage.record_primitive(&primitive_ids);
        attempted_coverage.record_combination(&confirmation_ids);
        hash_selection(&mut plan_hasher, refit, &primitive_ids, &confirmation_ids)?;
        let primitive_slice = primitive
            .iter()
            .filter(|row| {
                primitive_ids[&row.arm]
                    .binary_search(&row.cluster_id)
                    .is_ok()
            })
            .cloned()
            .collect::<Vec<_>>();
        let confirmation_slice = confirmation
            .iter()
            .filter(|row| confirmation_ids.binary_search(&row.cluster_id).is_ok())
            .cloned()
            .collect::<Vec<_>>();
        let frozen = match freeze_primitive_transport(
            &primitive_slice,
            sampling_proportions,
            declared_independent_unit,
            feature_contract.clone(),
            transport_config,
        ) {
            Ok(frozen) => frozen,
            Err(error) => {
                failures.push(TransportRefitFailure {
                    refit,
                    stage: TransportRefitFailureStage::PrimitiveFit,
                    reason: error.to_string(),
                });
                continue;
            }
        };
        successful_primitive_plans.insert(primitive_plan);
        primitive_metric_success_coverage.record_primitive(&primitive_ids);
        raw_residuals.push(frozen.receipt().raw_normalizer() - 1.0);
        cluster_ess.push(frozen.receipt().effective_baseline_clusters());
        let report = match score_combination_confirmation(
            &frozen,
            declared_independent_unit,
            feature_contract,
            &confirmation_slice,
        ) {
            Ok(report) => report,
            Err(error) => {
                failures.push(TransportRefitFailure {
                    refit,
                    stage: TransportRefitFailureStage::CombinationScore,
                    reason: error.to_string(),
                });
                continue;
            }
        };
        successful_combination_plans.insert(combination_plan);
        successful_joint_plans.insert(joint_plan);
        joint_metric_success_coverage.record_primitive(&primitive_ids);
        joint_metric_success_coverage.record_combination(&confirmation_ids);
        score_gains.push(report.heldout_proper_score_gain());
        if let Some(energy) = report.predicted_vs_heldout_energy() {
            energies.push(energy);
        }
    }
    Ok(RefitCollection {
        plan_sha256: finish_sha256(plan_hasher),
        failures,
        raw_residuals,
        score_gains,
        cluster_ess,
        energies,
        realized_counts,
        plan_diversity: RefitPlanDiversity {
            attempted_primitive: attempted_primitive_plans.len(),
            attempted_combination: attempted_combination_plans.len(),
            attempted_joint: attempted_joint_plans.len(),
            successful_primitive: successful_primitive_plans.len(),
            successful_combination: successful_combination_plans.len(),
            successful_joint: successful_joint_plans.len(),
        },
        attempted_selection_coverage: attempted_coverage.finish(),
        primitive_metric_success_selection_coverage: primitive_metric_success_coverage
            .finish_primitive(),
        joint_metric_success_selection_coverage: joint_metric_success_coverage.finish(),
    })
}

fn validate_refit_config(config: TransportRefitConfig) -> Result<(), TransportRefitError> {
    if !(MIN_REFITS..=MAX_REFITS).contains(&config.n_refits) {
        return Err(TransportRefitError::InvalidRefitCount);
    }
    if !config.retain_fraction.is_finite()
        || config.retain_fraction <= 0.0
        || config.retain_fraction >= 1.0
    {
        return Err(TransportRefitError::InvalidRetainFraction);
    }
    Ok(())
}

fn validate_work_budget(
    primitive: &[PrimitiveTransportSample],
    confirmation: &[CombinationConfirmationSample],
    transport: PrimitiveTransportConfig,
    refits: TransportRefitConfig,
) -> Result<(), TransportRefitError> {
    let rows = primitive
        .len()
        .checked_add(confirmation.len())
        .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    let features = primitive
        .iter()
        .map(|row| row.features.len())
        .chain(confirmation.iter().map(|row| row.features.len()))
        .max()
        .unwrap_or(1)
        .max(1);
    let repeated_fits = refits
        .n_refits
        .checked_add(2)
        .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    let work = repeated_fits
        .checked_mul(transport.n_folds)
        .and_then(|value| value.checked_mul(transport.fit.max_iterations))
        .and_then(|value| value.checked_mul(rows))
        .and_then(|value| value.checked_mul(features))
        .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    if work > MAX_REFIT_WORK_UNITS {
        return Err(TransportRefitError::WorkBudgetExceeded);
    }
    let identifier_bytes = primitive
        .iter()
        .map(|row| row.cluster_id.len())
        .chain(confirmation.iter().map(|row| row.cluster_id.len()))
        .try_fold(0usize, usize::checked_add)
        .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    let identifier_byte_work = identifier_bytes
        .checked_mul(repeated_fits)
        .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    if identifier_byte_work > MAX_REFIT_IDENTIFIER_BYTE_WORK {
        return Err(TransportRefitError::WorkBudgetExceeded);
    }
    let point_energy_rows = primitive
        .iter()
        .filter(|row| row.arm == PrimitiveArm::Baseline)
        .count()
        .checked_add(confirmation.len())
        .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    let (minimum_retained_energy_rows, maximum_retained_energy_rows) = retained_energy_row_bounds(
        primitive,
        confirmation,
        transport.n_folds,
        refits.retain_fraction,
    )?;
    let point_energy_calls =
        usize::from(!confirmation.is_empty() && point_energy_rows <= MAX_EXACT_DISTANCE_ROWS);
    let refit_energy_calls = usize::from(
        !confirmation.is_empty() && minimum_retained_energy_rows <= MAX_EXACT_DISTANCE_ROWS,
    )
    .checked_mul(refits.n_refits)
    .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    let charged_retained_energy_rows = maximum_retained_energy_rows.min(MAX_EXACT_DISTANCE_ROWS);
    let energy_work = point_energy_calls
        .checked_mul(point_energy_rows)
        .and_then(|value| value.checked_mul(point_energy_rows))
        .and_then(|value| {
            refit_energy_calls
                .checked_mul(charged_retained_energy_rows)
                .and_then(|refit| refit.checked_mul(charged_retained_energy_rows))
                .and_then(|refit| value.checked_add(refit))
        })
        .and_then(|value| value.checked_mul(features))
        .and_then(|value| value.checked_mul(3))
        .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    if energy_work > MAX_REFIT_ENERGY_WORK_UNITS {
        return Err(TransportRefitError::WorkBudgetExceeded);
    }
    Ok(())
}

fn retained_energy_row_bounds(
    primitive: &[PrimitiveTransportSample],
    confirmation: &[CombinationConfirmationSample],
    n_folds: usize,
    retain_fraction: f64,
) -> Result<(usize, usize), TransportRefitError> {
    let mut baseline = BTreeMap::<&str, usize>::new();
    for row in primitive
        .iter()
        .filter(|row| row.arm == PrimitiveArm::Baseline)
    {
        *baseline.entry(&row.cluster_id).or_default() += 1;
    }
    let mut combination = BTreeMap::<&str, usize>::new();
    for row in confirmation {
        *combination.entry(&row.cluster_id).or_default() += 1;
    }
    let retained_baseline = retained_count(baseline.len(), retain_fraction, n_folds);
    let retained_combination = retained_count(combination.len(), retain_fraction, 1);
    let minimum = smallest_cluster_row_sum(&baseline, retained_baseline)?
        .checked_add(smallest_cluster_row_sum(
            &combination,
            retained_combination,
        )?)
        .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    let maximum = largest_cluster_row_sum(&baseline, retained_baseline)?
        .checked_add(largest_cluster_row_sum(&combination, retained_combination)?)
        .ok_or(TransportRefitError::WorkBudgetExceeded)?;
    Ok((minimum, maximum))
}

fn smallest_cluster_row_sum(
    counts: &BTreeMap<&str, usize>,
    retained: usize,
) -> Result<usize, TransportRefitError> {
    let mut values = counts.values().copied().collect::<Vec<_>>();
    values.sort_unstable();
    values
        .into_iter()
        .take(retained)
        .try_fold(0usize, usize::checked_add)
        .ok_or(TransportRefitError::WorkBudgetExceeded)
}

fn largest_cluster_row_sum(
    counts: &BTreeMap<&str, usize>,
    retained: usize,
) -> Result<usize, TransportRefitError> {
    let mut values = counts.values().copied().collect::<Vec<_>>();
    values.sort_unstable_by(|left, right| right.cmp(left));
    values
        .into_iter()
        .take(retained)
        .try_fold(0usize, usize::checked_add)
        .ok_or(TransportRefitError::WorkBudgetExceeded)
}

fn primitive_cluster_groups(
    rows: &[PrimitiveTransportSample],
) -> BTreeMap<PrimitiveArm, Vec<String>> {
    let mut groups = BTreeMap::<PrimitiveArm, Vec<String>>::new();
    for row in rows {
        groups
            .entry(row.arm)
            .or_default()
            .push(row.cluster_id.clone());
    }
    for ids in groups.values_mut() {
        ids.sort();
        ids.dedup();
    }
    groups
}

fn confirmation_cluster_groups(rows: &[CombinationConfirmationSample]) -> Vec<String> {
    let mut ids = rows
        .iter()
        .map(|row| row.cluster_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn select_primitive_clusters(
    groups: &BTreeMap<PrimitiveArm, Vec<String>>,
    config: TransportRefitConfig,
    n_folds: usize,
    refit: usize,
) -> BTreeMap<PrimitiveArm, Vec<String>> {
    groups
        .iter()
        .map(|(arm, ids)| {
            let retained = retained_count(ids.len(), config.retain_fraction, n_folds);
            (
                *arm,
                select_ids(ids, config.seed, refit, arm_tag(*arm), retained),
            )
        })
        .collect()
}

fn select_confirmation_clusters(
    ids: &[String],
    config: TransportRefitConfig,
    refit: usize,
) -> Vec<String> {
    let retained = retained_count(ids.len(), config.retain_fraction, 1);
    select_ids(ids, config.seed, refit, b"combination", retained)
}

fn realized_counts(
    groups: &BTreeMap<PrimitiveArm, Vec<String>>,
    confirmation: &[String],
    config: TransportRefitConfig,
    n_folds: usize,
) -> RealizedRefitCounts {
    let counts = |arm| {
        let total = groups.get(&arm).map_or(0, Vec::len);
        (
            retained_count(total, config.retain_fraction, n_folds),
            total,
        )
    };
    RealizedRefitCounts {
        baseline: counts(PrimitiveArm::Baseline),
        first: counts(PrimitiveArm::First),
        second: counts(PrimitiveArm::Second),
        combination: (
            retained_count(confirmation.len(), config.retain_fraction, 1),
            confirmation.len(),
        ),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn retained_count(total: usize, fraction: f64, minimum: usize) -> usize {
    (((total as f64) * fraction).floor() as usize)
        .max(minimum)
        .min(total)
}

fn select_ids(
    ids: &[String],
    seed: u64,
    refit: usize,
    stratum: &[u8],
    retained: usize,
) -> Vec<String> {
    let mut ranked = ids
        .iter()
        .map(|id| {
            let mut hasher = Sha256::new();
            hasher.update(b"mic.transport_refit.select.v1\0");
            hash_u64(&mut hasher, seed);
            hash_u64(&mut hasher, refit as u64);
            hasher.update(stratum);
            hasher.update(id.as_bytes());
            (hasher.finalize(), id)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(right.1)));
    let mut selected = ranked
        .into_iter()
        .take(retained)
        .map(|(_, id)| id.clone())
        .collect::<Vec<_>>();
    selected.sort();
    selected
}

const fn arm_tag(arm: PrimitiveArm) -> &'static [u8] {
    match arm {
        PrimitiveArm::Baseline => b"baseline",
        PrimitiveArm::First => b"first",
        PrimitiveArm::Second => b"second",
    }
}

fn empirical_quantiles(mut values: Vec<f64>) -> Option<EmpiricalRefitQuantiles> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let last = values.len() - 1;
    Some(EmpiricalRefitQuantiles {
        n_values: values.len(),
        q025: values[(last + 20) / 40],
        q500: values[values.len() / 2],
        q975: values[(39 * last + 20) / 40],
    })
}

fn hash_selection(
    hasher: &mut Sha256,
    refit: usize,
    primitive: &BTreeMap<PrimitiveArm, Vec<String>>,
    confirmation: &[String],
) -> Result<(), TransportRefitError> {
    hash_usize(hasher, refit)?;
    for arm in [
        PrimitiveArm::Baseline,
        PrimitiveArm::First,
        PrimitiveArm::Second,
    ] {
        hasher.update(arm_tag(arm));
        let ids = primitive.get(&arm).map_or(&[][..], Vec::as_slice);
        hash_usize(hasher, ids.len())?;
        for id in ids {
            hash_string(hasher, id)?;
        }
    }
    hasher.update(b"combination");
    hash_usize(hasher, confirmation.len())?;
    for id in confirmation {
        hash_string(hasher, id)?;
    }
    Ok(())
}

fn selection_fingerprint(
    primitive: &BTreeMap<PrimitiveArm, Vec<String>>,
    confirmation: &[String],
) -> Result<String, TransportRefitError> {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.transport_refit.selection.v1\0");
    hash_selection(&mut hasher, 0, primitive, confirmation)?;
    Ok(finish_sha256(hasher))
}

fn primitive_selection_fingerprint(
    primitive: &BTreeMap<PrimitiveArm, Vec<String>>,
) -> Result<String, TransportRefitError> {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.transport_refit.primitive_selection.v1\0");
    hash_selection(&mut hasher, 0, primitive, &[])?;
    Ok(finish_sha256(hasher))
}

fn combination_selection_fingerprint(
    confirmation: &[String],
) -> Result<String, TransportRefitError> {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.transport_refit.combination_selection.v1\0");
    let empty = BTreeMap::new();
    hash_selection(&mut hasher, 0, &empty, confirmation)?;
    Ok(finish_sha256(hasher))
}

fn hash_string(hasher: &mut Sha256, value: &str) -> Result<(), TransportRefitError> {
    hash_usize(hasher, value.len())?;
    hasher.update(value.as_bytes());
    Ok(())
}

fn hash_usize(hasher: &mut Sha256, value: usize) -> Result<(), TransportRefitError> {
    hash_u64(
        hasher,
        u64::try_from(value).map_err(|_| TransportRefitError::FingerprintOverflow)?,
    );
    Ok(())
}

fn hash_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_be_bytes());
}

fn finish_sha256(hasher: Sha256) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in hasher.finalize() {
        write!(&mut encoded, "{byte:02x}").expect("writing into a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FitConfig;

    fn contract() -> FrozenFeatureContract {
        FrozenFeatureContract {
            feature_schema_sha256: "a".repeat(64),
            feature_transform_sha256: "b".repeat(64),
        }
    }

    fn transport_config() -> PrimitiveTransportConfig {
        PrimitiveTransportConfig {
            seed: 41,
            n_folds: 2,
            fit: FitConfig {
                n_classes: 3,
                l2_penalty: 0.1,
                max_iterations: 5_000,
                gradient_tolerance: 1e-6,
                initial_step: 1.0,
            },
        }
    }

    fn fixture() -> (
        Vec<PrimitiveTransportSample>,
        Vec<CombinationConfirmationSample>,
    ) {
        let primitive = [
            PrimitiveArm::Baseline,
            PrimitiveArm::First,
            PrimitiveArm::Second,
        ]
        .into_iter()
        .flat_map(|arm| {
            let arm_offset = match arm {
                PrimitiveArm::Baseline => 0.0,
                PrimitiveArm::First => 0.5,
                PrimitiveArm::Second => 1.0,
            };
            (0..8).map(move |cluster| PrimitiveTransportSample {
                features: vec![f64::from(cluster) / 4.0 + arm_offset],
                arm,
                cluster_id: format!("{arm:?}-{cluster}"),
            })
        })
        .collect();
        let confirmation = (0..8)
            .map(|cluster| CombinationConfirmationSample {
                features: vec![f64::from(cluster) / 4.0 + 1.0],
                cluster_id: format!("combination-{cluster}"),
            })
            .collect();
        (primitive, confirmation)
    }

    fn run(seed: u64) -> TransportRefitReport {
        let (primitive, confirmation) = fixture();
        refit_transport_uncertainty(
            &primitive,
            &confirmation,
            [1.0 / 3.0; 3],
            "experimental_run",
            &contract(),
            transport_config(),
            TransportRefitConfig {
                seed,
                n_refits: 20,
                retain_fraction: 0.75,
            },
        )
        .unwrap()
    }

    #[test]
    fn transport_nuisance_refits_are_seeded_deterministic_and_noncertifying() {
        let first = run(73);
        let second = run(73);
        assert_eq!(first, second);
        assert_eq!(first.n_completed(), 20);
        assert_eq!(first.raw_normalizer_residual().unwrap().n_values, 20);
        assert_eq!(first.proper_score_gain().unwrap().n_values, 20);
        let json = serde_json::to_value(first).unwrap();
        assert_eq!(json["authority"], "diagnostic_only");
        assert_eq!(json["certificate_eligible"], false);
        assert_eq!(json["calibrated_test"], false);
        assert_eq!(json["feature_transform_treatment"], "frozen_not_refit");
        assert_eq!(
            json["metric_quantile_status"]["primitive_metrics"],
            "reported_conditional_on_successful_refits"
        );
        assert_eq!(
            json["metric_quantile_status"]["proper_score"],
            "reported_conditional_on_successful_refits"
        );
        assert_eq!(
            json["attempted_selection_coverage"]["baseline"]["coverage_fraction"],
            1.0
        );
    }

    #[test]
    fn refit_seed_changes_plan_without_changing_point_estimate() {
        let first = run(1);
        let second = run(2);
        assert_ne!(first.resample_plan_sha256(), second.resample_plan_sha256());
        let first_json = serde_json::to_value(first).unwrap();
        let second_json = serde_json::to_value(second).unwrap();
        assert_eq!(first_json["point"], second_json["point"]);

        let (primitive, _) = fixture();
        let groups = primitive_cluster_groups(&primitive);
        let selected = |seed| {
            select_primitive_clusters(
                &groups,
                TransportRefitConfig {
                    seed,
                    n_refits: 20,
                    retain_fraction: 0.75,
                },
                2,
                0,
            )
        };
        assert_ne!(selected(1), selected(2));
    }

    #[test]
    fn no_op_resampling_is_insufficient_and_suppresses_quantiles() {
        let primitive = [
            PrimitiveArm::Baseline,
            PrimitiveArm::First,
            PrimitiveArm::Second,
        ]
        .into_iter()
        .flat_map(|arm| {
            (0..2).map(move |cluster| PrimitiveTransportSample {
                features: vec![0.0],
                arm,
                cluster_id: format!("minimal-{arm:?}-{cluster}"),
            })
        })
        .collect::<Vec<_>>();
        let confirmation = vec![CombinationConfirmationSample {
            features: vec![0.0],
            cluster_id: "only-combination".into(),
        }];
        let report = refit_transport_uncertainty(
            &primitive,
            &confirmation,
            [1.0 / 3.0; 3],
            "experimental_run",
            &contract(),
            transport_config(),
            TransportRefitConfig {
                seed: 1,
                n_refits: 20,
                retain_fraction: 0.5,
            },
        )
        .unwrap();
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["status"], "insufficient");
        assert_eq!(json["plan_diversity"]["successful_joint"], 1);
        assert_eq!(
            json["metric_quantile_status"]["proper_score"],
            "suppressed_degenerate_resampling"
        );
        assert_eq!(json["proper_score_gain"], serde_json::Value::Null);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn invalid_refit_contract_fails_before_point_fitting() {
        let (primitive, confirmation) = fixture();
        let invalid = refit_transport_uncertainty(
            &primitive,
            &confirmation,
            [1.0 / 3.0; 3],
            "experimental_run",
            &contract(),
            transport_config(),
            TransportRefitConfig {
                seed: 1,
                n_refits: 2,
                retain_fraction: 1.0,
            },
        )
        .unwrap_err();
        assert!(matches!(invalid, TransportRefitError::InvalidRefitCount));

        let mut expensive = transport_config();
        expensive.fit.max_iterations = usize::MAX;
        let budget = refit_transport_uncertainty(
            &primitive,
            &confirmation,
            [1.0 / 3.0; 3],
            "experimental_run",
            &contract(),
            expensive,
            TransportRefitConfig {
                seed: 1,
                n_refits: 20,
                retain_fraction: 0.5,
            },
        )
        .unwrap_err();
        assert!(matches!(budget, TransportRefitError::WorkBudgetExceeded));
        assert!(matches!(
            validate_primitive_refit_request(
                &primitive,
                expensive,
                TransportRefitConfig {
                    seed: 1,
                    n_refits: 20,
                    retain_fraction: 0.5,
                },
            ),
            Err(TransportRefitError::WorkBudgetExceeded)
        ));

        let large_primitive = [
            (PrimitiveArm::Baseline, 1_000),
            (PrimitiveArm::First, 1_500),
            (PrimitiveArm::Second, 1_500),
        ]
        .into_iter()
        .flat_map(|(arm, rows)| {
            (0..rows).map(move |row| PrimitiveTransportSample {
                features: vec![0.0],
                arm,
                cluster_id: format!("{arm:?}-{}", row % 2),
            })
        })
        .collect::<Vec<_>>();
        let large_confirmation = (0..1_000)
            .map(|row| CombinationConfirmationSample {
                features: vec![0.0],
                cluster_id: format!("combination-{}", row % 2),
            })
            .collect::<Vec<_>>();
        let mut cheap_fit = transport_config();
        cheap_fit.fit.max_iterations = 1;
        assert!(matches!(
            validate_work_budget(
                &large_primitive,
                &large_confirmation,
                cheap_fit,
                TransportRefitConfig {
                    seed: 1,
                    n_refits: 1_000,
                    retain_fraction: 0.5,
                },
            ),
            Err(TransportRefitError::WorkBudgetExceeded)
        ));

        let identifier_heavy = [
            PrimitiveArm::Baseline,
            PrimitiveArm::First,
            PrimitiveArm::Second,
        ]
        .into_iter()
        .flat_map(|arm| {
            (0..333).map(move |row| PrimitiveTransportSample {
                features: vec![0.0],
                arm,
                cluster_id: format!("{arm:?}-{row}-{:x<1000}", ""),
            })
        })
        .collect::<Vec<_>>();
        assert!(matches!(
            validate_primitive_refit_request(
                &identifier_heavy,
                cheap_fit,
                TransportRefitConfig {
                    seed: 1,
                    n_refits: 1_000,
                    retain_fraction: 0.5,
                },
            ),
            Err(TransportRefitError::WorkBudgetExceeded)
        ));

        let guaranteed_energy_skip = [
            (PrimitiveArm::Baseline, 6_000),
            (PrimitiveArm::First, 2),
            (PrimitiveArm::Second, 2),
        ]
        .into_iter()
        .flat_map(|(arm, clusters)| {
            (0..clusters).map(move |cluster| PrimitiveTransportSample {
                features: vec![0.0],
                arm,
                cluster_id: format!("skip-{arm:?}-{cluster}"),
            })
        })
        .collect::<Vec<_>>();
        assert!(
            validate_work_budget(
                &guaranteed_energy_skip,
                &[CombinationConfirmationSample {
                    features: vec![0.0],
                    cluster_id: "skip-combination".into(),
                }],
                cheap_fit,
                TransportRefitConfig {
                    seed: 1,
                    n_refits: 20,
                    retain_fraction: 0.9,
                },
            )
            .is_ok()
        );
    }

    #[test]
    fn survivor_and_energy_conditioning_are_explicitly_suppressed() {
        assert_eq!(
            refit_quantile_status(2, MIN_COMPLETION_FRACTION - 0.01),
            RefitQuantileStatus::SuppressedCompletionBelowFloor
        );
        assert_eq!(
            refit_quantile_status(1, 1.0),
            RefitQuantileStatus::SuppressedDegenerateResampling
        );
        assert_eq!(
            energy_refit_quantile_status(
                RefitQuantileStatus::ReportedConditionalOnSuccessfulRefits,
                19,
                20,
            ),
            RefitQuantileStatus::SuppressedIncompleteEnergyBudget
        );
    }

    #[test]
    fn confirmation_variation_cannot_mask_degenerate_primitive_refits() {
        let primitive = [
            PrimitiveArm::Baseline,
            PrimitiveArm::First,
            PrimitiveArm::Second,
        ]
        .into_iter()
        .flat_map(|arm| {
            (0..2).map(move |cluster| PrimitiveTransportSample {
                features: vec![f64::from(cluster)],
                arm,
                cluster_id: format!("minimal-{arm:?}-{cluster}"),
            })
        })
        .collect::<Vec<_>>();
        let confirmation = (0..2)
            .map(|cluster| CombinationConfirmationSample {
                features: vec![f64::from(cluster)],
                cluster_id: format!("combination-{cluster}"),
            })
            .collect::<Vec<_>>();
        let report = refit_transport_uncertainty(
            &primitive,
            &confirmation,
            [1.0 / 3.0; 3],
            "experimental_run",
            &contract(),
            transport_config(),
            TransportRefitConfig {
                seed: 1,
                n_refits: 20,
                retain_fraction: 0.5,
            },
        )
        .unwrap();
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["plan_diversity"]["successful_primitive"], 1);
        assert!(json["plan_diversity"]["successful_joint"].as_u64().unwrap() > 1);
        assert_eq!(
            json["metric_quantile_status"]["primitive_metrics"],
            "suppressed_degenerate_resampling"
        );
        assert_eq!(json["raw_normalizer_residual"], serde_json::Value::Null);
        assert_eq!(json["effective_baseline_clusters"], serde_json::Value::Null);
        assert_ne!(json["proper_score_gain"], serde_json::Value::Null);
    }
}
