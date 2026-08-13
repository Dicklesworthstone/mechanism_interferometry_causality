#![forbid(unsafe_code)]
//! Deterministic CPU reference model for joint regime prediction.
//!
//! This module deliberately owns no certificate policy. It fits a finite
//! multinomial nuisance model to an already-frozen training slice and exposes
//! posterior probabilities and proper losses. Cluster-level splitting,
//! sampling contracts, confirmation isolation, and causal status remain the
//! responsibility of their dedicated crates.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One labeled row supplied to the deterministic reference optimizer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MultinomialSample {
    /// Finite feature vector.
    pub features: Vec<f64>,
    /// Regime index in `0..n_classes`.
    pub class: usize,
}

/// Closed configuration for the deterministic batch optimizer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FitConfig {
    /// Number of regime classes. Class zero is the reference logit.
    pub n_classes: usize,
    /// Positive L2 penalty applied to every fitted coefficient and intercept.
    pub l2_penalty: f64,
    /// Maximum full-batch gradient iterations.
    pub max_iterations: usize,
    /// Required Euclidean gradient norm for convergence.
    pub gradient_tolerance: f64,
    /// Initial backtracking step size.
    pub initial_step: f64,
}

impl Default for FitConfig {
    fn default() -> Self {
        Self {
            n_classes: 2,
            l2_penalty: 1e-4,
            max_iterations: 20_000,
            gradient_tolerance: 1e-8,
            initial_step: 1.0,
        }
    }
}

/// Optimization facts attached to the fitted nuisance model.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FitSummary {
    /// Accepted full-batch steps.
    pub iterations: usize,
    /// Unpenalized mean logarithmic loss on the supplied training rows.
    pub training_log_loss: f64,
    /// Penalized objective minimized by the optimizer.
    pub penalized_objective: f64,
    /// Final Euclidean gradient norm.
    pub gradient_l2: f64,
}

/// A reference-class multinomial linear model.
///
/// The reference class has an identically zero logit. Row `c - 1` of
/// `coefficients` and `intercepts` parameterizes class `c`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MultinomialLinearModel {
    n_features: usize,
    n_classes: usize,
    coefficients: Vec<Vec<f64>>,
    intercepts: Vec<f64>,
    summary: FitSummary,
}

impl MultinomialLinearModel {
    /// Fits the deterministic, full-batch, L2-regularized reference model.
    ///
    /// Every declared class must appear. The method returns no model unless the
    /// configured gradient tolerance is reached.
    pub fn fit(
        samples: &[MultinomialSample],
        config: FitConfig,
    ) -> Result<Self, MultinomialFitError> {
        validate_config(config)?;
        let n_features = validate_samples(samples, config.n_classes)?;
        let mut parameters = vec![0.0; (config.n_classes - 1) * (n_features + 1)];
        let mut accepted_steps = 0;

        for _ in 0..config.max_iterations {
            let (objective, gradient) = objective_and_gradient(
                samples,
                config.n_classes,
                n_features,
                config.l2_penalty,
                &parameters,
            );
            let gradient_l2 = squared_l2(&gradient).sqrt();
            if gradient_l2 <= config.gradient_tolerance {
                return build_model(
                    samples,
                    config.n_classes,
                    n_features,
                    config.l2_penalty,
                    parameters,
                    accepted_steps,
                    objective,
                    gradient_l2,
                );
            }

            let gradient_sq = squared_l2(&gradient);
            let mut step = config.initial_step;
            let mut accepted = None;
            for _ in 0..60 {
                let candidate: Vec<f64> = parameters
                    .iter()
                    .zip(&gradient)
                    .map(|(parameter, derivative)| parameter - step * derivative)
                    .collect();
                let candidate_objective = penalized_objective(
                    samples,
                    config.n_classes,
                    n_features,
                    config.l2_penalty,
                    &candidate,
                );
                if candidate_objective.is_finite()
                    && candidate_objective <= objective - 1e-4 * step * gradient_sq
                {
                    accepted = Some(candidate);
                    break;
                }
                step *= 0.5;
            }
            parameters = accepted.ok_or(MultinomialFitError::OptimizationStalled)?;
            accepted_steps += 1;
        }

        let (_, gradient) = objective_and_gradient(
            samples,
            config.n_classes,
            n_features,
            config.l2_penalty,
            &parameters,
        );
        Err(MultinomialFitError::DidNotConverge {
            iterations: config.max_iterations,
            gradient_l2: squared_l2(&gradient).sqrt(),
        })
    }

    /// Number of expected features.
    #[must_use]
    pub const fn n_features(&self) -> usize {
        self.n_features
    }

    /// Number of regime classes.
    #[must_use]
    pub const fn n_classes(&self) -> usize {
        self.n_classes
    }

