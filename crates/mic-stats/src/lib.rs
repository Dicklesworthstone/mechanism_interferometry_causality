#![forbid(unsafe_code)]
//! Reference inference primitives shared by localization, orientation, and curvature tests.

use mic_core::compensated_sum;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Statistical primitive errors.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum StatsError {
    /// A vector length mismatch.
    #[error("all input vectors must have equal nonzero length")]
    Shape,
    /// A probability or bandwidth was invalid.
    #[error("{name} must be finite and in the required range, got {value}")]
    Invalid {
        /// Name of the offending input.
        name: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// A matrix was ragged or had incompatible dimensions.
    #[error("feature matrix is ragged or incompatible")]
    MatrixShape,
}

/// Studentized projected generalized covariance estimate.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GcmEstimate {
    /// Mean weighted residual product.
    pub estimate: f64,
    /// Estimated standard error of the mean.
    pub standard_error: f64,
    /// Studentized statistic.
    pub z_score: f64,
    /// Number of observations.
    pub sample_size: usize,
}

/// Computes a cross-fitted weighted residual-product projection.
pub fn gcm_projection(
    a: &[f64],
    b: &[f64],
    mean_a: &[f64],
    mean_b: &[f64],
    witness: &[f64],
) -> Result<GcmEstimate, StatsError> {
    let n = a.len();
    if n == 0
        || [b.len(), mean_a.len(), mean_b.len(), witness.len()]
            .iter()
            .any(|&m| m != n)
    {
        return Err(StatsError::Shape);
    }
    let terms: Vec<f64> = (0..n)
        .map(|i| witness[i] * (a[i] - mean_a[i]) * (b[i] - mean_b[i]))
        .collect();
    if terms.iter().any(|value| !value.is_finite()) {
        return Err(StatsError::Invalid {
            name: "residual product",
            value: f64::NAN,
        });
    }
    let estimate = compensated_sum(&terms) / n as f64;
    let centered_sq: Vec<f64> = terms
        .iter()
        .map(|value| (value - estimate).powi(2))
        .collect();
    let variance = if n > 1 {
        compensated_sum(&centered_sq) / (n - 1) as f64
    } else {
        0.0
    };
    let standard_error = (variance / n as f64).sqrt();
    let z_score = if standard_error > 0.0 {
        estimate / standard_error
    } else {
        0.0
    };
    Ok(GcmEstimate {
        estimate,
        standard_error,
        z_score,
        sample_size: n,
    })
}

/// Kish effective sample size for nonnegative weights.
pub fn effective_sample_size(weights: &[f64]) -> Result<f64, StatsError> {
    if weights.is_empty()
        || weights
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(StatsError::Invalid {
            name: "weight",
            value: f64::NAN,
        });
    }
    let sum = compensated_sum(weights);
    let squares: Vec<f64> = weights.iter().map(|value| value * value).collect();
    let sum_sq = compensated_sum(&squares);
    Ok(if sum_sq > 0.0 {
        sum * sum / sum_sq
    } else {
        0.0
    })
}

/// Unbiased squared maximum mean discrepancy using an RBF kernel.
pub fn mmd2_unbiased(
    left: &[Vec<f64>],
    right: &[Vec<f64>],
    bandwidth: f64,
) -> Result<f64, StatsError> {
    validate_matrix_pair(left, right)?;
    if !bandwidth.is_finite() || bandwidth <= 0.0 {
        return Err(StatsError::Invalid {
            name: "bandwidth",
            value: bandwidth,
        });
    }
    let n = left.len();
    let m = right.len();
    if n < 2 || m < 2 {
        return Err(StatsError::Shape);
    }
    let gamma = 1.0 / (2.0 * bandwidth * bandwidth);
    let mut xx = 0.0;
    for i in 0..n {
        for j in 0..n {
            if i != j {
                xx += (-gamma * squared_distance(&left[i], &left[j])).exp();
            }
        }
    }
    let mut yy = 0.0;
    for i in 0..m {
        for j in 0..m {
            if i != j {
                yy += (-gamma * squared_distance(&right[i], &right[j])).exp();
            }
        }
    }
    let mut xy = 0.0;
    for x in left {
        for y in right {
            xy += (-gamma * squared_distance(x, y)).exp();
        }
    }
    Ok(xx / (n * (n - 1)) as f64 + yy / (m * (m - 1)) as f64 - 2.0 * xy / (n * m) as f64)
}

/// Sample energy distance between two multivariate samples.
pub fn energy_distance(left: &[Vec<f64>], right: &[Vec<f64>]) -> Result<f64, StatsError> {
    validate_matrix_pair(left, right)?;
    let n = left.len();
    let m = right.len();
    if n == 0 || m == 0 {
        return Err(StatsError::Shape);
    }
    let mut xy = 0.0;
    for x in left {
        for y in right {
            xy += squared_distance(x, y).sqrt();
        }
    }
    let mut xx = 0.0;
    for x in left {
        for y in left {
            xx += squared_distance(x, y).sqrt();
        }
    }
    let mut yy = 0.0;
    for x in right {
        for y in right {
            yy += squared_distance(x, y).sqrt();
        }
    }
    Ok(2.0 * xy / (n * m) as f64 - xx / (n * n) as f64 - yy / (m * m) as f64)
}

