#![forbid(unsafe_code)]
//! Two-stage fitted prediction of an API-held-out combination law.
//!
//! Stage A accepts only the baseline and two primitive arms. It cross-fits a
//! three-class regime classifier and freezes out-of-fold product weights on
//! baseline clusters. Stage B accepts only combination-arm confirmation rows
//! and compares them with that already-frozen weighted empirical law. The
//! output is descriptive and never carries certificate authority.

use std::{cmp::Ordering, collections::BTreeMap, fmt::Write as _};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{FitConfig, MultinomialFitError, MultinomialLinearModel, MultinomialSample};

const N_PRIMITIVE_ARMS: usize = 3;
const MAX_EXACT_DISTANCE_ROWS: usize = 4_096;
const MAX_CLUSTER_ID_CHARACTERS: usize = 1_024;

/// One of the three laws available to the nuisance-fitting stage.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PrimitiveArm {
    /// Unperturbed reference law `00`.
    Baseline,
    /// First primitive law `10`.
    First,
    /// Second primitive law `01`.
    Second,
}

impl PrimitiveArm {
    const fn index(self) -> usize {
        match self {
            Self::Baseline => 0,
            Self::First => 1,
            Self::Second => 2,
        }
    }
}

/// One primitive-stage row and its declared independent unit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrimitiveTransportSample {
    /// Frozen numeric state representation.
    pub features: Vec<f64>,
    /// Baseline, first primitive, or second primitive arm.
    pub arm: PrimitiveArm,
    /// Declared assignment or independent-unit identifier.
    pub cluster_id: String,
}

/// One separately supplied combination-stage row and its declared unit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CombinationConfirmationSample {
    /// The same frozen numeric state representation used in Stage A.
    pub features: Vec<f64>,
    /// Declared assignment or independent-unit identifier.
    pub cluster_id: String,
}

/// Content binding for the caller-frozen representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrozenFeatureContract {
    /// SHA-256 of the ordered feature schema and semantics.
    pub feature_schema_sha256: String,
    /// SHA-256 of the already-frozen transform, including scaling.
    pub feature_transform_sha256: String,
}

/// Deterministic Stage-A cross-fit configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrimitiveTransportConfig {
    /// Recorded seed for deterministic, arm-stratified cluster folds.
    pub seed: u64,
    /// Number of folds. Must be at least two.
    pub n_folds: usize,
    /// Exact three-class nuisance-fit configuration.
    pub fit: FitConfig,
}

/// Non-certificate authority of every fitted transport artifact.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FittedTransportAuthority {
    /// Descriptive diagnostic only.
    DiagnosticOnly,
}

/// One Stage-A fold summary.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrimitiveTransportFoldSummary {
    /// Held-out fold index.
    pub fold: usize,
    /// Primitive-arm clusters used for nuisance fitting.
    pub n_training_clusters: u32,
    /// Primitive-arm clusters used for out-of-fold evaluation.
    pub n_confirmation_clusters: u32,
    /// Weighted out-of-fold log loss on all three primitive arms.
    pub primitive_oof_log_loss: f64,
    /// Raw product normalizer on this fold's baseline clusters.
    pub raw_normalizer: f64,
}

/// Immutable public facts from Stage A. Its fields cannot be caller-forged.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrimitiveTransportReceipt {
    authority: FittedTransportAuthority,
    certificate_eligible: bool,
    seed: u64,
    n_folds: usize,
    primitive_sampling_proportions: [f64; N_PRIMITIVE_ARMS],
    fit_config: FitConfig,
    declared_independent_unit: String,
    feature_contract: FrozenFeatureContract,
    n_features: usize,
    n_primitive_clusters: u32,
    n_primitive_rows: u32,
    n_baseline_clusters: u32,
    fold_plan_sha256: String,
    primitive_input_sha256: String,
    folds: Vec<PrimitiveTransportFoldSummary>,
    primitive_oof_log_loss: f64,
    raw_normalizer: f64,
    raw_normalizer_residual: f64,
    effective_baseline_clusters: f64,
    cluster_ess_formula: &'static str,
    min_log_product_ratio: f64,
    max_log_product_ratio: f64,
    max_log_normalized_importance_ratio: f64,
    normalizer_facts: NormalizerFacts,
    combination_arm_representable_by_stage_a_api: bool,
    calibrated_test: bool,
}

/// Normalization facts kept together so the raw mass cannot disappear silently.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NormalizerFacts {
    /// Whether any ratio clipping changed the raw weights.
    pub ratio_clipping_applied: bool,
    /// Whether the unnormalized product mass was preserved and reported.
    pub raw_normalizer_checked: bool,
}

impl PrimitiveTransportReceipt {
    /// Recorded deterministic fitting seed.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Raw global mass before product-weight normalization.
    #[must_use]
    pub const fn raw_normalizer(&self) -> f64 {
        self.raw_normalizer
    }

    /// Cluster-level effective sample size of the transported baseline law.
    #[must_use]
    pub const fn effective_baseline_clusters(&self) -> f64 {
        self.effective_baseline_clusters
    }

    /// Content binding for the primitive-only input.
    #[must_use]
    pub fn primitive_input_sha256(&self) -> &str {
        &self.primitive_input_sha256
    }

    /// Content binding for the fold plan.
    #[must_use]
    pub fn fold_plan_sha256(&self) -> &str {
        &self.fold_plan_sha256
    }
}

/// Weighted empirical energy-distance components.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WeightedEnergyDistance {
    /// Twice the cross-law mean Euclidean distance.
    pub twice_cross_distance: f64,
    /// First-law within mean Euclidean distance.
    pub first_within_distance: f64,
    /// Second-law within mean Euclidean distance.
    pub second_within_distance: f64,
    /// Nonnegative V-statistic energy distance.
    pub distance: f64,
}

/// Balanced held-out logarithmic score for the predicted density ratio.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HeldOutDensityRatioScore {
    /// Equal-cluster mean log loss under the frozen predicted ratio.
    pub log_loss: f64,
    /// Log loss of the uninformative ratio-one prediction.
    pub null_log_loss: f64,
    /// Null minus fitted loss; positive means the prediction beats ratio one.
    pub proper_score_gain: f64,
}

/// Final diagnostic comparing the frozen prediction with stage-two `11` rows.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FittedCombinationPredictionReport {
    authority: FittedTransportAuthority,
    certificate_eligible: bool,
    primitive_receipt: PrimitiveTransportReceipt,
    confirmation_input_sha256: String,
    n_combination_clusters: u32,
    n_combination_rows: u32,
    heldout_density_ratio_score: HeldOutDensityRatioScore,
    predicted_vs_heldout_energy: Option<WeightedEnergyDistance>,
    baseline_vs_heldout_energy: Option<WeightedEnergyDistance>,
    exact_energy_computed: bool,
    exact_energy_row_limit: usize,
    distance_metric: &'static str,
    distance_estimator: &'static str,
    combination_use: CombinationUseFacts,
    contracts: TransportContractFacts,
    calibrated_test: bool,
}