    /// Read-only optimization summary.
    #[must_use]
    pub const fn summary(&self) -> &FitSummary {
        &self.summary
    }

    /// Predicts normalized posterior regime probabilities.
    pub fn predict_probabilities(&self, features: &[f64]) -> Result<Vec<f64>, MultinomialFitError> {
        if features.len() != self.n_features {
            return Err(MultinomialFitError::FeatureDimension {
                expected: self.n_features,
                actual: features.len(),
            });
        }
        if features.iter().any(|value| !value.is_finite()) {
            return Err(MultinomialFitError::NonFiniteFeature);
        }
        Ok(probabilities_from_parts(
            features,
            &self.coefficients,
            &self.intercepts,
        ))
    }

    /// Computes mean held-out logarithmic loss without refitting.
    pub fn mean_log_loss(&self, samples: &[MultinomialSample]) -> Result<f64, MultinomialFitError> {
        if samples.is_empty() {
            return Err(MultinomialFitError::EmptySamples);
        }
        let mut total = 0.0;
        for sample in samples {
            if sample.class >= self.n_classes {
                return Err(MultinomialFitError::ClassOutOfRange {
                    class: sample.class,
                    n_classes: self.n_classes,
                });
            }
            let probabilities = self.predict_probabilities(&sample.features)?;
            total -= probabilities[sample.class].ln();
        }
        Ok(total / samples.len() as f64)
    }
}

/// Converts posterior regime probabilities into log density ratios.
///
/// For known state-independent sampling proportions `rho`, Bayes' rule gives
/// `log(p_c/p_b) = log(q_c/q_b) + log(rho_b/rho_c)`. This operation does not
/// test the sampling or selection contract; the caller must provide proportions
/// that have already been authorized for its analysis track.
pub fn posterior_log_density_ratios(
    posterior: &[f64],
    sampling_proportions: &[f64],
    baseline: usize,
) -> Result<Vec<f64>, MultinomialFitError> {
    validate_simplex(posterior, "posterior")?;
    validate_simplex(sampling_proportions, "sampling proportions")?;
    if posterior.len() != sampling_proportions.len() {
        return Err(MultinomialFitError::ProbabilityDimension {
            posterior: posterior.len(),
            sampling: sampling_proportions.len(),
        });
    }
    if baseline >= posterior.len() {
        return Err(MultinomialFitError::ClassOutOfRange {
            class: baseline,
            n_classes: posterior.len(),
        });
    }
    let log_q_baseline = posterior[baseline].ln();
    let log_rho_baseline = sampling_proportions[baseline].ln();
    Ok(posterior
        .iter()
        .zip(sampling_proportions)
        .map(|(q, rho)| q.ln() - log_q_baseline + log_rho_baseline - rho.ln())
        .collect())
}

