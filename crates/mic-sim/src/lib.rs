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
    /// Coordinates whose deletion is invariant.
    pub invariant_deletions: Vec<String>,
    /// Number of passing deletions.
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

/// Builds the balanced parity failure.
#[must_use]
pub fn parity_example(epsilon: f64) -> ParityExample {
    ParityExample {
        epsilon,
        support: vec!["P".into(), "T".into()],
        invariant_deletions: vec!["P".into(), "T".into()],
        pass_count: 2,
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
        curvature: -(1.0 - a * a).ln(),
        observed_ratio_covariance: -(a * a),
        hidden_conditional_covariance: a * a,
        locality_scope: "coordinated multi-source tilts; multiplicative flatness only".into(),
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
}