/// Exactly how confirmation data entered the fitted diagnostic.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CombinationUseFacts {
    /// Whether the normalized predicted and observed laws were compared.
    pub normalized_law_compared: bool,
    /// Must remain false for a valid two-stage artifact.
    pub confirmation_features_passed_to_nuisance_fit_api: bool,
    /// Must remain false because the metric is hard-coded before Stage B.
    pub confirmation_features_passed_to_metric_selection_api: bool,
}

/// Contracts the model layer deliberately does not self-verify.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TransportContractFacts {
    /// Common support was externally verified.
    pub common_support: ExternalVerification,
    /// The declared cluster identifier was externally verified as the unit.
    pub independent_unit: ExternalVerification,
    /// Selection/inclusion semantics were externally evaluated.
    pub selection_contract: ExternalVerification,
    /// Whether a trusted harness verified that the transform was frozen before
    /// opening the confirmation data. The model API cannot establish history.
    pub upstream_feature_transform_isolation: ExternalVerification,
}

/// External authority state that the model layer cannot promote by itself.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalVerification {
    /// No trusted external receipt was resolved by this diagnostic.
    Unverified,
}

impl FittedCombinationPredictionReport {
    /// Frozen Stage-A receipt used for the comparison.
    #[must_use]
    pub const fn primitive_receipt(&self) -> &PrimitiveTransportReceipt {
        &self.primitive_receipt
    }

    /// Global discrepancy between transported baseline and held-out combination.
    #[must_use]
    pub const fn predicted_vs_heldout_energy(&self) -> Option<f64> {
        match self.predicted_vs_heldout_energy {
            Some(summary) => Some(summary.distance),
            None => None,
        }
    }

    /// Linear-time held-out proper-score gain over the ratio-one null.
    #[must_use]
    pub const fn heldout_proper_score_gain(&self) -> f64 {
        self.heldout_density_ratio_score.proper_score_gain
    }

    /// Confirmation content binding.
    #[must_use]
    pub fn confirmation_input_sha256(&self) -> &str {
        &self.confirmation_input_sha256
    }
}

/// Opaque Stage-A output. It can only be constructed by fitting primitive arms.
#[derive(Debug, Clone)]
pub struct FrozenPrimitiveTransport {
    receipt: PrimitiveTransportReceipt,
    atoms: Vec<TransportAtom>,
    primitive_cluster_ids: Vec<String>,
    fold_models: Vec<MultinomialLinearModel>,
}

impl FrozenPrimitiveTransport {
    /// Immutable, serialization-safe Stage-A facts.
    #[must_use]
    pub const fn receipt(&self) -> &PrimitiveTransportReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone)]
struct TransportAtom {
    features: Vec<f64>,
    cluster_id: String,
    baseline_mass: f64,
    log_product_ratio: f64,
    fold: usize,
}

#[derive(Debug, Clone)]
struct ClusterMeta {
    arm: PrimitiveArm,
    fold: usize,
    row_count: u32,
}

struct PrimitiveFoldFit {
    atoms: Vec<TransportAtom>,
    summaries: Vec<PrimitiveTransportFoldSummary>,
    models: Vec<MultinomialLinearModel>,
    oof_log_loss: f64,
}

/// Fail-closed fitted-transport errors.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum FittedTransportError {
    /// No rows were supplied.
    #[error("fitted transport requires nonempty input rows")]
    EmptyRows,
    /// Fold count is invalid.
    #[error("fold count must be at least two and supported by every primitive arm")]
    InvalidFoldCount,
    /// Nuisance model is not a three-class classifier.
    #[error("primitive transport requires a three-class nuisance model")]
    InvalidClassCount,
    /// Sampling proportions are invalid.
    #[error("primitive sampling proportions must be finite, positive, and sum to one")]
    InvalidSamplingProportions,
    /// A feature schema or transform fingerprint was invalid.
    #[error("feature schema and transform must be lowercase SHA-256 digests")]
    InvalidFeatureContract,
    /// Features are empty, nonfinite, or inconsistent.
    #[error("all rows must have the same nonzero finite feature dimension")]
    InvalidFeatures,
    /// Unit identifier was empty.
    #[error("cluster identifiers must be nonempty")]
    EmptyClusterId,
    /// Unit identifier exceeded the closed request boundary.
    #[error("cluster identifiers must contain at most {MAX_CLUSTER_ID_CHARACTERS} characters")]
    ClusterIdTooLong,
    /// One primitive cluster appeared under multiple arms.
    #[error("cluster {cluster_id} spans primitive arms")]
    ClusterSpansArms {
        /// Conflicting identifier.
        cluster_id: String,
    },
    /// Confirmation reused a primitive-stage unit.
    #[error("confirmation cluster {cluster_id} was already present in Stage A")]
    ConfirmationReusesPrimitiveCluster {
        /// Reused identifier.
        cluster_id: String,
    },
    /// A cluster had too many rows.
    #[error("cluster {cluster_id} exceeds the u32 row-count limit")]
    ClusterTooLarge {
        /// Oversized identifier.
        cluster_id: String,
    },
    /// One primitive arm had too few clusters.
    #[error("primitive arm {arm:?} has {n_clusters} clusters, fewer than {n_folds} folds")]
    InsufficientArmClusters {
        /// Sparse arm.
        arm: PrimitiveArm,
        /// Available clusters.
        n_clusters: usize,
        /// Requested folds.
        n_folds: usize,
    },
    /// Exact quadratic distance exceeded its resource ceiling.
    #[error("exact energy distance has {rows} rows, exceeding the limit {limit}")]
    ExactDistanceBudgetExceeded {
        /// Total rows in the comparison.
        rows: usize,
        /// Fixed ceiling.
        limit: usize,
    },
    /// Product weights or normalizer were nonfinite.
    #[error("product weights produced a nonfinite or nonpositive raw normalizer")]
    InvalidProductWeights,
    /// Frozen-feature distance exceeded finite floating-point range.
    #[error("frozen-feature distance is not representable as a finite f64")]
    NonFiniteDistance,
    /// The held-out logarithmic score exceeded finite floating-point range.
    #[error("held-out density-ratio score is not representable as a finite f64")]
    NonFiniteScore,
    /// Stage A and Stage B declared different feature or unit contracts.
    #[error("confirmation feature or independent-unit contract differs from Stage A")]
    ConfirmationContractMismatch,
    /// Lower-level deterministic fit failed.
    #[error(transparent)]
    Fit(#[from] MultinomialFitError),
}

