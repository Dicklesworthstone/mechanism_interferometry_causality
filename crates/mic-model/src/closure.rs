#![forbid(unsafe_code)]
//! Hierarchical four-corner joint regime models.
//!
//! The restricted model ties the `11` logit to the sum of the `10` and `01`
//! state fields after inserting known sampling offsets. The saturated model
//! adds an explicit interaction field. Held-out proper-loss advantage is a
//! diagnostic estimand; it is not a calibrated hypothesis test or a certificate.

use crate::{FitSummary, MultinomialSample};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const N_CLASSES: usize = 4;

/// Hierarchical model fitted to the four design corners.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClosureModelKind {
    /// `11` is constrained to the sum of the two primitive state fields.
    MainEffectsOnly,
    /// `11` receives an additional state-dependent interaction field.
    MainEffectsPlusInteraction,
}

/// Deterministic optimizer configuration for one hierarchical model.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClosureFitConfig {
    /// Positive L2 penalty on nonconstant field coefficients; intercepts are unpenalized.
    pub l2_penalty: f64,
    /// Maximum full-batch iterations.
    pub max_iterations: usize,
    /// Required gradient norm for returning a model.
    pub gradient_tolerance: f64,
    /// Initial backtracking step size.
    pub initial_step: f64,
}

impl Default for ClosureFitConfig {
    fn default() -> Self {
        Self {
            l2_penalty: 1e-4,
            max_iterations: 20_000,
            gradient_tolerance: 1e-8,
            initial_step: 1.0,
        }
    }
}

/// Fitted restricted or interaction-augmented four-corner classifier.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FourCornerClosureModel {
    n_features: usize,
    kind: ClosureModelKind,
    sampling_proportions: [f64; N_CLASSES],
    primitive_a: Vec<f64>,
    primitive_b: Vec<f64>,
    interaction: Option<Vec<f64>>,
    summary: FitSummary,
}

impl FourCornerClosureModel {
    /// Fits one hierarchical model on an already-frozen training slice.
    pub fn fit(
        samples: &[MultinomialSample],
        sampling_proportions: [f64; N_CLASSES],
        kind: ClosureModelKind,
        config: ClosureFitConfig,
    ) -> Result<Self, ClosureFitError> {
        let weights = vec![1.0; samples.len()];
        Self::fit_weighted(samples, &weights, sampling_proportions, kind, config)
    }

