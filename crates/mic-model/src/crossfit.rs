#![forbid(unsafe_code)]
//! Cluster-honest cross-fitting for the four-corner closure diagnostic.
//!
//! Fold assignment is deterministic from a recorded seed. A dependence unit is
//! never split, while assignment episodes nested within it may carry different
//! corners. Each dependence unit receives equal total weight within a corner,
//! then its weight is divided equally across episodes and rows. Declared corner
//! pooling mass matches the classifier offsets, and every training and
//! confirmation fold contains all four corners. The resulting proper-loss
//! advantage remains diagnostic: this module does not calibrate a hypothesis
//! test or issue causal authority.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ClosureFitConfig, ClosureFitError, ClosureModelKind, FourCornerClosureModel, MultinomialSample,
    compare_held_out_closure_models_weighted,
};

const N_CLASSES: usize = 4;
const OVERLAP_POSTERIOR_FLOOR: f64 = 1e-6;

/// One row with its externally declared independent unit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClusteredMultinomialSample {
    /// State features used by the reference regime model.
    pub features: Vec<f64>,
    /// Four-corner class in `00,10,01,11` order.
    pub class: usize,
    /// Highest declared dependence-unit identifier; it is never split across folds.
    pub dependence_unit_id: String,
    /// Assignment episode nested in the dependence unit and carrying one regime.
    pub assignment_episode_id: String,
}

/// Deterministic cross-fit plan and nuisance-fit configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClosureCrossFitConfig {
    /// Seed used by the deterministic dependence-unit fold hash.
    pub seed: u64,
    /// Number of outer folds. Must be at least two.
    pub n_folds: usize,
    /// Configuration shared by every restricted and saturated nuisance fit.
    pub fit: ClosureFitConfig,
}

/// Frozen empirical weighting used by the reference diagnostic.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClosureObservationWeighting {
    /// Equal corner mass is divided by dependence unit, episode, then row.
    EqualDependenceUnitWithinCornerThenEpisodeThenRow,
}

/// Authority of the supplied unit roles.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClosureUnitAuthority {
    /// The API enforced nesting and fold isolation but did not verify physical provenance.
    DeclaredUnverified,
}

/// Held-out diagnostic for one outer fold.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FoldClosureDiagnostic {
    /// Held-out fold index.
    fold: usize,
    /// Dependence units used to fit the nuisance models.
    n_training_dependence_units: u32,
    /// Untouched dependence units used for this comparison.
    n_confirmation_dependence_units: u32,
    /// Realized total training weight by corner; equals the declared pooling law.
    training_class_mass: [f64; N_CLASSES],
    /// Share of the global confirmation target mass present in this fold.
    confirmation_class_mass: [f64; N_CLASSES],
    /// Cluster-weighted held-out loss under modular closure.
    restricted_log_loss: f64,
    /// Cluster-weighted held-out loss with an interaction field.
    saturated_log_loss: f64,
    /// Restricted minus saturated loss; positive favors the interaction model.
    saturated_advantage: f64,
    /// Out-of-fold fitted interaction moments under the held-out baseline law.
    fitted_linear_interaction: FittedLinearInteractionSummary,
    /// Descriptive posterior-odds overlap diagnostics on untouched units.
    overlap: FittedOverlapSummary,
}

/// Out-of-fold moments of the regularized linear interaction projection.
///
/// These values are model dependent. They are population density-curvature
/// moments only if the frozen hierarchical regime model is correctly specified.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FittedLinearInteractionSummary {
    /// Equal-cluster baseline mean of the signed fitted interaction field.
    baseline_mean: f64,
    /// Equal-cluster baseline mean absolute fitted interaction.
    baseline_mean_abs: f64,
    /// Equal-cluster baseline root-mean-square fitted interaction.
    baseline_rms: f64,
    /// Maximum absolute fitted interaction on any held-out baseline row.
    baseline_max_abs: f64,
    /// Number of held-out baseline clusters summarized.
    n_baseline_clusters: u32,
    /// Always false: these moments are not a calibrated test.
    calibrated_test: bool,
}

impl FittedLinearInteractionSummary {
    /// Whether these fitted field moments form a calibrated test. Always false.
    #[must_use]
    pub fn calibrated_test(&self) -> bool {
        self.calibrated_test
    }
}

/// Model-dependent overlap diagnostics on one untouched fold.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FittedOverlapSummary {
    /// Density-ratio Kish ESS across held-out baseline clusters for `10`, `01`, and `11`.
    ratio_cluster_ess: [f64; 3],
    /// Largest finite absolute log density ratio seen on a held-out baseline row.
    max_abs_log_density_ratio: Option<f64>,
    /// Smallest regime posterior on any untouched row.
    min_regime_posterior: f64,
    /// Largest regime posterior on any untouched row.
    max_regime_posterior: f64,
    /// Equal-cluster share of untouched units containing a posterior near zero or one.
    boundary_cluster_fraction: f64,
    /// Counts of zero/nonfinite baseline evaluations for `10`, `01`, and `11` ratios.
    nonfinite_ratio_count: [u32; 3],
    /// True when the frozen descriptive thresholds flag an overlap concern.
    overlap_alarm: bool,
    /// Frozen posterior boundary used by this descriptive lens.
    posterior_floor: f64,
    /// Always false: overlap diagnostics do not calibrate the closure comparison.
    calibrated_test: bool,
}

