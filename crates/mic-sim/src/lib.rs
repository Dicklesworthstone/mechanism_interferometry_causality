#![forbid(unsafe_code)]
//! Exact simulation scenarios appearing in the paper.

use serde::{Deserialize, Serialize};

/// Closed-form running example result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunningExample {
    /// Source tilt A.
    pub a: f64,
    /// Source tilt B.
    pub b: f64,
    /// Gaussian noise scale.
    pub sigma: f64,
    /// Additive outcome interaction.
    pub outcome_synergy: f64,
    /// Complete-state mechanism curvature.
    pub full_state_curvature: f64,
    /// Negative-tail outcome-only curvature.
    pub marginal_curvature_negative_limit: f64,
    /// Positive-tail outcome-only curvature.
    pub marginal_curvature_positive_limit: f64,
    /// Scalar primitive-ratio moment.
    pub scalar_moment_battery: f64,
}

impl RunningExample {
    /// Evaluates outcome-only curvature at `y`.
    #[must_use]
    pub fn curvature_at(&self, y: f64) -> f64 {
        (1.0 + self.a * self.b * (y / self.sigma.powi(2)).tanh()).ln()
    }
}

/// Exact parity orientation failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityExample {
    /// Symmetric noise probability.
    pub epsilon: f64,
    /// Candidate support labels.
    pub support: Vec<String>,
    /// Exact `P(T=1)` under the baseline mechanism `T = P xor N`.
    pub baseline_child_marginal: f64,
    /// Exact `P(T=1)` under the intervened mechanism `T = not(P) xor N`.
    pub intervened_child_marginal: f64,
    /// Coordinates whose deletion is invariant, derived from the exact marginals.
    pub invariant_deletions: Vec<String>,
    /// Number of passing deletions, derived from the exact marginals.
    pub pass_count: usize,
}

/// Complete-state-flat latent-factor conservation fixture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatentConservationExample {
    /// Tilt scale.
    pub a: f64,
    /// Primitive A observed ratios for X=-1,+1.
    pub ra: [f64; 2],
    /// Primitive B observed ratios for X=-1,+1.
    pub rb: [f64; 2],
    /// Joint observed ratios.
    pub rab: [f64; 2],
    /// Constant observed curvature.
    pub curvature: f64,
    /// Observable ratio covariance.
    pub observed_ratio_covariance: f64,
    /// Mean hidden conditional covariance.
    pub hidden_conditional_covariance: f64,
    /// Explicit scope caveat.
    pub locality_scope: String,
}

/// Combination-specific implementation inconsistency fixture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImplementationExample {
    /// Primitive A tilt.
    pub a: f64,
    /// Primitive B tilt.
    pub b: f64,
    /// Combination-specific coupling.
    pub gamma: f64,
    /// Required normalizer.
    pub normalizer: f64,
    /// Curvature for product -1.
    pub curvature_minus: f64,
    /// Curvature for product +1.
    pub curvature_plus: f64,
    /// Negative-control curvature.
    pub negative_control_curvature: f64,
}

/// One exact law in the three-node causal-tomography cube.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TomographyLaw {
    /// Binary replacement design in `A`, `B`, `C` order.
    pub design: String,
    /// Joint probabilities in lexicographic `A,B,C` state order.
    pub probabilities: [f64; 8],
}

/// Exact multi-environment fixture for discovering an autonomous causal chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CausalTomographyExample {
    /// State order shared by every regime law.
    pub state_labels: Vec<String>,
    /// All eight single and combined replacement regimes.
    pub laws: Vec<TomographyLaw>,
    /// Exact minimal primitive supports in target order.
    pub primitive_families: Vec<Vec<String>>,
    /// Exact primitive targets.
    pub primitive_targets: Vec<String>,
    /// Union marginal-response sets for one rich tilt per target.
    pub response_sets: Vec<Vec<String>>,
    /// Why this fixture has stronger authority than a fitted benchmark.
    pub authority: String,
}

/// Exact flat four-law family whose primitive ratios are not causal mechanisms.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlatNoncausalCube {
    /// Baseline law in `X,Y = 00,01,10,11` order.
    pub p0: [f64; 4],
    /// First primitive corner.
    pub p10: [f64; 4],
    /// Second primitive corner.
    pub p01: [f64; 4],
    /// Product corner, exactly flat with the other three laws.
    pub p11: [f64; 4],
    /// First globally normalized primitive ratio.
    pub r1: [f64; 4],
    /// Second globally normalized primitive ratio.
    pub r2: [f64; 4],
}