/// Stage A: fits only `00`, `10`, and `01`, then freezes OOF baseline weights.
pub fn freeze_primitive_transport(
    samples: &[PrimitiveTransportSample],
    sampling_proportions: [f64; N_PRIMITIVE_ARMS],
    declared_independent_unit: &str,
    feature_contract: FrozenFeatureContract,
    config: PrimitiveTransportConfig,
) -> Result<FrozenPrimitiveTransport, FittedTransportError> {
    validate_stage_a(
        samples,
        sampling_proportions,
        declared_independent_unit,
        &feature_contract,
        config,
    )?;
    let mut clusters = primitive_cluster_metadata(samples, config)?;
    assign_folds(&mut clusters, config)?;
    let arm_counts = arm_cluster_counts(&clusters);
    let n_features = samples[0].features.len();
    let fold_plan_sha256 = fold_plan_fingerprint(&clusters, config)?;
    let primitive_input_sha256 = primitive_input_fingerprint(
        samples,
        sampling_proportions,
        declared_independent_unit,
        &feature_contract,
        config,
    )?;

    let fit = fit_primitive_folds(samples, &clusters, arm_counts, sampling_proportions, config)?;
    let atoms = fit.atoms;

    let log_raw_normalizer = log_weight_sum(&atoms)?;
    let raw_normalizer = log_raw_normalizer.exp();
    if !raw_normalizer.is_finite() || raw_normalizer <= 0.0 {
        return Err(FittedTransportError::InvalidProductWeights);
    }
    let effective_baseline_clusters = cluster_effective_sample_size(&atoms, log_raw_normalizer)?;
    let min_log_product_ratio = atoms
        .iter()
        .map(|atom| atom.log_product_ratio)
        .fold(f64::INFINITY, f64::min);
    let max_log_product_ratio = atoms
        .iter()
        .map(|atom| atom.log_product_ratio)
        .fold(f64::NEG_INFINITY, f64::max);
    let n_primitive_clusters =
        u32::try_from(clusters.len()).map_err(|_| FittedTransportError::InvalidFoldCount)?;
    let n_primitive_rows =
        u32::try_from(samples.len()).map_err(|_| FittedTransportError::InvalidFoldCount)?;
    let primitive_cluster_ids = clusters.keys().cloned().collect::<Vec<_>>();
    let receipt = PrimitiveTransportReceipt {
        authority: FittedTransportAuthority::DiagnosticOnly,
        certificate_eligible: false,
        seed: config.seed,
        n_folds: config.n_folds,
        primitive_sampling_proportions: sampling_proportions,
        fit_config: config.fit,
        declared_independent_unit: declared_independent_unit.to_owned(),
        feature_contract,
        n_features,
        n_primitive_clusters,
        n_primitive_rows,
        n_baseline_clusters: arm_counts[PrimitiveArm::Baseline.index()],
        fold_plan_sha256,
        primitive_input_sha256,
        folds: fit.summaries,
        primitive_oof_log_loss: fit.oof_log_loss,
        raw_normalizer,
        raw_normalizer_residual: raw_normalizer - 1.0,
        effective_baseline_clusters,
        cluster_ess_formula: "1 / sum_c(normalized_raw_cluster_mass_c^2)",
        min_log_product_ratio,
        max_log_product_ratio,
        max_log_normalized_importance_ratio: max_log_product_ratio - log_raw_normalizer,
        normalizer_facts: NormalizerFacts {
            ratio_clipping_applied: false,
            raw_normalizer_checked: true,
        },
        combination_arm_representable_by_stage_a_api: false,
        calibrated_test: false,
    };
    Ok(FrozenPrimitiveTransport {
        receipt,
        atoms,
        primitive_cluster_ids,
        fold_models: fit.models,
    })
}

fn fit_primitive_folds(
    samples: &[PrimitiveTransportSample],
    clusters: &BTreeMap<String, ClusterMeta>,
    arm_counts: [u32; N_PRIMITIVE_ARMS],
    sampling: [f64; N_PRIMITIVE_ARMS],
    config: PrimitiveTransportConfig,
) -> Result<PrimitiveFoldFit, FittedTransportError> {
    let mut result = PrimitiveFoldFit {
        atoms: Vec::new(),
        summaries: Vec::with_capacity(config.n_folds),
        models: Vec::with_capacity(config.n_folds),
        oof_log_loss: 0.0,
    };
    for fold in 0..config.n_folds {
        let (training, training_weights, n_training_clusters) =
            primitive_slice(samples, clusters, fold, false, sampling, None);
        let model = MultinomialLinearModel::fit_weighted(&training, &training_weights, config.fit)?;
        let (confirmation, confirmation_weights, n_confirmation_clusters) =
            primitive_slice(samples, clusters, fold, true, sampling, Some(arm_counts));
        let fold_mass = confirmation_weights.iter().sum::<f64>();
        let fold_oof_loss = model.mean_weighted_log_loss(&confirmation, &confirmation_weights)?;
        result.oof_log_loss += fold_mass * fold_oof_loss;
        let before = result.atoms.len();
        append_baseline_atoms(
            &mut result.atoms,
            samples,
            clusters,
            arm_counts[PrimitiveArm::Baseline.index()],
            sampling,
            fold,
            &model,
        )?;
        result.summaries.push(PrimitiveTransportFoldSummary {
            fold,
            n_training_clusters,
            n_confirmation_clusters,
            primitive_oof_log_loss: fold_oof_loss,
            raw_normalizer: fold_raw_normalizer(
                &result.atoms[before..],
                arm_counts[PrimitiveArm::Baseline.index()],
            )?,
        });
        result.models.push(model);
    }
    Ok(result)
}

fn append_baseline_atoms(
    atoms: &mut Vec<TransportAtom>,
    samples: &[PrimitiveTransportSample],
    clusters: &BTreeMap<String, ClusterMeta>,
    n_baseline_clusters: u32,
    sampling: [f64; N_PRIMITIVE_ARMS],
    fold: usize,
    model: &MultinomialLinearModel,
) -> Result<(), FittedTransportError> {
    let mut baseline = samples
        .iter()
        .filter(|sample| {
            sample.arm == PrimitiveArm::Baseline && clusters[&sample.cluster_id].fold == fold
        })
        .collect::<Vec<_>>();
    baseline.sort_by(|left, right| primitive_sample_order(left, right));
    for sample in baseline {
        let meta = &clusters[&sample.cluster_id];
        let ratios = model.predict_log_density_ratios(
            &sample.features,
            &sampling,
            PrimitiveArm::Baseline.index(),
        )?;
        let log_product_ratio =
            ratios[PrimitiveArm::First.index()] + ratios[PrimitiveArm::Second.index()];
        if !log_product_ratio.is_finite() {
            return Err(FittedTransportError::InvalidProductWeights);
        }
        atoms.push(TransportAtom {
            features: sample.features.clone(),
            cluster_id: sample.cluster_id.clone(),
            baseline_mass: 1.0 / f64::from(n_baseline_clusters) / f64::from(meta.row_count),
            log_product_ratio,
            fold,
        });
    }
    Ok(())
}

