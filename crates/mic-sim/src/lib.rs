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
        state_labels: (0_u8..8).map(|bits| format!("{:03b}", bits)).collect(),
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
}