/// Aggregate dependence-unit-honest proper-loss diagnostic.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CrossFittedClosureDiagnostic {
    /// Recorded deterministic seed.
    seed: u64,
    /// Number of outer folds.
    n_folds: usize,
    /// Declared four-corner sampling proportions used as model offsets.
    sampling_proportions: [f64; N_CLASSES],
    /// Exact deterministic nuisance-fit configuration.
    fit_config: ClosureFitConfig,
    /// Number of declared dependence units.
    n_dependence_units: u32,
    /// Frozen row-weight estimand.
    observation_weighting: ClosureObservationWeighting,
    /// Unit-role authority; always declared and unverified in this module.
    unit_authority: ClosureUnitAuthority,
    /// SHA-256 binding episode IDs, dependence units, classes, folds, seed, and fold count.
    fold_plan_sha256: String,
    /// Per-fold held-out diagnostics.
    folds: Vec<FoldClosureDiagnostic>,
    /// Cluster-weighted loss across all held-out units.
    restricted_log_loss: f64,
    /// Cluster-weighted loss across all held-out units.
    saturated_log_loss: f64,
    /// Restricted minus saturated aggregate loss.
    saturated_advantage: f64,
    /// Aggregate out-of-fold regularized linear interaction moments.
    fitted_linear_interaction: FittedLinearInteractionSummary,
    /// Always false: proper-loss improvement is not a calibrated test.
    calibrated_test: bool,
}

impl CrossFittedClosureDiagnostic {
    /// Recorded deterministic fold seed.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Number of declared dependence units summarized by the diagnostic.
    #[must_use]
    pub fn n_dependence_units(&self) -> u32 {
        self.n_dependence_units
    }

    /// Restricted minus saturated aggregate held-out loss.
    #[must_use]
    pub fn saturated_advantage(&self) -> f64 {
        self.saturated_advantage
    }

    /// Content binding of the deterministic dependence-unit fold plan.
    #[must_use]
    pub fn fold_plan_sha256(&self) -> &str {
        &self.fold_plan_sha256
    }

    /// Whether this cross-fitted comparison is a calibrated test. Always false.
    #[must_use]
    pub fn calibrated_test(&self) -> bool {
        self.calibrated_test
    }

    /// Model-dependent fitted linear interaction-field summary.
    #[must_use]
    pub fn fitted_linear_interaction(&self) -> &FittedLinearInteractionSummary {
        &self.fitted_linear_interaction
    }
}

/// Fail-closed cross-fitting errors.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ClosureCrossFitError {
    /// No clustered rows were supplied.
    #[error("cross-fitted closure requires clustered samples")]
    EmptySamples,
    /// The outer-fold count cannot separate discovery and confirmation.
    #[error("outer fold count must be at least two and no greater than the cluster count")]
    InvalidFoldCount,
    /// A row omitted its declared unit identifier.
    #[error("dependence-unit and assignment-episode identifiers must be nonempty")]
    EmptyUnitId,
    /// One assignment episode appeared under two regimes.
    #[error("assignment episode {episode_id} spans corner classes {first} and {second}")]
    EpisodeSpansCorners {
        /// Conflicting assignment episode identifier.
        episode_id: String,
        /// First observed class.
        first: usize,
        /// Conflicting class.
        second: usize,
    },
    /// One assignment episode appeared under two dependence units.
    #[error("assignment episode {episode_id} spans dependence units {first} and {second}")]
    EpisodeSpansDependenceUnits {
        /// Conflicting assignment episode identifier.
        episode_id: String,
        /// First declared dependence unit.
        first: String,
        /// Conflicting dependence unit.
        second: String,
    },
    /// A cluster row count exceeded the platform-independent diagnostic limit.
    #[error("cluster {cluster_id} has more than u32::MAX rows")]
    ClusterTooLarge {
        /// Oversized cluster identifier.
        cluster_id: String,
    },
    /// A training or confirmation slice omitted a design corner.
    #[error("fold {fold} {split} slice omits corner class {class}")]
    MissingFoldClass {
        /// Outer fold.
        fold: usize,
        /// `training` or `confirmation`.
        split: &'static str,
        /// Missing class.
        class: usize,
    },
    /// A corner has too few dependence units for the requested folds.
    #[error("corner class {class} has {n_clusters} dependence units, fewer than {n_folds} folds")]
    InsufficientClassClusters {
        /// Corner class.
        class: usize,
        /// Available dependence units in that class.
        n_clusters: usize,
        /// Requested outer folds.
        n_folds: usize,
    },
    /// A lower-level model fit or loss computation failed.
    #[error(transparent)]
    Fit(#[from] ClosureFitError),
}

#[derive(Debug, Clone)]
struct ClusterMeta {
    dependence_unit_id: String,
    class: usize,
    fold: usize,
    row_count: u32,
}

struct FoldSlice {
    rows: Vec<MultinomialSample>,
    weights: Vec<f64>,
    n_dependence_units: u32,
    class_mass: [f64; N_CLASSES],
    total_mass: f64,
}