/// Two causal models with the same observed rows and opposite effect signs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdentificationTwin {
    /// Candidate identification strategy.
    pub strategy: String,
    /// Columns in each observed support row.
    pub observed_columns: Vec<String>,
    /// Exact observed support shared by both models.
    pub observed_rows: Vec<Vec<f64>>,
    /// Probability mass for each support row.
    pub observed_masses: Vec<f64>,
    /// Effect under the model satisfying the strategy premise.
    pub premise_model_effect: f64,
    /// Opposite effect under the observationally equivalent violating model.
    pub twin_model_effect: f64,
    /// Unobserved premise that separates the models.
    pub separating_premise: String,
}

/// Exact observational twins for three common natural-experiment strategies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdentificationTwins {
    /// Sharp regression-discontinuity twin.
    pub regression_discontinuity: IdentificationTwin,
    /// Binary instrumental-variable twin.
    pub instrumental_variable: IdentificationTwin,
    /// Two-group, two-period difference-in-differences twin.
    pub difference_in_differences: IdentificationTwin,
}

/// Complete exact simulation bundle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExactSuite {
    /// Nonlinear running example.
    pub running_example: RunningExample,
    /// Symmetry failure.
    pub parity_orientation_failure: ParityExample,
    /// Latent conservation fixture.
    pub latent_conservation: LatentConservationExample,
    /// Implementation inconsistency fixture.
    pub implementation_inconsistency: ImplementationExample,
}

/// Builds the running example.
#[must_use]
pub fn running_example(a: f64, b: f64, sigma: f64) -> RunningExample {
    let product = a * b;
    RunningExample {
        a,
        b,
        sigma,
        outcome_synergy: product,
        full_state_curvature: 0.0,
        marginal_curvature_negative_limit: (1.0 - product).ln(),
        marginal_curvature_positive_limit: (1.0 + product).ln(),
        scalar_moment_battery: 1.0,
    }
}

/// Exact `P(child = 1)` when `child = parent xor noise` with independent Bernoulli noise.
#[must_use]
fn xor_marginal(parent_one_probability: f64, noise: f64) -> f64 {
    parent_one_probability * (1.0 - noise) + (1.0 - parent_one_probability) * noise
}

/// Builds the balanced parity failure with the pass count derived, not asserted.
///
/// Baseline: `T = P xor N` with `P ~ Bernoulli(1/2)`, `N ~ Bernoulli(epsilon)`.
/// Intervention: `T = not(P) xor N`.  The parent mechanism is untouched, so
/// deleting `T` always leaves an invariant parent marginal; deleting `P` is
/// invariant exactly when the child marginal is unchanged, which the balanced
/// parent forces regardless of `epsilon`.
#[must_use]
pub fn parity_example(epsilon: f64) -> ParityExample {
    let baseline_parent_marginal = 0.5;
    // The intervention replaces only the child mechanism, so the parent law is reused verbatim.
    let intervened_parent_marginal = baseline_parent_marginal;
    let baseline_child_marginal = xor_marginal(baseline_parent_marginal, epsilon);
    let intervened_child_marginal = xor_marginal(1.0 - intervened_parent_marginal, epsilon);
    let mut invariant_deletions = Vec::new();
    if (baseline_child_marginal - intervened_child_marginal).abs() == 0.0 {
        invariant_deletions.push("P".to_string());
    }
    if (baseline_parent_marginal - intervened_parent_marginal).abs() == 0.0 {
        invariant_deletions.push("T".to_string());
    }
    let pass_count = invariant_deletions.len();
    ParityExample {
        epsilon,
        support: vec!["P".into(), "T".into()],
        baseline_child_marginal,
        intervened_child_marginal,
        invariant_deletions,
        pass_count,
    }
}

/// Builds the latent conservation fixture.
#[must_use]
pub fn latent_conservation(a: f64) -> LatentConservationExample {
    LatentConservationExample {
        a,
        ra: [1.0 - a, 1.0 + a],
        rb: [1.0 + a, 1.0 - a],
        rab: [1.0, 1.0],
        curvature: -(-(a * a)).ln_1p(),
        observed_ratio_covariance: -(a * a),
        hidden_conditional_covariance: a * a,
        locality_scope: "coordinated multi-source tilts; multiplicative flatness only".into(),
    }
}

