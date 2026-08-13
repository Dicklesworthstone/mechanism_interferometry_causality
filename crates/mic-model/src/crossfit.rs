#![forbid(unsafe_code)]
//! Cluster-honest cross-fitting for the four-corner closure diagnostic.
//!
//! Fold assignment is deterministic from a recorded seed. A cluster is never
//! split, every cluster receives equal total weight, and every training and
//! confirmation fold must contain all four corners. The resulting proper-loss
//! advantage remains diagnostic: this module does not calibrate a hypothesis
//! test or issue causal authority.

use std::{collections::BTreeMap, fmt::Write as _};

use mic_data::fold_for_cluster;
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
    /// Cluster-weighted held-out loss under modular closure.
    pub restricted_log_loss: f64,
    /// Cluster-weighted held-out loss with an interaction field.
    pub saturated_log_loss: f64,
    /// Restricted minus saturated loss; positive favors the interaction model.
    pub saturated_advantage: f64,
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
    let fold_plan_sha256 = fold_plan_fingerprint(&clusters, config)?;

    let mut folds = Vec::with_capacity(config.n_folds);
    let mut restricted_loss_sum = 0.0;
    let mut saturated_loss_sum = 0.0;
    for fold in 0..config.n_folds {
        let (training, training_weights, n_training_clusters) =
            fold_slice(samples, &clusters, fold, false)?;
        let (confirmation, confirmation_weights, n_confirmation_clusters) =
            fold_slice(samples, &clusters, fold, true)?;
        require_all_classes(&training, fold, "training")?;
        require_all_classes(&confirmation, fold, "confirmation")?;

        let restricted = FourCornerClosureModel::fit_weighted(
            &training,
            &training_weights,
            sampling_proportions,
            ClosureModelKind::MainEffectsOnly,
            config.fit,
        )?;
        let saturated = FourCornerClosureModel::fit_weighted(
            &training,
            &training_weights,
            sampling_proportions,
            ClosureModelKind::MainEffectsPlusInteraction,
            config.fit,
        )?;
        let comparison = compare_held_out_closure_models_weighted(
            &restricted,
            &saturated,
            &confirmation,
            &confirmation_weights,
        )?;
        let fold_weight = f64::from(n_confirmation_clusters);
        restricted_loss_sum += fold_weight * comparison.restricted_log_loss;
        saturated_loss_sum += fold_weight * comparison.saturated_log_loss;
        folds.push(FoldClosureDiagnostic {
            fold,
            n_training_clusters,
            n_confirmation_clusters,
            restricted_log_loss: comparison.restricted_log_loss,
            saturated_log_loss: comparison.saturated_log_loss,
            saturated_advantage: comparison.saturated_advantage,
        });
    }

    let restricted_log_loss = restricted_loss_sum / f64::from(n_clusters);
    let saturated_log_loss = saturated_loss_sum / f64::from(n_clusters);
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
        calibrated_test: false,
    })
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
        let fold = fold_for_cluster(config.seed, &sample.cluster_id, config.n_folds)
            .ok_or(ClosureCrossFitError::InvalidFoldCount)?;
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
                        fold,
                        row_count: 1,
                    },
                );
            }
        }
    }
    Ok(clusters)
}

fn fold_slice(
    samples: &[ClusteredMultinomialSample],
    clusters: &BTreeMap<String, ClusterMeta>,
    fold: usize,
    held_out: bool,
) -> Result<(Vec<MultinomialSample>, Vec<f64>, u32), ClosureCrossFitError> {
    let n_clusters = u32::try_from(
        clusters
            .values()
            .filter(|meta| (meta.fold == fold) == held_out)
            .count(),
    )
    .map_err(|_| ClosureCrossFitError::InvalidFoldCount)?;
    let mut rows = Vec::new();
    let mut weights = Vec::new();
    for sample in samples {
        let meta = &clusters[&sample.cluster_id];
        if (meta.fold == fold) == held_out {
            rows.push(MultinomialSample {
                features: sample.features.clone(),
                class: sample.class,
            });
            weights.push(1.0 / f64::from(meta.row_count));
        }
    }
    Ok((rows, weights, n_clusters))
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
        let mut samples = Vec::new();
        for class in 0..N_CLASSES {
            for fold in 0..n_folds {
                let cluster_id = (0_u64..10_000)
                    .map(|candidate| format!("class-{class}-candidate-{candidate}"))
                    .find(|candidate| fold_for_cluster(seed, candidate, n_folds) == Some(fold))
                    .expect("10,000 deterministic candidates cover each of two folds");
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
        assert!(diagnostic.saturated_advantage.is_finite());
        assert!(
            diagnostic
                .folds
                .iter()
                .all(|fold| fold.n_confirmation_clusters == 4)
        );
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
            .filter(|sample| {
                sample.class != 3 || fold_for_cluster(11, &sample.cluster_id, 2) != Some(1)
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
            ClosureCrossFitError::MissingFoldClass {
                fold: 0,
                split: "training",
                class: 3,
            }
        );
    }
}