/// Fits the restricted and saturated models out of fold and compares them on
/// untouched dependence units with equal total weight per unit within corner.
pub fn cross_fit_closure_models(
    samples: &[ClusteredMultinomialSample],
    sampling_proportions: [f64; N_CLASSES],
    config: ClosureCrossFitConfig,
) -> Result<CrossFittedClosureDiagnostic, ClosureCrossFitError> {
    if samples.is_empty() {
        return Err(ClosureCrossFitError::EmptySamples);
    }
    if config.n_folds < 2 {
        return Err(ClosureCrossFitError::InvalidFoldCount);
    }

    let clusters = cluster_metadata(samples, config)?;
    let dependence_units = clusters
        .values()
        .map(|meta| meta.dependence_unit_id.as_str())
        .collect::<BTreeSet<_>>();
    let n_dependence_units = u32::try_from(dependence_units.len())
        .map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
    if config.n_folds > dependence_units.len() {
        return Err(ClosureCrossFitError::InvalidFoldCount);
    }
    let total_class_clusters = class_dependence_unit_counts(&clusters);
    let fold_plan_sha256 = fold_plan_fingerprint(&clusters, config)?;

    let mut folds = Vec::with_capacity(config.n_folds);
    let mut restricted_loss_sum = 0.0;
    let mut saturated_loss_sum = 0.0;
    let mut confirmation_mass_sum = 0.0;
    let mut curvature_signed_sum = 0.0;
    let mut curvature_abs_sum = 0.0;
    let mut curvature_square_sum = 0.0;
    let mut curvature_max_abs = 0.0_f64;
    let mut n_baseline_clusters = 0_u32;
    for fold in 0..config.n_folds {
        let (diagnostic, fold_weight) = run_fold(
            samples,
            &clusters,
            fold,
            sampling_proportions,
            total_class_clusters,
            config.fit,
        )?;
        restricted_loss_sum += fold_weight * diagnostic.restricted_log_loss;
        saturated_loss_sum += fold_weight * diagnostic.saturated_log_loss;
        confirmation_mass_sum += fold_weight;
        let baseline_weight = f64::from(diagnostic.fitted_linear_interaction.n_baseline_clusters);
        curvature_signed_sum +=
            baseline_weight * diagnostic.fitted_linear_interaction.baseline_mean;
        curvature_abs_sum +=
            baseline_weight * diagnostic.fitted_linear_interaction.baseline_mean_abs;
        curvature_square_sum +=
            baseline_weight * diagnostic.fitted_linear_interaction.baseline_rms.powi(2);
        curvature_max_abs =
            curvature_max_abs.max(diagnostic.fitted_linear_interaction.baseline_max_abs);
        n_baseline_clusters = n_baseline_clusters
            .checked_add(diagnostic.fitted_linear_interaction.n_baseline_clusters)
            .ok_or(ClosureCrossFitError::InvalidFoldCount)?;
        folds.push(diagnostic);
    }

    let restricted_log_loss = restricted_loss_sum / confirmation_mass_sum;
    let saturated_log_loss = saturated_loss_sum / confirmation_mass_sum;
    let baseline_clusters = f64::from(n_baseline_clusters);
    Ok(CrossFittedClosureDiagnostic {
        seed: config.seed,
        n_folds: config.n_folds,
        sampling_proportions,
        fit_config: config.fit,
        n_dependence_units,
        observation_weighting:
            ClosureObservationWeighting::EqualDependenceUnitWithinCornerThenEpisodeThenRow,
        unit_authority: ClosureUnitAuthority::DeclaredUnverified,
        fold_plan_sha256,
        folds,
        restricted_log_loss,
        saturated_log_loss,
        saturated_advantage: restricted_log_loss - saturated_log_loss,
        fitted_linear_interaction: FittedLinearInteractionSummary {
            baseline_mean: curvature_signed_sum / baseline_clusters,
            baseline_mean_abs: curvature_abs_sum / baseline_clusters,
            baseline_rms: (curvature_square_sum / baseline_clusters).sqrt(),
            baseline_max_abs: curvature_max_abs,
            n_baseline_clusters,
            calibrated_test: false,
        },
        calibrated_test: false,
    })
}

fn run_fold(
    samples: &[ClusteredMultinomialSample],
    clusters: &BTreeMap<String, ClusterMeta>,
    fold: usize,
    sampling_proportions: [f64; N_CLASSES],
    total_class_clusters: [u32; N_CLASSES],
    fit: ClosureFitConfig,
) -> Result<(FoldClosureDiagnostic, f64), ClosureCrossFitError> {
    let training = fold_slice(samples, clusters, fold, false, sampling_proportions, None)?;
    let confirmation = fold_slice(
        samples,
        clusters,
        fold,
        true,
        sampling_proportions,
        Some(total_class_clusters),
    )?;
    require_all_classes(&training.rows, fold, "training")?;
    require_all_classes(&confirmation.rows, fold, "confirmation")?;
    let restricted = FourCornerClosureModel::fit_weighted(
        &training.rows,
        &training.weights,
        sampling_proportions,
        ClosureModelKind::MainEffectsOnly,
        fit,
    )?;
    let saturated = FourCornerClosureModel::fit_weighted(
        &training.rows,
        &training.weights,
        sampling_proportions,
        ClosureModelKind::MainEffectsPlusInteraction,
        fit,
    )?;
    let comparison = compare_held_out_closure_models_weighted(
        &restricted,
        &saturated,
        &confirmation.rows,
        &confirmation.weights,
    )?;
    let fitted_linear_interaction =
        fold_fitted_linear_interaction_summary(samples, clusters, fold, &saturated)?;
    let overlap = fold_overlap_summary(samples, clusters, fold, &saturated, sampling_proportions)?;
    let diagnostic = FoldClosureDiagnostic {
        fold,
        n_training_dependence_units: training.n_dependence_units,
        n_confirmation_dependence_units: confirmation.n_dependence_units,
        training_class_mass: training.class_mass,
        confirmation_class_mass: confirmation.class_mass,
        restricted_log_loss: comparison.restricted_log_loss,
        saturated_log_loss: comparison.saturated_log_loss,
        saturated_advantage: comparison.saturated_advantage,
        fitted_linear_interaction,
        overlap,
    };
    Ok((diagnostic, confirmation.total_mass))
}