/// Stage B: accepts `11` rows only after Stage A is frozen and contract-bound.
pub fn score_combination_confirmation(
    frozen: &FrozenPrimitiveTransport,
    declared_independent_unit: &str,
    feature_contract: &FrozenFeatureContract,
    confirmation: &[CombinationConfirmationSample],
) -> Result<FittedCombinationPredictionReport, FittedTransportError> {
    if declared_independent_unit != frozen.receipt.declared_independent_unit
        || feature_contract != &frozen.receipt.feature_contract
    {
        return Err(FittedTransportError::ConfirmationContractMismatch);
    }
    validate_confirmation(frozen, confirmation)?;
    let confirmation_input_sha256 =
        confirmation_fingerprint(declared_independent_unit, feature_contract, confirmation)?;
    let confirmation_counts = confirmation_cluster_counts(confirmation)?;
    let total_rows = frozen.atoms.len().checked_add(confirmation.len()).ok_or(
        FittedTransportError::ExactDistanceBudgetExceeded {
            rows: usize::MAX,
            limit: MAX_EXACT_DISTANCE_ROWS,
        },
    )?;
    let log_z = frozen.receipt.raw_normalizer.ln();
    let predicted = frozen
        .atoms
        .iter()
        .map(|atom| {
            (
                atom.features.as_slice(),
                (atom.baseline_mass.ln() + atom.log_product_ratio - log_z).exp(),
            )
        })
        .collect::<Vec<_>>();
    let baseline = frozen
        .atoms
        .iter()
        .map(|atom| (atom.features.as_slice(), atom.baseline_mass))
        .collect::<Vec<_>>();
    let mut held_out = confirmation
        .iter()
        .map(|row| {
            let count = confirmation_counts[&row.cluster_id];
            (
                row.features.as_slice(),
                1.0 / confirmation_counts.len() as f64 / f64::from(count),
            )
        })
        .collect::<Vec<_>>();
    held_out.sort_by(|left, right| feature_order(left.0, right.0));
    let heldout_density_ratio_score =
        heldout_density_ratio_score(frozen, confirmation, &confirmation_counts, log_z)?;
    let (predicted_vs_heldout_energy, baseline_vs_heldout_energy) =
        if total_rows <= MAX_EXACT_DISTANCE_ROWS {
            (
                Some(weighted_energy_distance(&predicted, &held_out)?),
                Some(weighted_energy_distance(&baseline, &held_out)?),
            )
        } else {
            (None, None)
        };

    let n_combination_clusters = u32::try_from(confirmation_counts.len())
        .map_err(|_| FittedTransportError::InvalidFoldCount)?;
    let n_combination_rows =
        u32::try_from(confirmation.len()).map_err(|_| FittedTransportError::InvalidFoldCount)?;
    Ok(FittedCombinationPredictionReport {
        authority: FittedTransportAuthority::DiagnosticOnly,
        certificate_eligible: false,
        primitive_receipt: frozen.receipt.clone(),
        confirmation_input_sha256,
        n_combination_clusters,
        n_combination_rows,
        heldout_density_ratio_score,
        predicted_vs_heldout_energy,
        baseline_vs_heldout_energy,
        exact_energy_computed: total_rows <= MAX_EXACT_DISTANCE_ROWS,
        exact_energy_row_limit: MAX_EXACT_DISTANCE_ROWS,
        distance_metric: "euclidean_on_frozen_features",
        distance_estimator: "weighted_v_statistic",
        combination_use: CombinationUseFacts {
            normalized_law_compared: true,
            confirmation_features_passed_to_nuisance_fit_api: false,
            confirmation_features_passed_to_metric_selection_api: false,
        },
        contracts: TransportContractFacts {
            common_support: ExternalVerification::Unverified,
            independent_unit: ExternalVerification::Unverified,
            selection_contract: ExternalVerification::Unverified,
            upstream_feature_transform_isolation: ExternalVerification::Unverified,
        },
        calibrated_test: false,
    })
}

fn validate_stage_a(
    samples: &[PrimitiveTransportSample],
    sampling: [f64; N_PRIMITIVE_ARMS],
    declared_independent_unit: &str,
    contract: &FrozenFeatureContract,
    config: PrimitiveTransportConfig,
) -> Result<(), FittedTransportError> {
    if samples.is_empty() {
        return Err(FittedTransportError::EmptyRows);
    }
    if declared_independent_unit.trim().is_empty() {
        return Err(FittedTransportError::ConfirmationContractMismatch);
    }
    if config.n_folds < 2 {
        return Err(FittedTransportError::InvalidFoldCount);
    }
    if config.fit.n_classes != N_PRIMITIVE_ARMS {
        return Err(FittedTransportError::InvalidClassCount);
    }
    if sampling
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        || (sampling.iter().sum::<f64>() - 1.0).abs() > 1e-8
    {
        return Err(FittedTransportError::InvalidSamplingProportions);
    }
    if !is_sha256(&contract.feature_schema_sha256) || !is_sha256(&contract.feature_transform_sha256)
    {
        return Err(FittedTransportError::InvalidFeatureContract);
    }
    let width = samples[0].features.len();
    if width == 0
        || samples.iter().any(|sample| {
            sample.features.len() != width || sample.features.iter().any(|value| !value.is_finite())
        })
    {
        return Err(FittedTransportError::InvalidFeatures);
    }
    Ok(())
}

fn primitive_cluster_metadata(
    samples: &[PrimitiveTransportSample],
    config: PrimitiveTransportConfig,
) -> Result<BTreeMap<String, ClusterMeta>, FittedTransportError> {
    let mut clusters = BTreeMap::<String, ClusterMeta>::new();
    for sample in samples {
        if sample.cluster_id.trim().is_empty() {
            return Err(FittedTransportError::EmptyClusterId);
        }
        if sample.cluster_id.chars().count() > MAX_CLUSTER_ID_CHARACTERS {
            return Err(FittedTransportError::ClusterIdTooLong);
        }
        match clusters.get_mut(&sample.cluster_id) {
            Some(meta) if meta.arm != sample.arm => {
                return Err(FittedTransportError::ClusterSpansArms {
                    cluster_id: sample.cluster_id.clone(),
                });
            }
            Some(meta) => {
                meta.row_count = meta.row_count.checked_add(1).ok_or_else(|| {
                    FittedTransportError::ClusterTooLarge {
                        cluster_id: sample.cluster_id.clone(),
                    }
                })?;
            }
            None => {
                clusters.insert(
                    sample.cluster_id.clone(),
                    ClusterMeta {
                        arm: sample.arm,
                        fold: 0,
                        row_count: 1,
                    },
                );
            }
        }
    }
    let counts = arm_cluster_counts(&clusters);
    for arm in [
        PrimitiveArm::Baseline,
        PrimitiveArm::First,
        PrimitiveArm::Second,
    ] {
        let n_clusters = counts[arm.index()] as usize;
        if n_clusters < config.n_folds {
            return Err(FittedTransportError::InsufficientArmClusters {
                arm,
                n_clusters,
                n_folds: config.n_folds,
            });
        }
    }
    Ok(clusters)
}