/// Builds an exact three-node `A -> B -> C` replacement cube.
///
/// The baseline conditionals are
/// `P(A=1)=1/2`, `P(B=1|A=0,1)=(1/4,3/4)`, and
/// `P(C=1|B=0,1)=(1/5,4/5)`. The three primitive replacements set those
/// conditionals to `3/4`, `(1/2,2/3)`, and `(2/5,2/3)`, respectively.
/// Every combination reuses the untouched baseline conditionals, so the cube
/// is pointwise flat by construction while retaining nontrivial propagation.
#[must_use]
pub fn causal_tomography_chain() -> CausalTomographyExample {
    let laws = (0_u8..8)
        .map(|bits| TomographyLaw {
            design: format!(
                "{}{}{}",
                u8::from(bits & 4 != 0),
                u8::from(bits & 2 != 0),
                u8::from(bits & 1 != 0)
            ),
            probabilities: chain_law(bits),
        })
        .collect();
    CausalTomographyExample {
        state_labels: (0_u8..8).map(|bits| format!("{bits:03b}")).collect(),
        laws,
        primitive_families: vec![
            vec!["A".into()],
            vec!["A".into(), "B".into()],
            vec!["B".into(), "C".into()],
        ],
        primitive_targets: vec!["A".into(), "B".into(), "C".into()],
        response_sets: vec![
            vec!["A".into(), "B".into(), "C".into()],
            vec!["B".into(), "C".into()],
            vec!["C".into()],
        ],
        authority: "exact structural-equation construction".into(),
    }
}

/// Builds a normalized, common-support, rank-two flat cube that has no local
/// normalized single-mechanism interpretation on either two-node DAG.
///
/// This is the smallest conformance witness that low rank, global
/// normalization, and zero curvature do not establish causality. Each
/// primitive ratio depends on both coordinates, and averaging it over either
/// candidate target yields `(5/4, 3/4)` or its reversal rather than one.
#[must_use]
pub fn flat_noncausal_cube() -> FlatNoncausalCube {
    let p0 = [0.25; 4];
    let r1 = [1.5, 1.0, 1.0, 0.5];
    let r2 = [1.0, 1.5, 0.5, 1.0];
    FlatNoncausalCube {
        p0,
        p10: multiply(p0, r1),
        p01: multiply(p0, r2),
        p11: multiply(multiply(p0, r1), r2),
        r1,
        r2,
    }
}

/// Builds exact RD, IV, and difference-in-differences observational twins.
///
/// Each pair has identical observed rows under a `+1` effect model satisfying
/// the identifying premise and a `-1` effect model violating only that premise.
/// These fixtures ensure that a strategy router can nominate a contract but
/// cannot promote continuity, exclusion, or parallel trends from rows alone.
#[must_use]
pub fn identification_twins() -> IdentificationTwins {
    IdentificationTwins {
        regression_discontinuity: IdentificationTwin {
            strategy: "regression_discontinuity".into(),
            observed_columns: vec![
                "running_variable".into(),
                "treatment".into(),
                "outcome".into(),
            ],
            observed_rows: vec![vec![-1.0, 0.0, 0.0], vec![1.0, 1.0, 1.0]],
            observed_masses: vec![0.5, 0.5],
            premise_model_effect: 1.0,
            twin_model_effect: -1.0,
            separating_premise: "continuity of untreated potential outcomes at the cutoff".into(),
        },
        instrumental_variable: IdentificationTwin {
            strategy: "instrumental_variable".into(),
            observed_columns: vec!["instrument".into(), "treatment".into(), "outcome".into()],
            observed_rows: vec![vec![0.0, 0.0, 0.0], vec![1.0, 1.0, 1.0]],
            observed_masses: vec![0.5, 0.5],
            premise_model_effect: 1.0,
            twin_model_effect: -1.0,
            separating_premise: "instrument exclusion from the outcome mechanism".into(),
        },
        difference_in_differences: IdentificationTwin {
            strategy: "difference_in_differences".into(),
            observed_columns: vec![
                "group".into(),
                "post".into(),
                "treatment".into(),
                "outcome".into(),
            ],
            observed_rows: vec![
                vec![0.0, 0.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0, 0.0],
                vec![1.0, 0.0, 0.0, 0.0],
                vec![1.0, 1.0, 1.0, 1.0],
            ],
            observed_masses: vec![0.25; 4],
            premise_model_effect: 1.0,
            twin_model_effect: -1.0,
            separating_premise: "parallel untreated potential-outcome trends".into(),
        },
    }
}