fn cluster_metadata(
    samples: &[ClusteredMultinomialSample],
    config: ClosureCrossFitConfig,
) -> Result<BTreeMap<String, ClusterMeta>, ClosureCrossFitError> {
    let mut episodes = BTreeMap::<String, ClusterMeta>::new();
    for sample in samples {
        if sample.dependence_unit_id.is_empty() || sample.assignment_episode_id.is_empty() {
            return Err(ClosureCrossFitError::EmptyUnitId);
        }
        if sample.class >= N_CLASSES {
            return Err(ClosureFitError::ClassOutOfRange {
                class: sample.class,
            }
            .into());
        }
        match episodes.get_mut(&sample.assignment_episode_id) {
            Some(meta) => {
                if meta.class != sample.class {
                    return Err(ClosureCrossFitError::EpisodeSpansCorners {
                        episode_id: sample.assignment_episode_id.clone(),
                        first: meta.class,
                        second: sample.class,
                    });
                }
                if meta.dependence_unit_id != sample.dependence_unit_id {
                    return Err(ClosureCrossFitError::EpisodeSpansDependenceUnits {
                        episode_id: sample.assignment_episode_id.clone(),
                        first: meta.dependence_unit_id.clone(),
                        second: sample.dependence_unit_id.clone(),
                    });
                }
                meta.row_count = meta.row_count.checked_add(1).ok_or_else(|| {
                    ClosureCrossFitError::ClusterTooLarge {
                        cluster_id: sample.assignment_episode_id.clone(),
                    }
                })?;
            }
            None => {
                episodes.insert(
                    sample.assignment_episode_id.clone(),
                    ClusterMeta {
                        dependence_unit_id: sample.dependence_unit_id.clone(),
                        class: sample.class,
                        fold: 0,
                        row_count: 1,
                    },
                );
            }
        }
    }
    let mut dependence_classes = BTreeMap::<String, BTreeSet<usize>>::new();
    for meta in episodes.values() {
        dependence_classes
            .entry(meta.dependence_unit_id.clone())
            .or_default()
            .insert(meta.class);
    }
    let mut by_class: [BTreeSet<String>; N_CLASSES] = std::array::from_fn(|_| BTreeSet::new());
    for (dependence_unit_id, classes) in &dependence_classes {
        for class in classes {
            by_class[*class].insert(dependence_unit_id.clone());
        }
    }
    for (class, dependence_units) in by_class.iter().enumerate() {
        if dependence_units.len() < config.n_folds {
            return Err(ClosureCrossFitError::InsufficientClassClusters {
                class,
                n_clusters: dependence_units.len(),
                n_folds: config.n_folds,
            });
        }
    }
    let folds = assign_dependence_folds(&dependence_classes, config);
    for meta in episodes.values_mut() {
        meta.fold = folds[&meta.dependence_unit_id];
    }
    Ok(episodes)
}

fn assign_dependence_folds(
    dependence_classes: &BTreeMap<String, BTreeSet<usize>>,
    config: ClosureCrossFitConfig,
) -> BTreeMap<String, usize> {
    if dependence_classes
        .values()
        .all(|classes| classes.len() == 1)
    {
        let mut output = BTreeMap::new();
        for class in 0..N_CLASSES {
            let mut units = dependence_classes
                .iter()
                .filter(|(_, classes)| classes.contains(&class))
                .map(|(unit, _)| unit.clone())
                .collect::<Vec<_>>();
            units.sort_by(|left, right| {
                stratified_fold_hash(config.seed, class, left)
                    .cmp(&stratified_fold_hash(config.seed, class, right))
                    .then_with(|| left.cmp(right))
            });
            for (index, unit) in units.into_iter().enumerate() {
                output.insert(unit, index % config.n_folds);
            }
        }
        return output;
    }

    let mut units = dependence_classes.iter().collect::<Vec<_>>();
    units.sort_by(|(left_id, left), (right_id, right)| {
        right
            .len()
            .cmp(&left.len())
            .then_with(|| {
                dependence_fold_hash(config.seed, left_id)
                    .cmp(&dependence_fold_hash(config.seed, right_id))
            })
            .then_with(|| left_id.cmp(right_id))
    });
    let mut class_counts = vec![[0_usize; N_CLASSES]; config.n_folds];
    let mut unit_counts = vec![0_usize; config.n_folds];
    let mut output = BTreeMap::new();
    for (unit, classes) in units {
        let fold = (0..config.n_folds)
            .min_by_key(|fold| {
                let class_load = classes
                    .iter()
                    .map(|class| class_counts[*fold][*class])
                    .sum::<usize>();
                (class_load, unit_counts[*fold], *fold)
            })
            .expect("fold count was validated as positive");
        for class in classes {
            class_counts[fold][*class] += 1;
        }
        unit_counts[fold] += 1;
        output.insert(unit.clone(), fold);
    }
    output
}

