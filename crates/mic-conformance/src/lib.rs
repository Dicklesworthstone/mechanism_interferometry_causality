#![forbid(unsafe_code)]
//! Cross-crate conformance tests. The crate intentionally exposes no runtime API.

#[cfg(test)]
mod tests {
    use mic_audit::{EvidenceLedger, ExecutionMode, code};
    use mic_core::{RatioSquare, covariance, four_law_moment};
    use mic_data::{DataSource, ExperimentManifest, InferenceTrack, RegimeSpec, SelectionContract};
    use mic_design::{DesignPoint, audit_design, audit_sampling_odds};
    use mic_engine::{
        LensEstimate, PreflightPolicy, PreflightStatus, audit_lens_battery, run_preflight,
    };
    use mic_model::PosteriorSquare;
    use mic_sim::exact_suite;
    use mic_stats::{CandidateSupport, parsimony_frontier};
    use std::path::PathBuf;

    #[test]
    fn running_example_demonstrates_scalar_blind_spot() {
        let suite = exact_suite();
        assert_eq!(suite.running_example.scalar_moment_battery, 1.0);
        assert!(suite.running_example.curvature_at(1.0).abs() > 1e-3);
        let witness = [-1.0, 1.0];
        let ones = [1.0, 1.0];
        let rab = [0.8, 1.2];
        assert!(four_law_moment(&witness, &ones, &ones, &rab).unwrap() > 0.0);
        assert!(four_law_moment(&ones, &ones, &ones, &rab).unwrap().abs() < 1e-14);
    }

    #[test]
    fn latent_fixture_satisfies_three_way_conservation() {
        let example = exact_suite().latent_conservation;
        let observed = covariance(&example.ra, &example.rb).unwrap();
        assert!((observed - example.observed_ratio_covariance).abs() < 1e-14);
        assert!((observed + example.hidden_conditional_covariance).abs() < 1e-14);
        for index in 0..2 {
            let square = RatioSquare {
                ra: example.ra[index],
                rb: example.rb[index],
                rab: example.rab[index],
            };
            assert!((square.curvature().unwrap() - example.curvature).abs() < 1e-14);
        }
    }

    #[test]
    fn posterior_odds_need_sampling_anchor() {
        let rho = [0.1, 0.2, 0.3, 0.4];
        assert!(!audit_sampling_odds(rho, 1e-12).unwrap().is_product);
        let posterior = PosteriorSquare { q: rho };
        assert!(posterior.density_curvature(rho).unwrap().abs() < 1e-14);
    }

    #[test]
    fn six_corner_design_retains_nonface_flatness_content() {
        let points: Vec<_> = ["001", "010", "011", "100", "101", "110"]
            .iter()
            .map(|label| DesignPoint::parse(label).unwrap())
            .collect();
        let audit = audit_design(&points, 1e-12).unwrap();
        assert_eq!(audit.lack_of_fit_dimension, 2);
        assert_eq!(audit.lack_of_fit_basis.len(), 2);
        assert!(audit.square_faces.is_empty());
        assert!(!audit.squares_span_lack_of_fit);
    }

    #[test]
    fn preflight_blocks_nonproduct_gcm_sampling() {
        let labels = ["00", "10", "01", "11"];
        let probabilities = [0.1, 0.2, 0.3, 0.4];
        let manifest = ExperimentManifest {
            schema_version: "1.0.0".into(),
            experiment_id: "nonproduct".into(),
            strict: true,
            inference_track: InferenceTrack::Both,
            selection: SelectionContract::StateIndependentWithinRegime,
            cluster_column: "cluster".into(),
            regime_column: "regime".into(),
            state_columns: vec!["x".into()],
            candidate_state_blocks: Vec::new(),
            regimes: labels
                .iter()
                .zip(probabilities)
                .map(|(label, sampling_proportion)| RegimeSpec {
                    id: (*label).into(),
                    design: DesignPoint::parse(label).unwrap(),
                    sampling_proportion,
                    perturbations: Vec::new(),
                })
                .collect(),
            data: DataSource {
                format: "synthetic".into(),
                path: PathBuf::from("none"),
            },
            seed: 9,
        };
        let report = run_preflight(&manifest, PreflightPolicy::default()).unwrap();
        assert_eq!(report.status, PreflightStatus::Blocked);
    }

    #[test]
    fn parsimony_frontier_recovers_minimal_local_support() {
        let candidates = vec![
            CandidateSupport {
                variables: vec!["target".into(), "parent".into(), "bystander".into()],
                holdout_loss: 0.401,
                complexity: 3.0,
            },
            CandidateSupport {
                variables: vec!["target".into(), "parent".into()],
                holdout_loss: 0.400,
                complexity: 2.0,
            },
            CandidateSupport {
                variables: vec!["parent".into(), "bystander".into()],
                holdout_loss: 0.690,
                complexity: 2.0,
            },
            CandidateSupport {
                variables: vec!["bystander".into()],
                holdout_loss: 0.693,
                complexity: 1.0,
            },
        ];
        let frontier = parsimony_frontier(&candidates, 0.01).unwrap();
        assert_eq!(
            frontier.minimal_support,
            vec!["parent".to_string(), "target".into()]
        );
        assert_eq!(frontier.inclusion_frequencies["target"], 1.0);
        assert_eq!(frontier.inclusion_frequencies["parent"], 1.0);
        assert!(frontier.inclusion_frequencies["bystander"] < 1.0);
    }

    #[test]
    fn lens_disagreement_forces_strict_abstention() {
        let policy = PreflightPolicy::default();
        let mut disagreeing = EvidenceLedger::new(ExecutionMode::Strict);
        let audit = audit_lens_battery(
            &[
                LensEstimate {
                    family: "linear".into(),
                    estimate: 0.02,
                    standard_error: 0.01,
                },
                LensEstimate {
                    family: "kernel".into(),
                    estimate: 0.75,
                    standard_error: 0.01,
                },
            ],
            &policy,
            "curvature",
            &mut disagreeing,
        )
        .unwrap();
        assert!(!audit.agrees);
        assert_eq!(
            disagreeing.status(true),
            mic_audit::CertificateStatus::Abstained,
            "learner-dependent projections must not certify"
        );
        assert!(
            disagreeing
                .findings
                .iter()
                .any(|finding| finding.code == code::ESTIMATOR_FAMILY_DISAGREEMENT)
        );

        let mut agreeing = EvidenceLedger::new(ExecutionMode::Strict);
        let audit = audit_lens_battery(
            &[
                LensEstimate {
                    family: "linear".into(),
                    estimate: 0.020,
                    standard_error: 0.010,
                },
                LensEstimate {
                    family: "kernel".into(),
                    estimate: 0.025,
                    standard_error: 0.010,
                },
            ],
            &policy,
            "curvature",
            &mut agreeing,
        )
        .unwrap();
        assert!(audit.agrees);
        assert_eq!(agreeing.status(true), mic_audit::CertificateStatus::Passed);
    }
}