fn assign_folds(
    clusters: &mut BTreeMap<String, ClusterMeta>,
    config: PrimitiveTransportConfig,
) -> Result<(), FittedTransportError> {
    let mut by_arm: [Vec<String>; N_PRIMITIVE_ARMS] = std::array::from_fn(|_| Vec::new());
    for (cluster_id, meta) in clusters.iter() {
        by_arm[meta.arm.index()].push(cluster_id.clone());
    }
    for (arm, ids) in by_arm.iter_mut().enumerate() {
        ids.sort_by(|left, right| {
            fold_hash(config.seed, arm, left)
                .cmp(&fold_hash(config.seed, arm, right))
                .then_with(|| left.cmp(right))
        });
        for (index, id) in ids.iter().enumerate() {
            clusters
                .get_mut(id)
                .ok_or(FittedTransportError::InvalidFoldCount)?
                .fold = index % config.n_folds;
        }
    }
    Ok(())
}

fn fold_hash(seed: u64, arm: usize, cluster_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.primitive_transport.fold.v1\0");
    hasher.update(seed.to_be_bytes());
    hasher.update(u64::try_from(arm).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(
        u64::try_from(cluster_id.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(cluster_id.as_bytes());
    hasher.finalize().into()
}

fn arm_cluster_counts(clusters: &BTreeMap<String, ClusterMeta>) -> [u32; N_PRIMITIVE_ARMS] {
    let mut counts = [0_u32; N_PRIMITIVE_ARMS];
    for meta in clusters.values() {
        counts[meta.arm.index()] += 1;
    }
    counts
}

fn primitive_slice(
    samples: &[PrimitiveTransportSample],
    clusters: &BTreeMap<String, ClusterMeta>,
    fold: usize,
    held_out: bool,
    sampling: [f64; N_PRIMITIVE_ARMS],
    denominator_counts: Option<[u32; N_PRIMITIVE_ARMS]>,
) -> (Vec<MultinomialSample>, Vec<f64>, u32) {
    let mut selected_counts = [0_u32; N_PRIMITIVE_ARMS];
    for meta in clusters
        .values()
        .filter(|meta| (meta.fold == fold) == held_out)
    {
        selected_counts[meta.arm.index()] += 1;
    }
    let denominators = denominator_counts.unwrap_or(selected_counts);
    let n_clusters = selected_counts.iter().sum();
    let mut selected = samples
        .iter()
        .filter(|sample| (clusters[&sample.cluster_id].fold == fold) == held_out)
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| primitive_sample_order(left, right));
    let mut rows = Vec::with_capacity(selected.len());
    let mut weights = Vec::with_capacity(selected.len());
    for sample in selected {
        let meta = &clusters[&sample.cluster_id];
        rows.push(MultinomialSample {
            features: sample.features.clone(),
            class: sample.arm.index(),
        });
        weights.push(
            sampling[meta.arm.index()]
                / f64::from(denominators[meta.arm.index()])
                / f64::from(meta.row_count),
        );
    }
    (rows, weights, n_clusters)
}

fn primitive_sample_order(
    left: &PrimitiveTransportSample,
    right: &PrimitiveTransportSample,
) -> Ordering {
    left.cluster_id
        .cmp(&right.cluster_id)
        .then_with(|| left.arm.cmp(&right.arm))
        .then_with(|| feature_order(&left.features, &right.features))
}

fn feature_order(left: &[f64], right: &[f64]) -> Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| left.total_cmp(right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn log_weight_sum(atoms: &[TransportAtom]) -> Result<f64, FittedTransportError> {
    let max = atoms
        .iter()
        .map(|atom| atom.baseline_mass.ln() + atom.log_product_ratio)
        .fold(f64::NEG_INFINITY, f64::max);
    let scaled = atoms
        .iter()
        .map(|atom| (atom.baseline_mass.ln() + atom.log_product_ratio - max).exp())
        .sum::<f64>();
    let result = max + scaled.ln();
    if result.is_finite() {
        Ok(result)
    } else {
        Err(FittedTransportError::InvalidProductWeights)
    }
}

fn fold_raw_normalizer(
    atoms: &[TransportAtom],
    total_baseline_clusters: u32,
) -> Result<f64, FittedTransportError> {
    let fold_clusters = atoms
        .iter()
        .map(|atom| atom.cluster_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let share = fold_clusters as f64 / f64::from(total_baseline_clusters);
    let value = (log_weight_sum(atoms)? - share.ln()).exp();
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(FittedTransportError::InvalidProductWeights)
    }
}

fn cluster_effective_sample_size(
    atoms: &[TransportAtom],
    log_z: f64,
) -> Result<f64, FittedTransportError> {
    let mut mass = BTreeMap::<&str, f64>::new();
    for atom in atoms {
        *mass.entry(&atom.cluster_id).or_default() +=
            (atom.baseline_mass.ln() + atom.log_product_ratio - log_z).exp();
    }
    let sum_squares = mass.values().map(|value| value * value).sum::<f64>();
    let ess = 1.0 / sum_squares;
    if ess.is_finite() && ess > 0.0 {
        Ok(ess)
    } else {
        Err(FittedTransportError::InvalidProductWeights)
    }
}

fn validate_confirmation(
    frozen: &FrozenPrimitiveTransport,
    rows: &[CombinationConfirmationSample],
) -> Result<(), FittedTransportError> {
    if rows.is_empty() {
        return Err(FittedTransportError::EmptyRows);
    }
    if rows.iter().any(|row| {
        row.features.len() != frozen.receipt.n_features
            || row.features.iter().any(|value| !value.is_finite())
    }) {
        return Err(FittedTransportError::InvalidFeatures);
    }
    for row in rows {
        if row.cluster_id.trim().is_empty() {
            return Err(FittedTransportError::EmptyClusterId);
        }
        if row.cluster_id.chars().count() > MAX_CLUSTER_ID_CHARACTERS {
            return Err(FittedTransportError::ClusterIdTooLong);
        }
        if frozen
            .primitive_cluster_ids
            .binary_search(&row.cluster_id)
            .is_ok()
        {
            return Err(FittedTransportError::ConfirmationReusesPrimitiveCluster {
                cluster_id: row.cluster_id.clone(),
            });
        }
    }
    Ok(())
}

fn confirmation_cluster_counts(
    rows: &[CombinationConfirmationSample],
) -> Result<BTreeMap<String, u32>, FittedTransportError> {
    let mut counts = BTreeMap::<String, u32>::new();
    for row in rows {
        let count = counts.entry(row.cluster_id.clone()).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| FittedTransportError::ClusterTooLarge {
                cluster_id: row.cluster_id.clone(),
            })?;
    }
    Ok(counts)
}

fn heldout_density_ratio_score(
    frozen: &FrozenPrimitiveTransport,
    confirmation: &[CombinationConfirmationSample],
    confirmation_counts: &BTreeMap<String, u32>,
    log_z: f64,
) -> Result<HeldOutDensityRatioScore, FittedTransportError> {
    let baseline_loss = frozen
        .atoms
        .iter()
        .map(|atom| atom.baseline_mass * softplus(atom.log_product_ratio - log_z))
        .sum::<f64>();
    let baseline_fold_mass = baseline_fold_mass(frozen)?;
    let mut combination_loss = 0.0;
    for row in confirmation {
        let mut row_loss = 0.0;
        for (model, fold_mass) in frozen.fold_models.iter().zip(&baseline_fold_mass) {
            let ratios = model.predict_log_density_ratios(
                &row.features,
                &frozen.receipt.primitive_sampling_proportions,
                PrimitiveArm::Baseline.index(),
            )?;
            let log_ratio =
                ratios[PrimitiveArm::First.index()] + ratios[PrimitiveArm::Second.index()] - log_z;
            let contribution = fold_mass * softplus(-log_ratio);
            if !contribution.is_finite() {
                return Err(FittedTransportError::NonFiniteScore);
            }
            row_loss += contribution;
        }
        let mass = 1.0
            / confirmation_counts.len() as f64
            / f64::from(confirmation_counts[&row.cluster_id]);
        combination_loss += mass * row_loss;
    }
    let log_loss = f64::midpoint(baseline_loss, combination_loss);
    let null_log_loss = 2.0_f64.ln();
    let proper_score_gain = null_log_loss - log_loss;
    if [baseline_loss, combination_loss, log_loss, proper_score_gain]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(FittedTransportError::NonFiniteScore);
    }
    Ok(HeldOutDensityRatioScore {
        log_loss,
        null_log_loss,
        proper_score_gain,
    })
}

fn baseline_fold_mass(frozen: &FrozenPrimitiveTransport) -> Result<Vec<f64>, FittedTransportError> {
    let mut mass = vec![0.0; frozen.receipt.n_folds];
    for atom in &frozen.atoms {
        mass[atom.fold] += atom.baseline_mass;
    }
    let total = mass.iter().sum::<f64>();
    if !total.is_finite() || total <= 0.0 {
        return Err(FittedTransportError::InvalidProductWeights);
    }
    for value in &mut mass {
        *value /= total;
    }
    Ok(mass)
}

fn softplus(value: f64) -> f64 {
    if value > 0.0 {
        value + (-value).exp().ln_1p()
    } else {
        value.exp().ln_1p()
    }
}

fn weighted_energy_distance(
    first: &[(&[f64], f64)],
    second: &[(&[f64], f64)],
) -> Result<WeightedEnergyDistance, FittedTransportError> {
    let mut cross = 0.0;
    for (left, left_mass) in first {
        for (right, right_mass) in second {
            cross += left_mass * right_mass * euclidean(left, right)?;
        }
    }
    let first_within = within_distance(first)?;
    let second_within = within_distance(second)?;
    let twice_cross_distance = 2.0 * cross;
    let distance = (twice_cross_distance - first_within - second_within).max(0.0);
    if [twice_cross_distance, first_within, second_within, distance]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(FittedTransportError::NonFiniteDistance);
    }
    Ok(WeightedEnergyDistance {
        twice_cross_distance,
        first_within_distance: first_within,
        second_within_distance: second_within,
        distance,
    })
}