    /// Fits one hierarchical model with explicit positive observation weights.
    ///
    /// Cluster-honest callers should give every cluster the same total weight;
    /// this method does not infer an assignment unit from rows.
    pub fn fit_weighted(
        samples: &[MultinomialSample],
        weights: &[f64],
        sampling_proportions: [f64; N_CLASSES],
        kind: ClosureModelKind,
        config: ClosureFitConfig,
    ) -> Result<Self, ClosureFitError> {
        validate_config(config)?;
        validate_sampling_proportions(sampling_proportions)?;
        let n_features = validate_samples(samples)?;
        let total_weight = validate_weights(samples, weights)?;
        let weighted_samples = WeightedSamples {
            samples,
            weights,
            total_weight,
        };
        let width = n_features + 1;
        let n_blocks = match kind {
            ClosureModelKind::MainEffectsOnly => 2,
            ClosureModelKind::MainEffectsPlusInteraction => 3,
        };
        let mut parameters = vec![0.0; n_blocks * width];
        let offsets = sampling_offsets(sampling_proportions);
        for accepted_steps in 0..config.max_iterations {
            let (objective, gradient) = objective_and_gradient(
                weighted_samples,
                kind,
                &offsets,
                n_features,
                config.l2_penalty,
                &parameters,
            );
            let gradient_l2 = squared_l2(&gradient).sqrt();
            if gradient_l2 <= config.gradient_tolerance {
                let summary = FitSummary {
                    iterations: accepted_steps,
                    training_log_loss: 0.0,
                    penalized_objective: objective,
                    gradient_l2,
                };
                return Ok(build_model(
                    samples,
                    weights,
                    sampling_proportions,
                    kind,
                    n_features,
                    &parameters,
                    summary,
                ));
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
                let candidate_objective = objective_only(
                    weighted_samples,
                    kind,
                    &offsets,
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
            parameters = accepted.ok_or(ClosureFitError::OptimizationStalled)?;
        }

        let (_, gradient) = objective_and_gradient(
            weighted_samples,
            kind,
            &offsets,
            n_features,
            config.l2_penalty,
            &parameters,
        );
        Err(ClosureFitError::DidNotConverge {
            iterations: config.max_iterations,
            gradient_l2: squared_l2(&gradient).sqrt(),
        })
    }

    /// Returns the hierarchical restriction represented by this fit.
    #[must_use]
    pub const fn kind(&self) -> ClosureModelKind {
        self.kind
    }

    /// Returns the optimizer facts without granting them inferential authority.
    #[must_use]
    pub const fn summary(&self) -> &FitSummary {
        &self.summary
    }

    /// Predicts posterior probabilities ordered as `00,10,01,11`.
    pub fn predict_probabilities(&self, features: &[f64]) -> Result<[f64; 4], ClosureFitError> {
        validate_feature_vector(features, self.n_features)?;
        let basis = basis(features);
        let a = dot(&self.primitive_a, &basis);
        let b = dot(&self.primitive_b, &basis);
        let interaction = self
            .interaction
            .as_ref()
            .map_or(0.0, |field| dot(field, &basis));
        Ok(probabilities([
            0.0,
            (self.sampling_proportions[1] / self.sampling_proportions[0]).ln() + a,
            (self.sampling_proportions[2] / self.sampling_proportions[0]).ln() + b,
            (self.sampling_proportions[3] / self.sampling_proportions[0]).ln()
                + a
                + b
                + interaction,
        ]))
    }

    /// Evaluates the regularized model's fitted interaction projection.
    ///
    /// The restricted model returns exactly zero. The saturated model returns
    /// its additional `11` field because known sampling offsets have already
    /// been separated from the density logits.
    ///
    /// This equals population density curvature only when the hierarchical
    /// regime model is correctly specified. Under misspecification it can
    /// absorb primitive-field approximation error, so the API does not call it
    /// an unqualified curvature estimate.
    pub fn fitted_interaction_field(&self, features: &[f64]) -> Result<f64, ClosureFitError> {
        validate_feature_vector(features, self.n_features)?;
        Ok(self
            .interaction
            .as_ref()
            .map_or(0.0, |field| dot(field, &basis(features))))
    }

    /// Mean logarithmic loss on untouched rows.
    pub fn mean_log_loss(&self, samples: &[MultinomialSample]) -> Result<f64, ClosureFitError> {
        let weights = vec![1.0; samples.len()];
        self.mean_weighted_log_loss(samples, &weights)
    }

    /// Weighted logarithmic loss on untouched rows.
    ///
    /// The weights must be finite and positive. Equal total weight per cluster
    /// prevents row-rich clusters from dominating a predictive diagnostic.
    pub fn mean_weighted_log_loss(
        &self,
        samples: &[MultinomialSample],
        weights: &[f64],
    ) -> Result<f64, ClosureFitError> {
        let total_weight = validate_weights(samples, weights)?;
        let mut loss = 0.0;
        for (sample, weight) in samples.iter().zip(weights) {
            if sample.class >= N_CLASSES {
                return Err(ClosureFitError::ClassOutOfRange {
                    class: sample.class,
                });
            }
            let prediction = self.predict_probabilities(&sample.features)?;
            loss -= weight * prediction[sample.class].ln();
        }
        Ok(loss / total_weight)
    }
}

/// Held-out proper-loss comparison of the tied and interaction models.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HeldOutClosureComparison {
    /// Mean logarithmic loss from the tied main-effects model.
    pub(crate) restricted_log_loss: f64,
    /// Mean logarithmic loss from the interaction-augmented model.
    pub(crate) saturated_log_loss: f64,
    /// Restricted minus saturated loss; positive values favor the interaction.
    pub(crate) saturated_advantage: f64,
    /// Explicit authority boundary.
    calibrated_test: bool,
}

impl HeldOutClosureComparison {
    /// Restricted minus saturated held-out loss.
    #[must_use]
    pub fn saturated_advantage(&self) -> f64 {
        self.saturated_advantage
    }