/// Fail-closed errors from the deterministic reference model.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum MultinomialFitError {
    /// No rows were supplied.
    #[error("multinomial fit requires at least one sample")]
    EmptySamples,
    /// The declared number of classes is invalid.
    #[error("n_classes must be at least two")]
    TooFewClasses,
    /// A class label lies outside the declared range.
    #[error("class {class} is outside 0..{n_classes}")]
    ClassOutOfRange {
        /// Observed class.
        class: usize,
        /// Declared class count.
        n_classes: usize,
    },
    /// A declared class was absent from training.
    #[error("class {class} is absent from the training slice")]
    MissingClass {
        /// Missing class.
        class: usize,
    },
    /// Feature vectors did not share one dimension.
    #[error("feature dimension mismatch: expected {expected}, got {actual}")]
    FeatureDimension {
        /// Expected length.
        expected: usize,
        /// Observed length.
        actual: usize,
    },
    /// A feature was NaN or infinite.
    #[error("features must be finite")]
    NonFiniteFeature,
    /// Optimizer configuration was invalid.
    #[error("invalid optimizer configuration: {0}")]
    InvalidConfiguration(&'static str),
    /// Backtracking could not find a finite descending step.
    #[error("deterministic optimizer could not find a descending step")]
    OptimizationStalled,
    /// The optimizer exhausted its explicit iteration budget.
    #[error("optimizer did not converge after {iterations} iterations; gradient L2={gradient_l2}")]
    DidNotConverge {
        /// Attempted iterations.
        iterations: usize,
        /// Final gradient norm.
        gradient_l2: f64,
    },
    /// A probability simplex was invalid.
    #[error("{0} must be a nonempty, finite, strictly positive normalized simplex")]
    InvalidSimplex(&'static str),
    /// Posterior and sampling vectors had different lengths.
    #[error("probability dimension mismatch: posterior {posterior}, sampling {sampling}")]
    ProbabilityDimension {
        /// Posterior length.
        posterior: usize,
        /// Sampling-proportion length.
        sampling: usize,
    },
}

fn validate_config(config: FitConfig) -> Result<(), MultinomialFitError> {
    if config.n_classes < 2 {
        return Err(MultinomialFitError::TooFewClasses);
    }
    if !config.l2_penalty.is_finite() || config.l2_penalty <= 0.0 {
        return Err(MultinomialFitError::InvalidConfiguration(
            "l2_penalty must be finite and positive",
        ));
    }
    if config.max_iterations == 0 {
        return Err(MultinomialFitError::InvalidConfiguration(
            "max_iterations must be positive",
        ));
    }
    if !config.gradient_tolerance.is_finite() || config.gradient_tolerance <= 0.0 {
        return Err(MultinomialFitError::InvalidConfiguration(
            "gradient_tolerance must be finite and positive",
        ));
    }
    if !config.initial_step.is_finite() || config.initial_step <= 0.0 {
        return Err(MultinomialFitError::InvalidConfiguration(
            "initial_step must be finite and positive",
        ));
    }
    Ok(())
}

fn validate_samples(
    samples: &[MultinomialSample],
    n_classes: usize,
) -> Result<usize, MultinomialFitError> {
    let first = samples.first().ok_or(MultinomialFitError::EmptySamples)?;
    let n_features = first.features.len();
    let mut seen = vec![false; n_classes];
    for sample in samples {
        if sample.features.len() != n_features {
            return Err(MultinomialFitError::FeatureDimension {
                expected: n_features,
                actual: sample.features.len(),
            });
        }
        if sample.features.iter().any(|value| !value.is_finite()) {
            return Err(MultinomialFitError::NonFiniteFeature);
        }
        if sample.class >= n_classes {
            return Err(MultinomialFitError::ClassOutOfRange {
                class: sample.class,
                n_classes,
            });
        }
        seen[sample.class] = true;
    }
    if let Some(class) = seen.iter().position(|present| !present) {
        return Err(MultinomialFitError::MissingClass { class });
    }
    Ok(n_features)
}

fn build_model(
    samples: &[MultinomialSample],
    n_classes: usize,
    n_features: usize,
    l2_penalty: f64,
    parameters: Vec<f64>,
    iterations: usize,
    objective: f64,
    gradient_l2: f64,
) -> Result<MultinomialLinearModel, MultinomialFitError> {
    let width = n_features + 1;
    let mut coefficients = Vec::with_capacity(n_classes - 1);
    let mut intercepts = Vec::with_capacity(n_classes - 1);
    for class in 1..n_classes {
        let start = (class - 1) * width;
        intercepts.push(parameters[start]);
        coefficients.push(parameters[start + 1..start + width].to_vec());
    }
    let training_log_loss = mean_log_loss_from_parts(samples, &coefficients, &intercepts);
    debug_assert!(
        (objective - (training_log_loss + 0.5 * l2_penalty * squared_l2(&parameters))).abs() < 1e-8
    );
    Ok(MultinomialLinearModel {
        n_features,
        n_classes,
        coefficients,
        intercepts,
        summary: FitSummary {
            iterations,
            training_log_loss,
            penalized_objective: objective,
            gradient_l2,
        },
    })
}

fn objective_and_gradient(
    samples: &[MultinomialSample],
    n_classes: usize,
    n_features: usize,
    l2_penalty: f64,
    parameters: &[f64],
) -> (f64, Vec<f64>) {
    let width = n_features + 1;
    let mut gradient = vec![0.0; parameters.len()];
    let mut log_loss = 0.0;
    for sample in samples {
        let probabilities =
            probabilities_from_parameters(&sample.features, n_classes, n_features, parameters);
        log_loss -= probabilities[sample.class].ln();
        for class in 1..n_classes {
            let residual = probabilities[class] - f64::from(sample.class == class);
            let start = (class - 1) * width;
            gradient[start] += residual;
            for (feature_index, feature) in sample.features.iter().enumerate() {
                gradient[start + 1 + feature_index] += residual * feature;
            }
        }
    }
    let scale = 1.0 / samples.len() as f64;
    for (derivative, parameter) in gradient.iter_mut().zip(parameters) {
        *derivative = *derivative * scale + l2_penalty * parameter;
    }
    let objective = log_loss * scale + 0.5 * l2_penalty * squared_l2(parameters);
    (objective, gradient)
}

fn penalized_objective(
    samples: &[MultinomialSample],
    n_classes: usize,
    n_features: usize,
    l2_penalty: f64,
    parameters: &[f64],
) -> f64 {
    let mut log_loss = 0.0;
    for sample in samples {
        let probabilities =
            probabilities_from_parameters(&sample.features, n_classes, n_features, parameters);
        log_loss -= probabilities[sample.class].ln();
    }
    log_loss / samples.len() as f64 + 0.5 * l2_penalty * squared_l2(parameters)
}

fn mean_log_loss_from_parts(
    samples: &[MultinomialSample],
    coefficients: &[Vec<f64>],
    intercepts: &[f64],
) -> f64 {
    samples
        .iter()
        .map(|sample| {
            let probabilities =
                probabilities_from_parts(&sample.features, coefficients, intercepts);
            -probabilities[sample.class].ln()
        })
        .sum::<f64>()
        / samples.len() as f64
}

fn probabilities_from_parameters(
    features: &[f64],
    n_classes: usize,
    n_features: usize,
    parameters: &[f64],
) -> Vec<f64> {
    let width = n_features + 1;
    let mut logits = Vec::with_capacity(n_classes);
    logits.push(0.0);
    for class in 1..n_classes {
        let start = (class - 1) * width;
        let value = parameters[start]
            + features
                .iter()
                .enumerate()
                .map(|(index, feature)| parameters[start + 1 + index] * feature)
                .sum::<f64>();
        logits.push(value);
    }
    stable_softmax(&logits)
}

fn probabilities_from_parts(
    features: &[f64],
    coefficients: &[Vec<f64>],
    intercepts: &[f64],
) -> Vec<f64> {
    let mut logits = Vec::with_capacity(intercepts.len() + 1);
    logits.push(0.0);
    for (coefficient, intercept) in coefficients.iter().zip(intercepts) {
        logits.push(
            intercept
                + coefficient
                    .iter()
                    .zip(features)
                    .map(|(weight, feature)| weight * feature)
                    .sum::<f64>(),
        );
    }
    stable_softmax(&logits)
}

fn stable_softmax(logits: &[f64]) -> Vec<f64> {
    let max = logits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exponentials: Vec<f64> = logits.iter().map(|value| (value - max).exp()).collect();
    let total: f64 = exponentials.iter().sum();
    exponentials
        .into_iter()
        .map(|value| value / total)
        .collect()
}

fn validate_simplex(values: &[f64], name: &'static str) -> Result<(), MultinomialFitError> {
    if values.is_empty()
        || values
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(MultinomialFitError::InvalidSimplex(name));
    }
    let total: f64 = values.iter().sum();
    if (total - 1.0).abs() > 1e-8 {
        return Err(MultinomialFitError::InvalidSimplex(name));
    }
    Ok(())
}