fn dependence_fold_hash(seed: u64, dependence_unit_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.closure_crossfit.dependence_fold.v1\0");
    hasher.update(seed.to_be_bytes());
    hasher.update(
        u64::try_from(dependence_unit_id.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(dependence_unit_id.as_bytes());
    hasher.finalize().into()
}

fn stratified_fold_hash(seed: u64, class: usize, dependence_unit_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.closure_crossfit.stratified_fold.v1\0");
    hasher.update(seed.to_be_bytes());
    hasher.update(u64::try_from(class).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(
        u64::try_from(dependence_unit_id.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(dependence_unit_id.as_bytes());
    hasher.finalize().into()
}

fn class_dependence_unit_counts(clusters: &BTreeMap<String, ClusterMeta>) -> [u32; N_CLASSES] {
    let mut units_by_class: [BTreeSet<&str>; N_CLASSES] = std::array::from_fn(|_| BTreeSet::new());
    for meta in clusters.values() {
        units_by_class[meta.class].insert(&meta.dependence_unit_id);
    }
    let mut counts = [0_u32; N_CLASSES];
    for (class, units) in units_by_class.iter().enumerate() {
        counts[class] = u32::try_from(units.len()).unwrap_or(u32::MAX);
    }
    counts
}

fn fold_slice(
    samples: &[ClusteredMultinomialSample],
    clusters: &BTreeMap<String, ClusterMeta>,
    fold: usize,
    held_out: bool,
    sampling_proportions: [f64; N_CLASSES],
    denominator_counts: Option<[u32; N_CLASSES]>,
) -> Result<FoldSlice, ClosureCrossFitError> {
    let mut selected_units_by_class: [BTreeSet<&str>; N_CLASSES] =
        std::array::from_fn(|_| BTreeSet::new());
    let mut selected_episodes_by_unit_class = BTreeMap::<(&str, usize), u32>::new();
    for meta in clusters
        .values()
        .filter(|meta| (meta.fold == fold) == held_out)
    {
        selected_units_by_class[meta.class].insert(&meta.dependence_unit_id);
        let count = selected_episodes_by_unit_class
            .entry((&meta.dependence_unit_id, meta.class))
            .or_default();
        *count = count
            .checked_add(1)
            .ok_or(ClosureCrossFitError::InvalidFoldCount)?;
    }
    let mut selected_counts = [0_u32; N_CLASSES];
    for (class, units) in selected_units_by_class.iter().enumerate() {
        selected_counts[class] =
            u32::try_from(units.len()).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
    }
    let selected_dependence_units = clusters
        .values()
        .filter(|meta| (meta.fold == fold) == held_out)
        .map(|meta| meta.dependence_unit_id.as_str())
        .collect::<BTreeSet<_>>();
    let n_dependence_units = u32::try_from(selected_dependence_units.len())
        .map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
    let denominators = denominator_counts.unwrap_or(selected_counts);
    let mut rows = Vec::new();
    let mut weights = Vec::new();
    let mut class_mass = [0.0; N_CLASSES];
    let mut selected = samples
        .iter()
        .filter(|sample| (clusters[&sample.assignment_episode_id].fold == fold) == held_out)
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| canonical_sample_order(left, right));
    for sample in selected {
        let meta = &clusters[&sample.assignment_episode_id];
        rows.push(MultinomialSample {
            features: sample.features.clone(),
            class: sample.class,
        });
        let episode_count =
            selected_episodes_by_unit_class[&(meta.dependence_unit_id.as_str(), meta.class)];
        let weight = sampling_proportions[meta.class]
            / f64::from(denominators[meta.class])
            / f64::from(episode_count)
            / f64::from(meta.row_count);
        weights.push(weight);
        class_mass[meta.class] += weight;
    }
    let total_mass = class_mass.iter().sum();
    Ok(FoldSlice {
        rows,
        weights,
        n_dependence_units,
        class_mass,
        total_mass,
    })
}

fn fold_fitted_linear_interaction_summary(
    samples: &[ClusteredMultinomialSample],
    clusters: &BTreeMap<String, ClusterMeta>,
    fold: usize,
    saturated: &FourCornerClosureModel,
) -> Result<FittedLinearInteractionSummary, ClosureCrossFitError> {
    let mut per_cluster = BTreeMap::<String, (f64, f64, f64, f64, u32)>::new();
    let mut baseline_rows = samples
        .iter()
        .filter(|sample| {
            let meta = &clusters[&sample.assignment_episode_id];
            meta.fold == fold && meta.class == 0
        })
        .collect::<Vec<_>>();
    baseline_rows.sort_by(|left, right| canonical_sample_order(left, right));
    for sample in baseline_rows {
        let interaction = saturated.fitted_interaction_field(&sample.features)?;
        let entry = per_cluster
            .entry(sample.dependence_unit_id.clone())
            .or_default();
        entry.0 += interaction;
        entry.1 += interaction.abs();
        entry.2 += interaction * interaction;
        entry.3 = entry.3.max(interaction.abs());
        entry.4 = entry
            .4
            .checked_add(1)
            .ok_or_else(|| ClosureCrossFitError::ClusterTooLarge {
                cluster_id: sample.dependence_unit_id.clone(),
            })?;
    }
    let n_baseline_clusters =
        u32::try_from(per_cluster.len()).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
    let mut signed = 0.0;
    let mut absolute = 0.0;
    let mut squared = 0.0;
    let mut maximum = 0.0_f64;
    for (sum, abs_sum, square_sum, max_abs, count) in per_cluster.values() {
        let rows = f64::from(*count);
        signed += sum / rows;
        absolute += abs_sum / rows;
        squared += square_sum / rows;
        maximum = maximum.max(*max_abs);
    }
    let clusters = f64::from(n_baseline_clusters);
    Ok(FittedLinearInteractionSummary {
        baseline_mean: signed / clusters,
        baseline_mean_abs: absolute / clusters,
        baseline_rms: (squared / clusters).sqrt(),
        baseline_max_abs: maximum,
        n_baseline_clusters,
        calibrated_test: false,
    })
}

fn fold_overlap_summary(
    samples: &[ClusteredMultinomialSample],
    clusters: &BTreeMap<String, ClusterMeta>,
    fold: usize,
    model: &FourCornerClosureModel,
    sampling_proportions: [f64; N_CLASSES],
) -> Result<FittedOverlapSummary, ClosureCrossFitError> {
    let mut ratio_sums = BTreeMap::<String, ([f64; 3], u32, [bool; 3])>::new();
    let mut boundary_by_cluster = BTreeMap::<String, bool>::new();
    let mut min_regime_posterior = 1.0_f64;
    let mut max_regime_posterior = 0.0_f64;
    let mut max_abs_log_density_ratio = None::<f64>;
    let mut nonfinite_ratio_count = [0_u32; 3];

    let mut held_out = samples
        .iter()
        .filter(|sample| clusters[&sample.assignment_episode_id].fold == fold)
        .collect::<Vec<_>>();
    held_out.sort_by(|left, right| canonical_sample_order(left, right));
    for sample in held_out {
        let posterior = model.predict_probabilities(&sample.features)?;
        let boundary = posterior.iter().any(|probability| {
            *probability <= OVERLAP_POSTERIOR_FLOOR || *probability >= 1.0 - OVERLAP_POSTERIOR_FLOOR
        });
        boundary_by_cluster
            .entry(sample.dependence_unit_id.clone())
            .and_modify(|current| *current |= boundary)
            .or_insert(boundary);
        for probability in posterior {
            min_regime_posterior = min_regime_posterior.min(probability);
            max_regime_posterior = max_regime_posterior.max(probability);
        }
        if sample.class != 0 {
            continue;
        }
        let entry = ratio_sums
            .entry(sample.dependence_unit_id.clone())
            .or_insert(([0.0; 3], 0, [false; 3]));
        entry.1 = entry
            .1
            .checked_add(1)
            .ok_or_else(|| ClosureCrossFitError::ClusterTooLarge {
                cluster_id: sample.dependence_unit_id.clone(),
            })?;
        for class in 1..N_CLASSES {
            let ratio = posterior[class] / posterior[0] * sampling_proportions[0]
                / sampling_proportions[class];
            let ratio_index = class - 1;
            if !ratio.is_finite() || ratio <= 0.0 {
                nonfinite_ratio_count[ratio_index] = nonfinite_ratio_count[ratio_index]
                    .checked_add(1)
                    .ok_or(ClosureCrossFitError::InvalidFoldCount)?;
                entry.2[ratio_index] = true;
            } else {
                let next_sum = entry.0[ratio_index] + ratio;
                if !next_sum.is_finite() {
                    nonfinite_ratio_count[ratio_index] = nonfinite_ratio_count[ratio_index]
                        .checked_add(1)
                        .ok_or(ClosureCrossFitError::InvalidFoldCount)?;
                    entry.2[ratio_index] = true;
                    continue;
                }
                entry.0[ratio_index] = next_sum;
                let magnitude = ratio.ln().abs();
                max_abs_log_density_ratio =
                    Some(max_abs_log_density_ratio.map_or(magnitude, |old| old.max(magnitude)));
            }
        }
    }

    let mut cluster_weights = [Vec::new(), Vec::new(), Vec::new()];
    for (sums, count, invalid) in ratio_sums.values() {
        for index in 0..3 {
            cluster_weights[index].push(if invalid[index] {
                0.0
            } else {
                sums[index] / f64::from(*count)
            });
        }
    }
    let ratio_cluster_ess = cluster_weights.map(|weights| kish_ess(&weights));
    let boundary_clusters = boundary_by_cluster.values().filter(|value| **value).count();
    let boundary_cluster_fraction = boundary_clusters as f64 / boundary_by_cluster.len() as f64;
    Ok(FittedOverlapSummary {
        ratio_cluster_ess,
        max_abs_log_density_ratio,
        min_regime_posterior,
        max_regime_posterior,
        boundary_cluster_fraction,
        nonfinite_ratio_count,
        overlap_alarm: nonfinite_ratio_count.iter().any(|count| *count > 0)
            || boundary_clusters > 0,
        posterior_floor: OVERLAP_POSTERIOR_FLOOR,
        calibrated_test: false,
    })
}

fn kish_ess(weights: &[f64]) -> f64 {
    let maximum = weights.iter().copied().fold(0.0_f64, f64::max);
    if maximum <= 0.0 || !maximum.is_finite() {
        return 0.0;
    }
    let scaled_sum = weights.iter().map(|weight| weight / maximum).sum::<f64>();
    let scaled_square_sum = weights
        .iter()
        .map(|weight| (weight / maximum).powi(2))
        .sum::<f64>();
    scaled_sum.powi(2) / scaled_square_sum
}

fn canonical_sample_order(
    left: &ClusteredMultinomialSample,
    right: &ClusteredMultinomialSample,
) -> std::cmp::Ordering {
    left.dependence_unit_id
        .cmp(&right.dependence_unit_id)
        .then_with(|| left.assignment_episode_id.cmp(&right.assignment_episode_id))
        .then_with(|| left.class.cmp(&right.class))
        .then_with(|| {
            left.features
                .iter()
                .zip(&right.features)
                .map(|(left, right)| left.total_cmp(right))
                .find(|ordering| !ordering.is_eq())
                .unwrap_or_else(|| left.features.len().cmp(&right.features.len()))
        })
}

fn require_all_classes(
    samples: &[MultinomialSample],
    fold: usize,
    split: &'static str,
) -> Result<(), ClosureCrossFitError> {
    let mut seen = [false; N_CLASSES];
    for sample in samples {
        if sample.class < N_CLASSES {
            seen[sample.class] = true;
        }
    }
    if let Some(class) = seen.iter().position(|present| !present) {
        return Err(ClosureCrossFitError::MissingFoldClass { fold, split, class });
    }
    Ok(())
}

fn fold_plan_fingerprint(
    clusters: &BTreeMap<String, ClusterMeta>,
    config: ClosureCrossFitConfig,
) -> Result<String, ClosureCrossFitError> {
    let n_folds =
        u64::try_from(config.n_folds).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
    let mut hasher = Sha256::new();
    hasher.update(b"mic.closure_crossfit.fold_plan.v3\0");
    hasher.update(config.seed.to_be_bytes());
    hasher.update(n_folds.to_be_bytes());
    for (episode_id, meta) in clusters {
        let id_len =
            u64::try_from(episode_id.len()).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
        let dependence_len = u64::try_from(meta.dependence_unit_id.len())
            .map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
        let class =
            u64::try_from(meta.class).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
        let fold = u64::try_from(meta.fold).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
        hasher.update(id_len.to_be_bytes());
        hasher.update(episode_id.as_bytes());
        hasher.update(dependence_len.to_be_bytes());
        hasher.update(meta.dependence_unit_id.as_bytes());
        hasher.update(class.to_be_bytes());
        hasher.update(fold.to_be_bytes());
    }
    let mut encoded = String::with_capacity(64);
    for byte in hasher.finalize() {
        write!(&mut encoded, "{byte:02x}").expect("writing into a String cannot fail");
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn balanced_samples(seed: u64, n_folds: usize) -> Vec<ClusteredMultinomialSample> {
        let _ = seed;
        let mut samples = Vec::new();
        for class in 0..N_CLASSES {
            for cluster in 0..n_folds {
                let cluster_id = format!("class-{class}-cluster-{cluster}");
                let count = if class == 3 { 3 } else { 1 };
                for row in 0..count {
                    let interaction = if class == 3 { 1.0 } else { 0.0 };
                    samples.push(ClusteredMultinomialSample {
                        features: vec![interaction + f64::from(row) * 0.01],
                        class,
                        dependence_unit_id: cluster_id.clone(),
                        assignment_episode_id: cluster_id.clone(),
                    });
                }
            }
        }
        samples
    }

    fn imbalanced_identical_law_samples() -> Vec<ClusteredMultinomialSample> {
        [2_usize, 3, 5, 7]
            .into_iter()
            .enumerate()
            .flat_map(|(class, n_clusters)| {
                (0..n_clusters).map(move |cluster| ClusteredMultinomialSample {
                    features: vec![0.0],
                    class,
                    dependence_unit_id: format!("identical-{class}-{cluster}"),
                    assignment_episode_id: format!("identical-{class}-{cluster}"),
                })
            })
            .collect()
    }

    #[test]
    fn cross_fit_records_seed_and_keeps_cluster_weight() {
        let seed = 29;
        let n_folds = 2;
        let samples = balanced_samples(seed, n_folds);
        let diagnostic = cross_fit_closure_models(
            &samples,
            [0.25; 4],
            ClosureCrossFitConfig {
                seed,
                n_folds,
                fit: ClosureFitConfig {
                    l2_penalty: 0.1,
                    max_iterations: 5_000,
                    gradient_tolerance: 1e-6,
                    ..ClosureFitConfig::default()
                },
            },
        )
        .unwrap();
        assert_eq!(diagnostic.seed, seed);
        assert_eq!(diagnostic.sampling_proportions, [0.25; 4]);
        assert_eq!(diagnostic.fit_config.l2_penalty, 0.1);
        assert_eq!(diagnostic.n_dependence_units, 8);
        assert_eq!(
            diagnostic.observation_weighting,
            ClosureObservationWeighting::EqualDependenceUnitWithinCornerThenEpisodeThenRow
        );
        assert_eq!(
            diagnostic.unit_authority,
            ClosureUnitAuthority::DeclaredUnverified
        );
        assert_eq!(diagnostic.folds.len(), n_folds);
        assert_eq!(diagnostic.fold_plan_sha256.len(), 64);
        assert!(!diagnostic.calibrated_test);
        assert!(!diagnostic.fitted_linear_interaction.calibrated_test);
        assert!(diagnostic.saturated_advantage.is_finite());
        assert!(
            diagnostic
                .folds
                .iter()
                .all(|fold| fold.n_confirmation_dependence_units == 4)
        );
        assert!(diagnostic.folds.iter().all(|fold| {
            fold.training_class_mass
                .iter()
                .all(|mass| (*mass - 0.25).abs() < 1e-12)
        }));
        assert!(diagnostic.folds.iter().all(|fold| {
            fold.overlap
                .ratio_cluster_ess
                .iter()
                .all(|ess| ess.is_finite() && *ess > 0.0)
                && !fold.overlap.calibrated_test
        }));
    }

    #[test]
    fn duplicate_rows_do_not_change_cluster_weighted_diagnostic() {
        let seed = 31;
        let n_folds = 2;
        let samples = balanced_samples(seed, n_folds);
        let mut duplicated = samples.clone();
        for sample in &samples {
            duplicated.extend(std::iter::repeat_n(sample.clone(), 9));
        }
        let config = ClosureCrossFitConfig {
            seed,
            n_folds,
            fit: ClosureFitConfig {
                l2_penalty: 0.1,
                max_iterations: 5_000,
                gradient_tolerance: 1e-6,
                ..ClosureFitConfig::default()
            },
        };
        let original = cross_fit_closure_models(&samples, [0.25; 4], config).unwrap();
        let repeated = cross_fit_closure_models(&duplicated, [0.25; 4], config).unwrap();
        assert!((original.restricted_log_loss - repeated.restricted_log_loss).abs() < 1e-12);
        assert!((original.saturated_log_loss - repeated.saturated_log_loss).abs() < 1e-12);
        assert_eq!(original.fold_plan_sha256, repeated.fold_plan_sha256);
    }

    #[test]
    fn declared_pooling_matches_weighted_fit_under_imbalanced_cluster_counts() {
        let sampling = [0.1, 0.2, 0.3, 0.4];
        let diagnostic = cross_fit_closure_models(
            &imbalanced_identical_law_samples(),
            sampling,
            ClosureCrossFitConfig {
                seed: 73,
                n_folds: 2,
                fit: ClosureFitConfig::default(),
            },
        )
        .unwrap();
        for fold in &diagnostic.folds {
            for (realized, declared) in fold.training_class_mass.iter().zip(sampling) {
                assert!((realized - declared).abs() < 1e-12);
            }
        }
        assert!(diagnostic.saturated_advantage.abs() < 1e-10);
        assert!(diagnostic.fitted_linear_interaction.baseline_mean_abs < 1e-10);
        assert!(diagnostic.fitted_linear_interaction.baseline_rms < 1e-10);
    }

    #[test]
    fn stratified_fold_plan_is_row_order_invariant_and_seed_bound() {
        let samples = imbalanced_identical_law_samples();
        let mut reversed = samples.clone();
        reversed.reverse();
        let config = ClosureCrossFitConfig {
            seed: 79,
            n_folds: 2,
            fit: ClosureFitConfig::default(),
        };
        let original = cross_fit_closure_models(&samples, [0.25; 4], config).unwrap();
        let reordered = cross_fit_closure_models(&reversed, [0.25; 4], config).unwrap();
        assert_eq!(original.fold_plan_sha256, reordered.fold_plan_sha256);
        assert_eq!(original, reordered);

        let changed = cross_fit_closure_models(
            &samples,
            [0.25; 4],
            ClosureCrossFitConfig { seed: 80, ..config },
        )
        .unwrap();
        assert_ne!(original.fold_plan_sha256, changed.fold_plan_sha256);
    }

    #[test]
    fn cross_regime_cluster_and_missing_fold_corner_fail_closed() {
        let mut samples = balanced_samples(7, 2);
        let shared = samples[0].assignment_episode_id.clone();
        samples.push(ClusteredMultinomialSample {
            features: vec![0.0],
            class: 1,
            dependence_unit_id: "conflicting-dependence".into(),
            assignment_episode_id: shared,
        });
        let error = cross_fit_closure_models(
            &samples,
            [0.25; 4],
            ClosureCrossFitConfig {
                seed: 7,
                n_folds: 2,
                fit: ClosureFitConfig::default(),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ClosureCrossFitError::EpisodeSpansCorners { .. }
        ));

        let sparse = balanced_samples(11, 2)
            .into_iter()
            .filter(|sample| {
                sample.class != 3 || sample.assignment_episode_id.ends_with("cluster-0")
            })
            .collect::<Vec<_>>();
        let error = cross_fit_closure_models(
            &sparse,
            [0.25; 4],
            ClosureCrossFitConfig {
                seed: 11,
                n_folds: 2,
                fit: ClosureFitConfig::default(),
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            ClosureCrossFitError::InsufficientClassClusters {
                class: 3,
                n_clusters: 1,
                n_folds: 2,
            }
        );
    }

    #[test]
    fn crossover_episodes_share_a_dependence_fold_without_pseudoreplication() {
        let mut samples = Vec::new();
        for subject in 0..2 {
            for class in 0..N_CLASSES {
                samples.push(ClusteredMultinomialSample {
                    features: vec![f64::from(u32::try_from(class).unwrap()) * 0.01],
                    class,
                    dependence_unit_id: format!("subject-{subject}"),
                    assignment_episode_id: format!("subject-{subject}-period-{class}"),
                });
            }
        }
        let report = cross_fit_closure_models(
            &samples,
            [0.25; 4],
            ClosureCrossFitConfig {
                seed: 91,
                n_folds: 2,
                fit: ClosureFitConfig {
                    l2_penalty: 0.1,
                    max_iterations: 5_000,
                    gradient_tolerance: 1e-6,
                    ..ClosureFitConfig::default()
                },
            },
        )
        .unwrap();
        assert_eq!(report.n_dependence_units(), 2);
        assert!(report.folds.iter().all(|fold| {
            fold.n_training_dependence_units == 1 && fold.n_confirmation_dependence_units == 1
        }));
    }

    #[test]
    fn one_assignment_episode_cannot_cross_dependence_units() {
        let mut samples = balanced_samples(103, 2);
        let episode_id = samples[0].assignment_episode_id.clone();
        let class = samples[0].class;
        samples.push(ClusteredMultinomialSample {
            features: vec![0.0],
            class,
            dependence_unit_id: "different-dependence-unit".into(),
            assignment_episode_id: episode_id,
        });
        let error = cross_fit_closure_models(
            &samples,
            [0.25; 4],
            ClosureCrossFitConfig {
                seed: 103,
                n_folds: 2,
                fit: ClosureFitConfig::default(),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ClosureCrossFitError::EpisodeSpansDependenceUnits { .. }
        ));
    }
}