    /// Whether this comparison is a calibrated test. Always false.
    #[must_use]
    pub fn calibrated_test(&self) -> bool {
        self.calibrated_test
    }
}

/// Compares two independently fitted models on untouched rows.
///
/// This value is diagnostic. Flexible learner advantage is not a calibrated
/// closure test without a separate cluster-honest inferential procedure.
pub fn compare_held_out_closure_models(
    restricted: &FourCornerClosureModel,
    saturated: &FourCornerClosureModel,
    samples: &[MultinomialSample],
) -> Result<HeldOutClosureComparison, ClosureFitError> {
    let weights = vec![1.0; samples.len()];
    compare_held_out_closure_models_weighted(restricted, saturated, samples, &weights)
}

/// Weighted held-out comparison of a compatible restricted/saturated pair.
pub fn compare_held_out_closure_models_weighted(
    restricted: &FourCornerClosureModel,
    saturated: &FourCornerClosureModel,
    samples: &[MultinomialSample],
    weights: &[f64],
) -> Result<HeldOutClosureComparison, ClosureFitError> {
    if restricted.kind != ClosureModelKind::MainEffectsOnly
        || saturated.kind != ClosureModelKind::MainEffectsPlusInteraction
        || restricted.n_features != saturated.n_features
        || restricted.sampling_proportions != saturated.sampling_proportions
    {
        return Err(ClosureFitError::IncompatibleModels);
    }
    let restricted_log_loss = restricted.mean_weighted_log_loss(samples, weights)?;
    let saturated_log_loss = saturated.mean_weighted_log_loss(samples, weights)?;
    Ok(HeldOutClosureComparison {
        restricted_log_loss,
        saturated_log_loss,
        saturated_advantage: restricted_log_loss - saturated_log_loss,
        calibrated_test: false,
    })
}

/// Fail-closed errors for hierarchical regime models.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ClosureFitError {
    /// No data were supplied.
    #[error("four-corner fit requires samples")]
    EmptySamples,
    /// A class was outside the four-corner ordering.
    #[error("class {class} is outside the four-corner ordering 0..4")]
    ClassOutOfRange {
        /// Observed class.
        class: usize,
    },
    /// A training regime was absent.
    #[error("corner class {class} is absent from training")]
    MissingClass {
        /// Missing class.
        class: usize,
    },
    /// Feature dimensions disagreed.
    #[error("feature dimension mismatch: expected {expected}, got {actual}")]
    FeatureDimension {
        /// Expected dimension.
        expected: usize,
        /// Observed dimension.
        actual: usize,
    },
    /// A feature was nonfinite.
    #[error("features must be finite")]
    NonFiniteFeature,
    /// Sampling proportions did not form a positive simplex.
    #[error("sampling proportions must be finite, positive, and sum to one")]
    InvalidSamplingProportions,
    /// Weights were missing, nonfinite, or non-positive.
    #[error("weights must match samples and be finite and positive")]
    InvalidWeights,
    /// Configuration was invalid.
    #[error("invalid closure optimizer configuration: {0}")]
    InvalidConfiguration(&'static str),
    /// Backtracking could not find a descending step.
    #[error("closure optimizer could not find a descending step")]
    OptimizationStalled,
    /// The explicit iteration budget was exhausted.
    #[error(
        "closure optimizer did not converge after {iterations} iterations; gradient L2={gradient_l2}"
    )]
    DidNotConverge {
        /// Attempted iterations.
        iterations: usize,
        /// Final gradient norm.
        gradient_l2: f64,
    },
    /// Comparison inputs did not form a restricted/saturated pair.
    #[error("held-out comparison requires compatible restricted and saturated models")]
    IncompatibleModels,
}