/// Relative deletion discrepancy used by the equivalence state machine.
pub fn relative_discrepancy(deletion: f64, full: f64, stabilizer: f64) -> Result<f64, StatsError> {
    if !deletion.is_finite() || deletion < 0.0 {
        return Err(StatsError::Invalid {
            name: "deletion discrepancy",
            value: deletion,
        });
    }
    if !full.is_finite() || full < 0.0 {
        return Err(StatsError::Invalid {
            name: "full discrepancy",
            value: full,
        });
    }
    if !stabilizer.is_finite() || stabilizer <= 0.0 {
        return Err(StatsError::Invalid {
            name: "stabilizer",
            value: stabilizer,
        });
    }
    Ok(deletion / (full + stabilizer))
}

/// Small deterministic generator used only for reference bootstrap and fixtures.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Creates a deterministic generator.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64-bit value.
    #[must_use]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `0..upper` using rejection sampling.
    pub fn index(&mut self, upper: usize) -> Result<usize, StatsError> {
        if upper == 0 {
            return Err(StatsError::Shape);
        }
        let upper64 = upper as u64;
        let zone = u64::MAX - u64::MAX % upper64;
        loop {
            let value = self.next_u64();
            if value < zone {
                return usize::try_from(value % upper64).map_err(|_| StatsError::Shape);
            }
        }
    }
}

/// One candidate localized support scored on held-out data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateSupport {
    /// State-variable names in the candidate support.
    pub variables: Vec<String>,
    /// Held-out proper regime-prediction loss for a model fit on this support.
    pub holdout_loss: f64,
    /// Explicit nonnegative complexity measure for the fitted model.
    pub complexity: f64,
}

/// Parsimony-frontier summary of a completed localization ensemble.
///
/// The frontier is the set of candidates whose held-out loss is within
/// `loss_tolerance` of the best loss in the ensemble, ordered by increasing
/// complexity.  By locality, the true support is the smallest support carrying
/// full regime information, so the least-complex frontier member is the
/// preferred localization and per-variable inclusion frequencies across the
/// frontier are reported as stability paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsimonyFrontier {
    /// Best held-out loss over the completed ensemble.
    pub best_loss: f64,
    /// Absolute loss tolerance defining the frontier.
    pub loss_tolerance: f64,
    /// Indices into the candidate slice, ordered by complexity, then loss, then index.
    pub frontier: Vec<usize>,
    /// Variables of the least-complex frontier member.
    pub minimal_support: Vec<String>,
    /// Fraction of frontier members containing each variable, normalized per variable.
    pub inclusion_frequencies: std::collections::BTreeMap<String, f64>,
}

/// Computes the parsimony frontier of a completed localization ensemble.
///
/// The frontier threshold is computed once over the full candidate set; it is
/// never updated incrementally while results accumulate, so the output is
/// invariant to the order in which candidates were produced.
pub fn parsimony_frontier(
    candidates: &[CandidateSupport],
    loss_tolerance: f64,
) -> Result<ParsimonyFrontier, StatsError> {
    if candidates.is_empty() {
        return Err(StatsError::Shape);
    }
    if !loss_tolerance.is_finite() || loss_tolerance < 0.0 {
        return Err(StatsError::Invalid {
            name: "loss tolerance",
            value: loss_tolerance,
        });
    }
    for candidate in candidates {
        if !candidate.holdout_loss.is_finite() {
            return Err(StatsError::Invalid {
                name: "holdout loss",
                value: candidate.holdout_loss,
            });
        }
        if !candidate.complexity.is_finite() || candidate.complexity < 0.0 {
            return Err(StatsError::Invalid {
                name: "complexity",
                value: candidate.complexity,
            });
        }
        if candidate
            .variables
            .iter()
            .any(|name| name.trim().is_empty())
        {
            return Err(StatsError::Invalid {
                name: "variable name",
                value: f64::NAN,
            });
        }
        let mut unique = std::collections::BTreeSet::new();
        if !candidate.variables.iter().all(|name| unique.insert(name)) {
            return Err(StatsError::Invalid {
                name: "duplicate variable",
                value: f64::NAN,
            });
        }
    }
    let best_loss = candidates
        .iter()
        .map(|candidate| candidate.holdout_loss)
        .fold(f64::INFINITY, f64::min);
    let threshold = best_loss + loss_tolerance;
    let mut frontier: Vec<usize> = (0..candidates.len())
        .filter(|&index| candidates[index].holdout_loss <= threshold)
        .collect();
    frontier.sort_by(|&left, &right| {
        candidates[left]
            .complexity
            .total_cmp(&candidates[right].complexity)
            .then(
                candidates[left]
                    .holdout_loss
                    .total_cmp(&candidates[right].holdout_loss),
            )
            .then(left.cmp(&right))
    });
    let minimal_support = {
        let mut variables = candidates[frontier[0]].variables.clone();
        variables.sort();
        variables
    };
    let mut inclusion_frequencies = std::collections::BTreeMap::new();
    let frontier_size = frontier.len() as f64;
    for &index in &frontier {
        for name in &candidates[index].variables {
            *inclusion_frequencies.entry(name.clone()).or_insert(0.0) += 1.0;
        }
    }
    for value in inclusion_frequencies.values_mut() {
        *value /= frontier_size;
    }
    Ok(ParsimonyFrontier {
        best_loss,
        loss_tolerance,
        frontier,
        minimal_support,
        inclusion_frequencies,
    })
}