fn multiply(left: [f64; 4], right: [f64; 4]) -> [f64; 4] {
    std::array::from_fn(|index| left[index] * right[index])
}

fn chain_law(design: u8) -> [f64; 8] {
    let mut law = [0.0; 8];
    for state in 0_u8..8 {
        let a = state & 4 != 0;
        let b = state & 2 != 0;
        let c = state & 1 != 0;
        let p_a_one = if design & 4 != 0 {
            3.0 / 4.0
        } else {
            1.0 / 2.0
        };
        let p_b_one = if design & 2 != 0 {
            if a { 2.0 / 3.0 } else { 1.0 / 2.0 }
        } else if a {
            3.0 / 4.0
        } else {
            1.0 / 4.0
        };
        let p_c_one = if design & 1 != 0 {
            if b { 2.0 / 3.0 } else { 2.0 / 5.0 }
        } else if b {
            4.0 / 5.0
        } else {
            1.0 / 5.0
        };
        law[usize::from(state)] =
            bernoulli_mass(a, p_a_one) * bernoulli_mass(b, p_b_one) * bernoulli_mass(c, p_c_one);
    }
    law
}

fn bernoulli_mass(value: bool, probability_one: f64) -> f64 {
    if value {
        probability_one
    } else {
        1.0 - probability_one
    }
}

/// Builds the implementation-inconsistency fixture.
#[must_use]
pub fn implementation_inconsistency(a: f64, b: f64, gamma: f64) -> ImplementationExample {
    let normalizer = 1.0 + a * b * gamma;
    ImplementationExample {
        a,
        b,
        gamma,
        normalizer,
        curvature_minus: (1.0 - gamma).ln() - normalizer.ln(),
        curvature_plus: (1.0 + gamma).ln() - normalizer.ln(),
        negative_control_curvature: 0.0,
    }
}

/// Returns the paper's default exact suite.
#[must_use]
pub fn exact_suite() -> ExactSuite {
    ExactSuite {
        running_example: running_example(0.6, 0.5, 0.8),
        parity_orientation_failure: parity_example(0.1),
        latent_conservation: latent_conservation(0.3),
        implementation_inconsistency: implementation_inconsistency(0.45, 0.35, 0.4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_values_match() {
        let suite = exact_suite();
        assert!((suite.running_example.outcome_synergy - 0.3).abs() < 1e-14);
        assert!((suite.latent_conservation.curvature - 0.094_310_679_471_241_32).abs() < 1e-14);
        assert!(
            (suite.implementation_inconsistency.curvature_minus + 0.571_920_723_125_801_6).abs()
                < 1e-14
        );
        assert_eq!(suite.parity_orientation_failure.pass_count, 2);
    }

    #[test]
    fn parity_pass_count_is_derived_from_exact_marginals() {
        for epsilon in [0.0, 0.1, 0.25, 0.49] {
            let example = parity_example(epsilon);
            assert_eq!(example.baseline_child_marginal, 0.5);
            assert_eq!(example.intervened_child_marginal, 0.5);
            assert_eq!(example.pass_count, 2);
            assert_eq!(
                example.invariant_deletions,
                vec!["P".to_string(), "T".into()]
            );
        }
    }

    #[test]
    fn tomography_cube_contains_eight_normalized_positive_laws() {
        let example = causal_tomography_chain();
        assert_eq!(example.laws.len(), 8);
        for law in example.laws {
            assert!(
                law.probabilities
                    .iter()
                    .all(|probability| *probability > 0.0)
            );
            assert!((law.probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-14);
        }
    }

    #[test]
    fn noncausal_cube_is_globally_normalized() {
        let cube = flat_noncausal_cube();
        for law in [cube.p0, cube.p10, cube.p01, cube.p11] {
            assert!(law.iter().all(|probability| *probability > 0.0));
            assert!((law.iter().sum::<f64>() - 1.0).abs() < 1e-14);
        }
    }
}