fn validate_config(config: ClosureFitConfig) -> Result<(), ClosureFitError> {
    if !config.l2_penalty.is_finite() || config.l2_penalty <= 0.0 {
        return Err(ClosureFitError::InvalidConfiguration(
            "l2_penalty must be finite and positive",
        ));
    }
    if config.max_iterations == 0 {
        return Err(ClosureFitError::InvalidConfiguration(
            "max_iterations must be positive",
        ));
    }
    if !config.gradient_tolerance.is_finite() || config.gradient_tolerance <= 0.0 {
        return Err(ClosureFitError::InvalidConfiguration(
            "gradient_tolerance must be finite and positive",
        ));
    }
    if !config.initial_step.is_finite() || config.initial_step <= 0.0 {
        return Err(ClosureFitError::InvalidConfiguration(
            "initial_step must be finite and positive",
        ));
    }
    Ok(())
}

fn validate_sampling_proportions(values: [f64; 4]) -> Result<(), ClosureFitError> {
    if values
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        || (values.iter().sum::<f64>() - 1.0).abs() > 1e-8
    {
        return Err(ClosureFitError::InvalidSamplingProportions);
    }
    Ok(())
}

fn validate_samples(samples: &[MultinomialSample]) -> Result<usize, ClosureFitError> {
    let first = samples.first().ok_or(ClosureFitError::EmptySamples)?;
    let n_features = first.features.len();
    let mut seen = [false; N_CLASSES];
    for sample in samples {
        validate_feature_vector(&sample.features, n_features)?;
        if sample.class >= N_CLASSES {
            return Err(ClosureFitError::ClassOutOfRange {
                class: sample.class,
            });
        }
        seen[sample.class] = true;
    }
    if let Some(class) = seen.iter().position(|present| !present) {
        return Err(ClosureFitError::MissingClass { class });
    }
    Ok(n_features)
}

fn validate_weights(
    samples: &[MultinomialSample],
    weights: &[f64],
) -> Result<f64, ClosureFitError> {
    if samples.is_empty() {
        return Err(ClosureFitError::EmptySamples);
    }
    if weights.len() != samples.len()
        || weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(ClosureFitError::InvalidWeights);
    }
    Ok(weights.iter().sum())
}

fn validate_feature_vector(features: &[f64], expected: usize) -> Result<(), ClosureFitError> {
    if features.len() != expected {
        return Err(ClosureFitError::FeatureDimension {
            expected,
            actual: features.len(),
        });
    }
    if features.iter().any(|value| !value.is_finite()) {
        return Err(ClosureFitError::NonFiniteFeature);
    }
    Ok(())
}

fn build_model(
    samples: &[MultinomialSample],
    weights: &[f64],
    sampling_proportions: [f64; 4],
    kind: ClosureModelKind,
    n_features: usize,
    parameters: &[f64],
    summary: FitSummary,
) -> FourCornerClosureModel {
    let width = n_features + 1;
    let primitive_a = parameters[..width].to_vec();
    let primitive_b = parameters[width..2 * width].to_vec();
    let interaction = (kind == ClosureModelKind::MainEffectsPlusInteraction)
        .then(|| parameters[2 * width..3 * width].to_vec());
    let mut model = FourCornerClosureModel {
        n_features,
        kind,
        sampling_proportions,
        primitive_a,
        primitive_b,
        interaction,
        summary,
    };
    model.summary.training_log_loss = model
        .mean_weighted_log_loss(samples, weights)
        .expect("validated training rows remain valid");
    model
}

#[derive(Clone, Copy)]
struct WeightedSamples<'a> {
    samples: &'a [MultinomialSample],
    weights: &'a [f64],
    total_weight: f64,
}

