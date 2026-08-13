#![forbid(unsafe_code)]
//! Cluster-honest cross-fitting for the four-corner closure diagnostic.
//!
//! Fold assignment is deterministic from a recorded seed. A cluster is never
//! split, every cluster receives equal weight within its corner, declared
//! corner pooling mass matches the classifier offsets, and every training and
//! confirmation fold contains all four corners. The resulting proper-loss
//! advantage remains diagnostic: this module does not calibrate a hypothesis
//! test or issue causal authority.

use std::{collections::BTreeMap, fmt::Write as _};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ClosureFitConfig, ClosureFitError, ClosureModelKind, FourCornerClosureModel, MultinomialSample,
    compare_held_out_closure_models_weighted,
};

const N_CLASSES: usize = 4;

/// One row with its externally declared independent unit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClusteredMultinomialSample {
    /// State features used by the reference regime model.
    pub features: Vec<f64>,
    /// Four-corner class in `00,10,01,11` order.
    pub class: usize,
    /// Declared assignment or independent-unit identifier.
    pub cluster_id: String,
}

/// Deterministic cross-fit plan and nuisance-fit configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClosureCrossFitConfig {
    /// Seed used by the deterministic cluster-fold hash.
    pub seed: u64,
    /// Number of outer folds. Must be at least two.
    pub n_folds: usize,
    /// Configuration shared by every restricted and saturated nuisance fit.
    pub fit: ClosureFitConfig,
}

/// Held-out diagnostic for one outer fold.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FoldClosureDiagnostic {
    /// Held-out fold index.
    pub fold: usize,
    /// Independent units used to fit the nuisance models.
    pub n_training_clusters: u32,
    /// Untouched independent units used for this comparison.
    pub n_confirmation_clusters: u32,
    /// Realized total training weight by corner; equals the declared pooling law.
    pub training_class_mass: [f64; N_CLASSES],
    /// Share of the global confirmation target mass present in this fold.
    pub confirmation_class_mass: [f64; N_CLASSES],
    /// Cluster-weighted held-out loss under modular closure.
    pub restricted_log_loss: f64,
    /// Cluster-weighted held-out loss with an interaction field.
    pub saturated_log_loss: f64,
    /// Restricted minus saturated loss; positive favors the interaction model.
    pub saturated_advantage: f64,
    /// Out-of-fold curvature moments under the held-out baseline law.
    pub curvature: CurvatureFieldSummary,
}

/// Out-of-fold curvature-field moments under the baseline state law.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CurvatureFieldSummary {
    /// Equal-cluster baseline mean of the signed curvature field.
    pub baseline_mean: f64,
    /// Equal-cluster baseline mean absolute curvature.
    pub baseline_mean_abs: f64,
    /// Equal-cluster baseline root-mean-square curvature.
    pub baseline_rms: f64,
    /// Maximum absolute curvature on any held-out baseline row.
    pub baseline_max_abs: f64,
    /// Number of held-out baseline clusters summarized.
    pub n_baseline_clusters: u32,
    /// Always false: these moments are not a calibrated test.
    pub calibrated_test: bool,
}