fn within_distance(points: &[(&[f64], f64)]) -> Result<f64, FittedTransportError> {
    let mut total = 0.0;
    for (left, left_mass) in points {
        for (right, right_mass) in points {
            total += left_mass * right_mass * euclidean(left, right)?;
        }
    }
    if total.is_finite() {
        Ok(total)
    } else {
        Err(FittedTransportError::NonFiniteDistance)
    }
}

fn euclidean(left: &[f64], right: &[f64]) -> Result<f64, FittedTransportError> {
    let mut norm = 0.0_f64;
    for (left, right) in left.iter().zip(right) {
        let difference = left - right;
        if !difference.is_finite() {
            return Err(FittedTransportError::NonFiniteDistance);
        }
        norm = norm.hypot(difference);
    }
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(FittedTransportError::NonFiniteDistance)
    }
}

fn fold_plan_fingerprint(
    clusters: &BTreeMap<String, ClusterMeta>,
    config: PrimitiveTransportConfig,
) -> Result<String, FittedTransportError> {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.primitive_transport.fold_plan.v1\0");
    hash_u64(&mut hasher, config.seed);
    hash_usize(&mut hasher, config.n_folds)?;
    for (id, meta) in clusters {
        hash_string(&mut hasher, id)?;
        hash_usize(&mut hasher, meta.arm.index())?;
        hash_usize(&mut hasher, meta.fold)?;
    }
    Ok(finish_sha256(hasher))
}

fn primitive_input_fingerprint(
    samples: &[PrimitiveTransportSample],
    sampling: [f64; N_PRIMITIVE_ARMS],
    declared_independent_unit: &str,
    contract: &FrozenFeatureContract,
    config: PrimitiveTransportConfig,
) -> Result<String, FittedTransportError> {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.primitive_transport.input.v1\0");
    hash_u64(&mut hasher, config.seed);
    hash_usize(&mut hasher, config.n_folds)?;
    hash_fit_config(&mut hasher, config.fit)?;
    hash_string(&mut hasher, declared_independent_unit)?;
    for value in sampling {
        hash_u64(&mut hasher, value.to_bits());
    }
    hash_string(&mut hasher, &contract.feature_schema_sha256)?;
    hash_string(&mut hasher, &contract.feature_transform_sha256)?;
    let mut ordered = samples.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| primitive_sample_order(left, right));
    hash_usize(&mut hasher, ordered.len())?;
    for sample in ordered {
        hash_string(&mut hasher, &sample.cluster_id)?;
        hash_usize(&mut hasher, sample.arm.index())?;
        hash_features(&mut hasher, &sample.features)?;
    }
    Ok(finish_sha256(hasher))
}