fn objective_and_gradient(
    data: WeightedSamples<'_>,
    kind: ClosureModelKind,
    offsets: &[f64; 4],
    n_features: usize,
    l2_penalty: f64,
    parameters: &[f64],
) -> (f64, Vec<f64>) {
    let width = n_features + 1;
    let mut gradient = vec![0.0; parameters.len()];
    let mut log_loss = 0.0;
    for (sample, weight) in data.samples.iter().zip(data.weights) {
        let basis = basis(&sample.features);
        let prediction = probabilities_from_parameters(kind, offsets, &basis, parameters);
        log_loss -= weight * prediction[sample.class].ln();
        let residual = std::array::from_fn::<_, 4, _>(|class| {
            prediction[class] - f64::from(sample.class == class)
        });
        add_scaled(
            &mut gradient[..width],
            &basis,
            weight * (residual[1] + residual[3]),
        );
        add_scaled(
            &mut gradient[width..2 * width],
            &basis,
            weight * (residual[2] + residual[3]),
        );
        if kind == ClosureModelKind::MainEffectsPlusInteraction {
            add_scaled(
                &mut gradient[2 * width..3 * width],
                &basis,
                weight * residual[3],
            );
        }
    }
    let scale = 1.0 / data.total_weight;
    for (index, (derivative, parameter)) in gradient.iter_mut().zip(parameters).enumerate() {
        *derivative *= scale;
        if index % width != 0 {
            *derivative += l2_penalty * parameter;
        }
    }
    (
        log_loss * scale + 0.5 * l2_penalty * penalized_squared_l2(parameters, width),
        gradient,
    )
}

fn objective_only(
    data: WeightedSamples<'_>,
    kind: ClosureModelKind,
    offsets: &[f64; 4],
    l2_penalty: f64,
    parameters: &[f64],
) -> f64 {
    let log_loss = data
        .samples
        .iter()
        .zip(data.weights)
        .map(|(sample, weight)| {
            let prediction =
                probabilities_from_parameters(kind, offsets, &basis(&sample.features), parameters);
            -weight * prediction[sample.class].ln()
        })
        .sum::<f64>()
        / data.total_weight;
    let n_blocks = match kind {
        ClosureModelKind::MainEffectsOnly => 2,
        ClosureModelKind::MainEffectsPlusInteraction => 3,
    };
    let width = parameters.len() / n_blocks;
    log_loss + 0.5 * l2_penalty * penalized_squared_l2(parameters, width)
}

fn probabilities_from_parameters(
    kind: ClosureModelKind,
    offsets: &[f64; 4],
    basis: &[f64],
    parameters: &[f64],
) -> [f64; 4] {
    let width = basis.len();
    let a = dot(&parameters[..width], basis);
    let b = dot(&parameters[width..2 * width], basis);
    let interaction = if kind == ClosureModelKind::MainEffectsPlusInteraction {
        dot(&parameters[2 * width..3 * width], basis)
    } else {
        0.0
    };
    probabilities([
        offsets[0],
        offsets[1] + a,
        offsets[2] + b,
        offsets[3] + a + b + interaction,
    ])
}

fn sampling_offsets(rho: [f64; 4]) -> [f64; 4] {
    [
        0.0,
        (rho[1] / rho[0]).ln(),
        (rho[2] / rho[0]).ln(),
        (rho[3] / rho[0]).ln(),
    ]
}

fn probabilities(logits: [f64; 4]) -> [f64; 4] {
    let max = logits.into_iter().fold(f64::NEG_INFINITY, f64::max);
    let exponentials = logits.map(|value| (value - max).exp());
    let total = exponentials.iter().sum::<f64>();
    exponentials.map(|value| value / total)
}