fn validate_matrix_pair(left: &[Vec<f64>], right: &[Vec<f64>]) -> Result<(), StatsError> {
    if left.is_empty() || right.is_empty() {
        return Err(StatsError::Shape);
    }
    let dimension = left[0].len();
    if dimension == 0
        || left
            .iter()
            .chain(right)
            .any(|row| row.len() != dimension || row.iter().any(|value| !value.is_finite()))
    {
        return Err(StatsError::MatrixShape);
    }
    Ok(())
}

fn squared_distance(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(&x, &y)| (x - y).powi(2)).sum()
}

/// Feature-gated marker proving the Franken numerical adapters were selected.
#[cfg(feature = "franken")]
pub mod franken {
    /// Returns the pinned integration family used by this build.
    #[must_use]
    pub const fn backend_name() -> &'static str {
        "FrankenNumPy + FrankenSciPy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcm_zero_when_one_residual_is_zero() {
        let estimate = gcm_projection(
            &[0.0, 1.0, 0.0, 1.0],
            &[0.0, 0.0, 1.0, 1.0],
            &[0.0, 1.0, 0.0, 1.0],
            &[0.5; 4],
            &[1.0; 4],
        )
        .unwrap();
        assert_eq!(estimate.estimate, 0.0);
    }

    #[test]
    fn identical_samples_have_zero_energy_distance() {
        let sample = vec![vec![0.0], vec![1.0], vec![2.0]];
        assert!(energy_distance(&sample, &sample).unwrap().abs() < 1e-14);
    }

    #[test]
    fn deterministic_rng_repeats() {
        let mut left = SplitMix64::new(7);
        let mut right = SplitMix64::new(7);
        for _ in 0..32 {
            assert_eq!(left.next_u64(), right.next_u64());
        }
    }

    fn candidate(variables: &[&str], holdout_loss: f64, complexity: f64) -> CandidateSupport {
        CandidateSupport {
            variables: variables.iter().map(|name| (*name).into()).collect(),
            holdout_loss,
            complexity,
        }
    }

    #[test]
    fn frontier_prefers_smallest_support_within_tolerance() {
        let candidates = vec![
            candidate(&["t", "p1", "p2", "z"], 0.100, 4.0),
            candidate(&["t", "p1", "p2"], 0.101, 3.0),
            candidate(&["t", "p1"], 0.150, 2.0),
            candidate(&["z"], 0.900, 1.0),
        ];
        let frontier = parsimony_frontier(&candidates, 0.005).unwrap();
        assert_eq!(frontier.best_loss, 0.100);
        assert_eq!(frontier.frontier, vec![1, 0]);
        assert_eq!(
            frontier.minimal_support,
            vec!["p1".to_string(), "p2".into(), "t".into()]
        );
        assert_eq!(frontier.inclusion_frequencies["t"], 1.0);
        assert_eq!(frontier.inclusion_frequencies["z"], 0.5);
    }

    #[test]
    fn frontier_is_invariant_to_candidate_order() {
        let forward = vec![
            candidate(&["t", "p1"], 0.10, 2.0),
            candidate(&["t", "p1", "z"], 0.10, 3.0),
            candidate(&["z"], 0.50, 1.0),
        ];
        let reversed: Vec<CandidateSupport> = forward.iter().rev().cloned().collect();
        let left = parsimony_frontier(&forward, 0.01).unwrap();
        let right = parsimony_frontier(&reversed, 0.01).unwrap();
        assert_eq!(left.minimal_support, right.minimal_support);
        assert_eq!(left.inclusion_frequencies, right.inclusion_frequencies);
        assert_eq!(left.best_loss, right.best_loss);
    }

    #[test]
    fn frontier_frequencies_are_per_variable_probabilities() {
        let candidates = vec![
            candidate(&["a", "b"], 0.1, 2.0),
            candidate(&["a", "c"], 0.1, 2.0),
            candidate(&["a"], 0.1, 1.0),
        ];
        let frontier = parsimony_frontier(&candidates, 0.0).unwrap();
        assert_eq!(frontier.inclusion_frequencies["a"], 1.0);
        assert!(
            frontier
                .inclusion_frequencies
                .values()
                .all(|&value| (0.0..=1.0).contains(&value))
        );
        assert_eq!(frontier.minimal_support, vec!["a".to_string()]);
    }

    #[test]
    fn frontier_rejects_duplicate_variables() {
        let error = parsimony_frontier(&[candidate(&["a", "a"], 0.1, 1.0)], 0.0).unwrap_err();
        assert!(matches!(error, StatsError::Invalid { .. }));
    }
}