fn confirmation_fingerprint(
    declared_independent_unit: &str,
    contract: &FrozenFeatureContract,
    rows: &[CombinationConfirmationSample],
) -> Result<String, FittedTransportError> {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.combination_confirmation.input.v1\0");
    hash_string(&mut hasher, declared_independent_unit)?;
    hash_string(&mut hasher, &contract.feature_schema_sha256)?;
    hash_string(&mut hasher, &contract.feature_transform_sha256)?;
    let mut ordered = rows.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.cluster_id
            .cmp(&right.cluster_id)
            .then_with(|| feature_order(&left.features, &right.features))
    });
    hash_usize(&mut hasher, ordered.len())?;
    for row in ordered {
        hash_string(&mut hasher, &row.cluster_id)?;
        hash_features(&mut hasher, &row.features)?;
    }
    Ok(finish_sha256(hasher))
}

fn hash_fit_config(hasher: &mut Sha256, config: FitConfig) -> Result<(), FittedTransportError> {
    hash_usize(hasher, config.n_classes)?;
    hash_u64(hasher, config.l2_penalty.to_bits());
    hash_usize(hasher, config.max_iterations)?;
    hash_u64(hasher, config.gradient_tolerance.to_bits());
    hash_u64(hasher, config.initial_step.to_bits());
    Ok(())
}

fn hash_features(hasher: &mut Sha256, features: &[f64]) -> Result<(), FittedTransportError> {
    hash_usize(hasher, features.len())?;
    for value in features {
        hash_u64(hasher, value.to_bits());
    }
    Ok(())
}

fn hash_string(hasher: &mut Sha256, value: &str) -> Result<(), FittedTransportError> {
    hash_usize(hasher, value.len())?;
    hasher.update(value.as_bytes());
    Ok(())
}