fn basis(features: &[f64]) -> Vec<f64> {
    std::iter::once(1.0)
        .chain(features.iter().copied())
        .collect()
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn add_scaled(target: &mut [f64], values: &[f64], scale: f64) {
    for (slot, value) in target.iter_mut().zip(values) {
        *slot += scale * value;
    }
}

fn squared_l2(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum()
}

fn penalized_squared_l2(values: &[f64], width: usize) -> f64 {
    values
        .iter()
        .enumerate()
        .filter(|(index, _)| index % width != 0)
        .map(|(_, value)| value * value)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(x: f64, class: usize) -> MultinomialSample {
        MultinomialSample {
            features: vec![x],
            class,
        }
    }

    fn interaction_rows() -> Vec<MultinomialSample> {
        let mut rows = Vec::new();
        for (x, counts) in [(-1.0, [4, 1, 1, 4]), (1.0, [1, 4, 4, 1])] {
            for (class, count) in counts.into_iter().enumerate() {
                rows.extend((0..count).map(|_| row(x, class)));
            }
        }
        rows
    }

    #[test]
    fn l2_penalty_excludes_every_field_intercept() {
        // Three two-coefficient fields: indices 0,2,4 are intercepts.
        let parameters = [10.0, 1.0, 20.0, 2.0, 30.0, 3.0];
        assert_eq!(penalized_squared_l2(&parameters, 2), 1.0 + 4.0 + 9.0);
    }

    #[test]
    fn saturated_model_wins_on_interaction_world() {
        let rows = interaction_rows();
        let config = ClosureFitConfig {
            l2_penalty: 0.01,
            gradient_tolerance: 1e-7,
            ..ClosureFitConfig::default()
        };
        let restricted = FourCornerClosureModel::fit(
            &rows,
            [0.25; 4],
            ClosureModelKind::MainEffectsOnly,
            config,
        )
        .unwrap();
        let saturated = FourCornerClosureModel::fit(
            &rows,
            [0.25; 4],
            ClosureModelKind::MainEffectsPlusInteraction,
            config,
        )
        .unwrap();
        let comparison = compare_held_out_closure_models(&restricted, &saturated, &rows).unwrap();
        assert!(comparison.saturated_advantage > 0.1);
        assert!(!comparison.calibrated_test);
        assert!(saturated.fitted_interaction_field(&[-1.0]).unwrap() > 0.0);
        assert!(saturated.fitted_interaction_field(&[1.0]).unwrap() < 0.0);
        assert_eq!(restricted.fitted_interaction_field(&[-1.0]).unwrap(), 0.0);
    }

    #[test]
    fn invalid_sampling_and_missing_corner_fail_closed() {
        let rows = interaction_rows();
        let invalid_sampling = FourCornerClosureModel::fit(
            &rows,
            [0.4; 4],
            ClosureModelKind::MainEffectsOnly,
            ClosureFitConfig::default(),
        )
        .unwrap_err();
        assert_eq!(
            invalid_sampling,
            ClosureFitError::InvalidSamplingProportions
        );

        let missing_corner: Vec<_> = rows.into_iter().filter(|row| row.class != 3).collect();
        let error = FourCornerClosureModel::fit(
            &missing_corner,
            [0.25; 4],
            ClosureModelKind::MainEffectsOnly,
            ClosureFitConfig::default(),
        )
        .unwrap_err();
        assert_eq!(error, ClosureFitError::MissingClass { class: 3 });
    }

    #[test]
    fn weak_hidden_sensor_interaction_remains_diagnostic() {
        let mut rows = Vec::new();
        for (class, counts) in [[5, 5], [5, 5], [5, 5], [4, 6]].into_iter().enumerate() {
            for (state, count) in counts.into_iter().enumerate() {
                let x = if state == 0 { -1.0 } else { 1.0 };
                rows.extend((0..count).map(|_| row(x, class)));
            }
        }
        let config = ClosureFitConfig {
            l2_penalty: 0.1,
            max_iterations: 5_000,
            gradient_tolerance: 1e-6,
            ..ClosureFitConfig::default()
        };
        let restricted = FourCornerClosureModel::fit(
            &rows,
            [0.25; 4],
            ClosureModelKind::MainEffectsOnly,
            config,
        )
        .unwrap();
        let saturated = FourCornerClosureModel::fit(
            &rows,
            [0.25; 4],
            ClosureModelKind::MainEffectsPlusInteraction,
            config,
        )
        .unwrap();
        let comparison = compare_held_out_closure_models(&restricted, &saturated, &rows).unwrap();
        assert!(comparison.saturated_advantage > 0.0);
        assert!(!comparison.calibrated_test);
        assert!(saturated.fitted_interaction_field(&[-1.0]).unwrap() < 0.0);
        assert!(saturated.fitted_interaction_field(&[1.0]).unwrap() > 0.0);
    }
}