/// Aggregate cluster-honest proper-loss diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CrossFittedClosureDiagnostic {
    /// Recorded deterministic seed.
    pub seed: u64,
    /// Number of outer folds.
    pub n_folds: usize,
    /// Declared four-corner sampling proportions used as model offsets.
    pub sampling_proportions: [f64; N_CLASSES],
    /// Exact deterministic nuisance-fit configuration.
    pub fit_config: ClosureFitConfig,
    /// Number of independent units.
    pub n_clusters: u32,
    /// SHA-256 binding cluster IDs, classes, folds, seed, and fold count.
    pub fold_plan_sha256: String,
    /// Per-fold held-out diagnostics.
    pub folds: Vec<FoldClosureDiagnostic>,
    /// Cluster-weighted loss across all held-out units.
    pub restricted_log_loss: f64,
    /// Cluster-weighted loss across all held-out units.
    pub saturated_log_loss: f64,
    /// Restricted minus saturated aggregate loss.
    pub saturated_advantage: f64,
    /// Aggregate out-of-fold curvature moments under the baseline law.
    pub curvature: CurvatureFieldSummary,
    /// Always false: proper-loss improvement is not a calibrated test.
    pub calibrated_test: bool,
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
    #[error("cluster identifiers must be nonempty")]
    EmptyClusterId,
    /// One declared independent unit appeared under two regimes.
    #[error("cluster {cluster_id} spans corner classes {first} and {second}")]
    ClusterSpansCorners {
        /// Conflicting cluster identifier.
        cluster_id: String,
        /// First observed class.
        first: usize,
        /// Conflicting class.
        second: usize,
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
    /// A corner has too few independent units for the requested folds.
    #[error("corner class {class} has {n_clusters} clusters, fewer than {n_folds} folds")]
    InsufficientClassClusters {
        /// Corner class.
        class: usize,
        /// Available clusters in that class.
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
    class: usize,
    fold: usize,
    row_count: u32,
}

struct FoldSlice {
    rows: Vec<MultinomialSample>,
    weights: Vec<f64>,
    n_clusters: u32,
    class_mass: [f64; N_CLASSES],
    total_mass: f64,
}

/// Fits the restricted and saturated models out of fold and compares them on
/// untouched clusters with equal total weight per independent unit.
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
    let n_clusters =
        u32::try_from(clusters.len()).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
    if config.n_folds > clusters.len() {
        return Err(ClosureCrossFitError::InvalidFoldCount);
    }
    let total_class_clusters = class_cluster_counts(&clusters);
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
        let baseline_weight = f64::from(diagnostic.curvature.n_baseline_clusters);
        curvature_signed_sum += baseline_weight * diagnostic.curvature.baseline_mean;
        curvature_abs_sum += baseline_weight * diagnostic.curvature.baseline_mean_abs;
        curvature_square_sum += baseline_weight * diagnostic.curvature.baseline_rms.powi(2);
        curvature_max_abs = curvature_max_abs.max(diagnostic.curvature.baseline_max_abs);
        n_baseline_clusters = n_baseline_clusters
            .checked_add(diagnostic.curvature.n_baseline_clusters)
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
        n_clusters,
        fold_plan_sha256,
        folds,
        restricted_log_loss,
        saturated_log_loss,
        saturated_advantage: restricted_log_loss - saturated_log_loss,
        curvature: CurvatureFieldSummary {
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
    let curvature = fold_curvature_summary(samples, clusters, fold, &saturated)?;
    let diagnostic = FoldClosureDiagnostic {
        fold,
        n_training_clusters: training.n_clusters,
        n_confirmation_clusters: confirmation.n_clusters,
        training_class_mass: training.class_mass,
        confirmation_class_mass: confirmation.class_mass,
        restricted_log_loss: comparison.restricted_log_loss,
        saturated_log_loss: comparison.saturated_log_loss,
        saturated_advantage: comparison.saturated_advantage,
        curvature,
    };
    Ok((diagnostic, confirmation.total_mass))
}

fn cluster_metadata(
    samples: &[ClusteredMultinomialSample],
    config: ClosureCrossFitConfig,
) -> Result<BTreeMap<String, ClusterMeta>, ClosureCrossFitError> {
    let mut clusters = BTreeMap::<String, ClusterMeta>::new();
    for sample in samples {
        if sample.cluster_id.is_empty() {
            return Err(ClosureCrossFitError::EmptyClusterId);
        }
        if sample.class >= N_CLASSES {
            return Err(ClosureFitError::ClassOutOfRange {
                class: sample.class,
            }
            .into());
        }
        match clusters.get_mut(&sample.cluster_id) {
            Some(meta) => {
                if meta.class != sample.class {
                    return Err(ClosureCrossFitError::ClusterSpansCorners {
                        cluster_id: sample.cluster_id.clone(),
                        first: meta.class,
                        second: sample.class,
                    });
                }
                meta.row_count = meta.row_count.checked_add(1).ok_or_else(|| {
                    ClosureCrossFitError::ClusterTooLarge {
                        cluster_id: sample.cluster_id.clone(),
                    }
                })?;
            }
            None => {
                clusters.insert(
                    sample.cluster_id.clone(),
                    ClusterMeta {
                        class: sample.class,
                        fold: 0,
                        row_count: 1,
                    },
                );
            }
        }
    }
    let mut by_class: [Vec<String>; N_CLASSES] = std::array::from_fn(|_| Vec::new());
    for (cluster_id, meta) in &clusters {
        by_class[meta.class].push(cluster_id.clone());
    }
    for (class, cluster_ids) in by_class.iter_mut().enumerate() {
        if cluster_ids.len() < config.n_folds {
            return Err(ClosureCrossFitError::InsufficientClassClusters {
                class,
                n_clusters: cluster_ids.len(),
                n_folds: config.n_folds,
            });
        }
        cluster_ids.sort_by(|left, right| {
            stratified_fold_hash(config.seed, class, left)
                .cmp(&stratified_fold_hash(config.seed, class, right))
                .then_with(|| left.cmp(right))
        });
        for (index, cluster_id) in cluster_ids.iter().enumerate() {
            clusters
                .get_mut(cluster_id)
                .expect("class groups were built from the same cluster map")
                .fold = index % config.n_folds;
        }
    }
    Ok(clusters)
}

fn stratified_fold_hash(seed: u64, class: usize, cluster_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"mic.closure_crossfit.stratified_fold.v1\0");
    hasher.update(seed.to_be_bytes());
    hasher.update(class.to_be_bytes());
    hasher.update(cluster_id.len().to_be_bytes());
    hasher.update(cluster_id.as_bytes());
    hasher.finalize().into()
}