fn hash_usize(hasher: &mut Sha256, value: usize) -> Result<(), FittedTransportError> {
    let value = u64::try_from(value).map_err(|_| FittedTransportError::InvalidFoldCount)?;
    hash_u64(hasher, value);
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

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> FrozenFeatureContract {
        FrozenFeatureContract {
            feature_schema_sha256: "a".repeat(64),
            feature_transform_sha256: "b".repeat(64),
        }
    }

    fn config() -> PrimitiveTransportConfig {
        PrimitiveTransportConfig {
            seed: 41,
            n_folds: 2,
            fit: FitConfig {
                n_classes: 3,
                l2_penalty: 0.1,
                gradient_tolerance: 1e-7,
                ..FitConfig::default()
            },
        }
    }

    fn identical_primitives() -> Vec<PrimitiveTransportSample> {
        [
            PrimitiveArm::Baseline,
            PrimitiveArm::First,
            PrimitiveArm::Second,
        ]
        .into_iter()
        .flat_map(|arm| {
            (0..2).map(move |cluster| PrimitiveTransportSample {
                features: vec![0.0],
                arm,
                cluster_id: format!("{arm:?}-{cluster}"),
            })
        })
        .collect()
    }

    fn informative_primitives() -> Vec<PrimitiveTransportSample> {
        let locations = [
            (PrimitiveArm::Baseline, [-2.0, -1.0, 0.0, 1.0]),
            (PrimitiveArm::First, [-1.0, 0.0, 1.0, 2.0]),
            (PrimitiveArm::Second, [0.0, 1.0, 2.0, 3.0]),
        ];
        locations
            .into_iter()
            .flat_map(|(arm, values)| {
                values.into_iter().enumerate().map(move |(cluster, value)| {
                    PrimitiveTransportSample {
                        features: vec![value],
                        arm,
                        cluster_id: format!("informative-{arm:?}-{cluster}"),
                    }
                })
            })
            .collect()
    }

    fn freeze(
        samples: &[PrimitiveTransportSample],
        sampling: [f64; 3],
        feature_contract: FrozenFeatureContract,
        fit_config: PrimitiveTransportConfig,
    ) -> Result<FrozenPrimitiveTransport, FittedTransportError> {
        freeze_primitive_transport(
            samples,
            sampling,
            "experimental_run",
            feature_contract,
            fit_config,
        )
    }

    fn score(
        frozen: &FrozenPrimitiveTransport,
        samples: &[CombinationConfirmationSample],
    ) -> Result<FittedCombinationPredictionReport, FittedTransportError> {
        score_combination_confirmation(frozen, "experimental_run", &contract(), samples)
    }

    #[test]
    fn two_stage_flat_fixture_predicts_untouched_combination() {
        let frozen = freeze(
            &identical_primitives(),
            [1.0 / 3.0; 3],
            contract(),
            config(),
        )
        .unwrap();
        assert!((frozen.receipt().raw_normalizer() - 1.0).abs() < 1e-8);
        assert!((frozen.receipt().effective_baseline_clusters() - 2.0).abs() < 1e-8);
        let confirmation = (0..2)
            .map(|cluster| CombinationConfirmationSample {
                features: vec![0.0],
                cluster_id: format!("combination-{cluster}"),
            })
            .collect::<Vec<_>>();
        let report = score(&frozen, &confirmation).unwrap();
        assert!(report.predicted_vs_heldout_energy().unwrap() < 1e-12);
        assert!(report.heldout_proper_score_gain().abs() < 1e-8);
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["authority"], "diagnostic_only");
        assert_eq!(json["certificate_eligible"], false);
        assert_eq!(json["calibrated_test"], false);
        assert_eq!(
            json["combination_use"]["confirmation_features_passed_to_nuisance_fit_api"],
            false
        );
    }

    #[test]
    fn confirmation_changes_cannot_change_the_frozen_fit() {
        let frozen = freeze(
            &identical_primitives(),
            [0.2, 0.3, 0.5],
            contract(),
            config(),
        )
        .unwrap();
        let receipt_before = serde_json::to_vec(frozen.receipt()).unwrap();
        let near = vec![CombinationConfirmationSample {
            features: vec![0.0],
            cluster_id: "near".into(),
        }];
        let far = vec![CombinationConfirmationSample {
            features: vec![1000.0],
            cluster_id: "far".into(),
        }];
        let near_report = score(&frozen, &near).unwrap();
        let far_report = score(&frozen, &far).unwrap();
        assert_eq!(
            receipt_before,
            serde_json::to_vec(frozen.receipt()).unwrap()
        );
        assert_ne!(
            near_report.confirmation_input_sha256(),
            far_report.confirmation_input_sha256()
        );
        assert!(far_report.predicted_vs_heldout_energy().unwrap() > 100.0);
        assert_eq!(
            near_report.primitive_receipt(),
            far_report.primitive_receipt()
        );
    }

    #[test]
    fn row_duplication_preserves_the_cluster_estimand() {
        let samples = identical_primitives();
        let duplicated = samples
            .iter()
            .flat_map(|sample| std::iter::repeat_n(sample.clone(), 5))
            .collect::<Vec<_>>();
        let original = freeze(&samples, [0.2, 0.3, 0.5], contract(), config()).unwrap();
        let repeated = freeze(&duplicated, [0.2, 0.3, 0.5], contract(), config()).unwrap();
        assert!(
            (original.receipt().raw_normalizer() - repeated.receipt().raw_normalizer()).abs()
                < 1e-8
        );
        assert!(
            (original.receipt().effective_baseline_clusters()
                - repeated.receipt().effective_baseline_clusters())
            .abs()
                < 1e-8
        );
        assert_eq!(
            original.receipt().fold_plan_sha256(),
            repeated.receipt().fold_plan_sha256()
        );
    }

    #[test]
    fn primitive_row_order_is_canonical_and_cross_arm_clusters_fail() {
        let samples = identical_primitives();
        let mut reversed = samples.clone();
        reversed.reverse();
        let original = freeze(&samples, [1.0 / 3.0; 3], contract(), config()).unwrap();
        let reordered = freeze(&reversed, [1.0 / 3.0; 3], contract(), config()).unwrap();
        assert_eq!(original.receipt(), reordered.receipt());

        let mut spanning = samples;
        spanning.push(PrimitiveTransportSample {
            features: vec![0.0],
            arm: PrimitiveArm::Second,
            cluster_id: "Baseline-0".into(),
        });
        assert!(matches!(
            freeze(&spanning, [1.0 / 3.0; 3], contract(), config()).unwrap_err(),
            FittedTransportError::ClusterSpansArms { .. }
        ));
    }

    #[test]
    fn large_confirmation_keeps_linear_score_and_skips_quadratic_energy() {
        let frozen = freeze(
            &identical_primitives(),
            [1.0 / 3.0; 3],
            contract(),
            config(),
        )
        .unwrap();
        let confirmation = (0..MAX_EXACT_DISTANCE_ROWS)
            .map(|row| CombinationConfirmationSample {
                features: vec![0.0],
                cluster_id: format!("combination-{row}"),
            })
            .collect::<Vec<_>>();
        let report = score(&frozen, &confirmation).unwrap();
        assert_eq!(report.predicted_vs_heldout_energy(), None);
        assert!(report.heldout_proper_score_gain().abs() < 1e-8);
    }

    #[test]
    fn confirmation_cluster_ids_cannot_route_rows_to_favorable_fold_models() {
        let frozen = freeze(
            &informative_primitives(),
            [1.0 / 3.0; 3],
            contract(),
            config(),
        )
        .unwrap();
        let left = vec![CombinationConfirmationSample {
            features: vec![0.5],
            cluster_id: "confirmation-left".into(),
        }];
        let right = vec![CombinationConfirmationSample {
            features: vec![0.5],
            cluster_id: "confirmation-right".into(),
        }];
        let left_score = score(&frozen, &left).unwrap().heldout_proper_score_gain();
        let right_score = score(&frozen, &right).unwrap().heldout_proper_score_gain();
        assert_eq!(left_score.to_bits(), right_score.to_bits());
    }

    #[test]
    fn large_extreme_confirmation_fails_instead_of_serializing_null_scores() {
        let frozen = freeze(
            &informative_primitives(),
            [1.0 / 3.0; 3],
            contract(),
            config(),
        )
        .unwrap();
        let confirmation = (0..=MAX_EXACT_DISTANCE_ROWS)
            .map(|row| CombinationConfirmationSample {
                features: vec![-1e308],
                cluster_id: format!("extreme-{row}"),
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            score(&frozen, &confirmation).unwrap_err(),
            FittedTransportError::NonFiniteScore
                | FittedTransportError::Fit(MultinomialFitError::NonFinitePrediction)
        ));
    }

    #[test]
    fn stage_boundary_rejects_reused_units_and_wrong_class_count() {
        let mut wrong = config();
        wrong.fit.n_classes = 4;
        assert_eq!(
            freeze(&identical_primitives(), [0.2, 0.3, 0.5], contract(), wrong).unwrap_err(),
            FittedTransportError::InvalidClassCount
        );
        let frozen = freeze(
            &identical_primitives(),
            [0.2, 0.3, 0.5],
            contract(),
            config(),
        )
        .unwrap();
        let reused = vec![CombinationConfirmationSample {
            features: vec![0.0],
            cluster_id: "Baseline-0".into(),
        }];
        assert!(matches!(
            score(&frozen, &reused).unwrap_err(),
            FittedTransportError::ConfirmationReusesPrimitiveCluster { .. }
        ));

        let mut mismatched_contract = contract();
        mismatched_contract.feature_transform_sha256 = "c".repeat(64);
        assert_eq!(
            score_combination_confirmation(
                &frozen,
                "experimental_run",
                &mismatched_contract,
                &[CombinationConfirmationSample {
                    features: vec![0.0],
                    cluster_id: "new-unit".into(),
                }],
            )
            .unwrap_err(),
            FittedTransportError::ConfirmationContractMismatch
        );

        let mut oversized_primitive = identical_primitives();
        oversized_primitive[0].cluster_id = "x".repeat(MAX_CLUSTER_ID_CHARACTERS + 1);
        assert_eq!(
            freeze(&oversized_primitive, [0.2, 0.3, 0.5], contract(), config(),).unwrap_err(),
            FittedTransportError::ClusterIdTooLong
        );
        assert_eq!(
            score_combination_confirmation(
                &frozen,
                "experimental_run",
                &contract(),
                &[CombinationConfirmationSample {
                    features: vec![0.0],
                    cluster_id: "x".repeat(MAX_CLUSTER_ID_CHARACTERS + 1),
                }],
            )
            .unwrap_err(),
            FittedTransportError::ClusterIdTooLong
        );
    }

    #[test]
    fn weighted_energy_keeps_raw_normalizer_as_a_separate_obstruction() {
        let predicted = [
            (&[0.0][..], 0.012_195_121_951_219_5),
            (&[1.0][..], 0.987_804_878_048_780_5),
        ];
        let confirmation = predicted;
        let energy = weighted_energy_distance(&predicted, &confirmation).unwrap();
        assert!(energy.distance < 1e-12);
        let raw_normalizer = 1.64;
        assert!((raw_normalizer - 1.0_f64).abs() > 0.5);
    }

    #[test]
    fn exact_energy_rejects_unrepresentable_finite_feature_distance() {
        let first = [(&[1e308][..], 1.0)];
        let second = [(&[-1e308][..], 1.0)];
        assert_eq!(
            weighted_energy_distance(&first, &second).unwrap_err(),
            FittedTransportError::NonFiniteDistance
        );
    }
}