fn squared_l2(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(x: f64, class: usize) -> MultinomialSample {
        MultinomialSample {
            features: vec![x],
            class,
        }
    }

    #[test]
    fn deterministic_fit_improves_held_out_log_loss() {
        let training = vec![
            sample(-2.0, 0),
            sample(-1.5, 0),
            sample(-1.0, 0),
            sample(1.0, 1),
            sample(1.5, 1),
            sample(2.0, 1),
        ];
        let config = FitConfig {
            l2_penalty: 0.1,
            gradient_tolerance: 1e-7,
            ..FitConfig::default()
        };
        let model = MultinomialLinearModel::fit(&training, config).unwrap();
        assert!(model.summary().gradient_l2 <= config.gradient_tolerance);
        assert!(model.mean_log_loss(&training).unwrap() < 2.0_f64.ln());
        let low = model.predict_probabilities(&[-1.0]).unwrap();
        let high = model.predict_probabilities(&[1.0]).unwrap();
        assert!(low[0] > low[1]);
        assert!(high[1] > high[0]);
    }

    #[test]
    fn posterior_ratios_remove_arbitrary_sampling_odds() {
        let state_likelihoods = [0.2, 0.4, 0.1, 0.3];
        let rho = [0.1, 0.2, 0.3, 0.4];
        let normalizer: f64 = state_likelihoods
            .iter()
            .zip(rho)
            .map(|(likelihood, proportion)| likelihood * proportion)
            .sum();
        let posterior: Vec<f64> = state_likelihoods
            .iter()
            .zip(rho)
            .map(|(likelihood, proportion)| likelihood * proportion / normalizer)
            .collect();
        let ratios = posterior_log_density_ratios(&posterior, &rho, 0).unwrap();
        for (actual, likelihood) in ratios.iter().zip(state_likelihoods) {
            let expected = (likelihood / state_likelihoods[0]).ln();
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn fit_rejects_absent_regime() {
        let error = MultinomialLinearModel::fit(
            &[sample(-1.0, 0), sample(1.0, 1)],
            FitConfig {
                n_classes: 3,
                ..FitConfig::default()
            },
        )
        .unwrap_err();
        assert_eq!(error, MultinomialFitError::MissingClass { class: 2 });
    }

    #[test]
    fn probability_reconstruction_fails_closed() {
        let error = posterior_log_density_ratios(&[0.5, 0.5], &[0.5, 0.0], 0).unwrap_err();
        assert_eq!(
            error,
            MultinomialFitError::InvalidSimplex("sampling proportions")
        );
    }
}