fn class_cluster_counts(clusters: &BTreeMap<String, ClusterMeta>) -> [u32; N_CLASSES] {
    let mut counts = [0_u32; N_CLASSES];
    for meta in clusters.values() {
        counts[meta.class] += 1;
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
    let mut selected_counts = [0_u32; N_CLASSES];
    for meta in clusters
        .values()
        .filter(|meta| (meta.fold == fold) == held_out)
    {
        selected_counts[meta.class] += 1;
    }
    let n_clusters = selected_counts
        .iter()
        .try_fold(0_u32, |total, count| total.checked_add(*count));
    let n_clusters = n_clusters.ok_or(ClosureCrossFitError::InvalidFoldCount)?;
    let denominators = denominator_counts.unwrap_or(selected_counts);
    let mut rows = Vec::new();
    let mut weights = Vec::new();
    let mut class_mass = [0.0; N_CLASSES];
    let mut selected = samples
        .iter()
        .filter(|sample| (clusters[&sample.cluster_id].fold == fold) == held_out)
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| canonical_sample_order(left, right));
    for sample in selected {
        let meta = &clusters[&sample.cluster_id];
        rows.push(MultinomialSample {
            features: sample.features.clone(),
            class: sample.class,
        });
        let weight = sampling_proportions[meta.class]
            / f64::from(denominators[meta.class])
            / f64::from(meta.row_count);
        weights.push(weight);
        class_mass[meta.class] += weight;
    }
    let total_mass = class_mass.iter().sum();
    Ok(FoldSlice {
        rows,
        weights,
        n_clusters,
        class_mass,
        total_mass,
    })
}

fn fold_curvature_summary(
    samples: &[ClusteredMultinomialSample],
    clusters: &BTreeMap<String, ClusterMeta>,
    fold: usize,
    saturated: &FourCornerClosureModel,
) -> Result<CurvatureFieldSummary, ClosureCrossFitError> {
    let mut per_cluster = BTreeMap::<String, (f64, f64, f64, f64, u32)>::new();
    let mut baseline_rows = samples
        .iter()
        .filter(|sample| {
            let meta = &clusters[&sample.cluster_id];
            meta.fold == fold && meta.class == 0
        })
        .collect::<Vec<_>>();
    baseline_rows.sort_by(|left, right| canonical_sample_order(left, right));
    for sample in baseline_rows {
        let curvature = saturated.curvature_field(&sample.features)?;
        let entry = per_cluster.entry(sample.cluster_id.clone()).or_default();
        entry.0 += curvature;
        entry.1 += curvature.abs();
        entry.2 += curvature * curvature;
        entry.3 = entry.3.max(curvature.abs());
        entry.4 = entry
            .4
            .checked_add(1)
            .ok_or_else(|| ClosureCrossFitError::ClusterTooLarge {
                cluster_id: sample.cluster_id.clone(),
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
    Ok(CurvatureFieldSummary {
        baseline_mean: signed / clusters,
        baseline_mean_abs: absolute / clusters,
        baseline_rms: (squared / clusters).sqrt(),
        baseline_max_abs: maximum,
        n_baseline_clusters,
        calibrated_test: false,
    })
}

fn canonical_sample_order(
    left: &ClusteredMultinomialSample,
    right: &ClusteredMultinomialSample,
) -> std::cmp::Ordering {
    left.cluster_id
        .cmp(&right.cluster_id)
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
    hasher.update(b"mic.closure_crossfit.fold_plan.v2\0");
    hasher.update(config.seed.to_be_bytes());
    hasher.update(n_folds.to_be_bytes());
    for (cluster_id, meta) in clusters {
        let id_len =
            u64::try_from(cluster_id.len()).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
        let class =
            u64::try_from(meta.class).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
        let fold = u64::try_from(meta.fold).map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
        hasher.update(id_len.to_be_bytes());
        hasher.update(cluster_id.as_bytes());
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
                        cluster_id: cluster_id.clone(),
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
                    cluster_id: format!("identical-{class}-{cluster}"),
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
        assert_eq!(diagnostic.n_clusters, 8);
        assert_eq!(diagnostic.folds.len(), n_folds);
        assert_eq!(diagnostic.fold_plan_sha256.len(), 64);
        assert!(!diagnostic.calibrated_test);
        assert!(!diagnostic.curvature.calibrated_test);
        assert!(diagnostic.saturated_advantage.is_finite());
        assert!(
            diagnostic
                .folds
                .iter()
                .all(|fold| fold.n_confirmation_clusters == 4)
        );
        assert!(diagnostic.folds.iter().all(|fold| {
            fold.training_class_mass
                .iter()
                .all(|mass| (*mass - 0.25).abs() < 1e-12)
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
        assert!(diagnostic.curvature.baseline_mean_abs < 1e-10);
        assert!(diagnostic.curvature.baseline_rms < 1e-10);
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
        let shared = samples[0].cluster_id.clone();
        samples.push(ClusteredMultinomialSample {
            features: vec![0.0],
            class: 1,
            cluster_id: shared,
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
            ClosureCrossFitError::ClusterSpansCorners { .. }
        ));

        let sparse = balanced_samples(11, 2)
            .into_iter()
            .filter(|sample| sample.class != 3 || sample.cluster_id.ends_with("cluster-0"))
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
}
