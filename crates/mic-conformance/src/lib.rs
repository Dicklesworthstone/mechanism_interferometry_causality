#![forbid(unsafe_code)]
//! Cross-crate conformance tests. The crate intentionally exposes no runtime API.

#[cfg(test)]
mod tests {
    use mic_audit::{CertificateGates, CertificateStatus, EvidenceLedger, ExecutionMode, code};
    use mic_core::{RatioSquare, covariance, four_law_moment};
    use mic_data::{DataSource, ExperimentManifest, InferenceTrack, RegimeSpec, SelectionContract};
    use mic_design::{DesignPoint, audit_design, audit_sampling_odds};
    use mic_engine::{
        FourLawPolicy, LensEstimate, PreflightPolicy, PreflightStatus, SurveyAuthority,
        SurveyPolicy, audit_lens_battery, audit_orientation, run_preflight, run_tabular_audit,
        run_unsupervised_survey,
    };
    use mic_model::PosteriorSquare;
    use mic_sim::exact_suite;
    use mic_stats::{
        CandidateSupport, OrientationOutcome, classify_deletion, parsimony_frontier,
        simultaneous_mean_bounds,
    };
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
            disagreeing.status(&CertificateGates::unresolved()),
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
        assert_eq!(
            agreeing.status(&CertificateGates::unresolved()),
            mic_audit::CertificateStatus::Abstained,
            "lens agreement does not establish any population certificate gate"
        );
    }

    #[test]
    fn parity_fixture_flows_to_multiple_passes_abstention() {
        let parity = exact_suite().parity_orientation_failure;
        assert_eq!(parity.pass_count, 2);
        let epsilon = 0.05;
        let deletions: Vec<_> = parity
            .invariant_deletions
            .iter()
            .map(|variable| classify_deletion(variable.clone(), 0.0, 0.0, 0.01, epsilon).unwrap())
            .collect();
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let audit = audit_orientation(&deletions, 1.0, 0.1, "orientation", &mut ledger).unwrap();
        assert_eq!(
            audit.outcome,
            OrientationOutcome::MultiplePasses {
                passes: parity.invariant_deletions.clone()
            }
        );
        assert_eq!(
            ledger.status(&CertificateGates::unresolved()),
            CertificateStatus::Abstained
        );
        assert!(
            ledger
                .findings
                .iter()
                .any(|finding| finding.code == code::ORIENTATION_UNRESOLVED)
        );
    }

    #[test]
    fn simultaneous_bounds_certify_a_unique_target_end_to_end() {
        // Deletion discrepancy contributions per cluster: the target column sits
        // near zero, the parent column sits far above the tolerance.
        let contributions: Vec<Vec<f64>> = (0..24)
            .map(|index| {
                let jitter = 0.001 * f64::from(index % 5);
                vec![0.005 + jitter, 0.60 + jitter]
            })
            .collect();
        let bounds = simultaneous_mean_bounds(&contributions, 400, 0.95, 20_260_812).unwrap();
        let epsilon = 0.05;
        let deletions = vec![
            classify_deletion(
                "t",
                bounds.means[0],
                bounds.lower[0],
                bounds.upper[0],
                epsilon,
            )
            .unwrap(),
            classify_deletion(
                "p",
                bounds.means[1],
                bounds.lower[1],
                bounds.upper[1],
                epsilon,
            )
            .unwrap(),
        ];
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let audit = audit_orientation(&deletions, 0.8, 0.1, "orientation", &mut ledger).unwrap();
        assert_eq!(
            audit.outcome,
            OrientationOutcome::UniqueTarget { target: "t".into() }
        );
        assert!(!ledger.has_blocking_error());
        assert_eq!(
            ledger.status(&CertificateGates::unresolved()),
            CertificateStatus::Abstained,
            "unique orientation cannot launder missing locality, normalization, or flatness evidence"
        );
    }

    #[test]
    fn weak_interventions_abstain_rather_than_orient() {
        let deletions = vec![
            classify_deletion("t", 0.01, 0.0, 0.02, 0.05).unwrap(),
            classify_deletion("p", 0.9, 0.6, 1.2, 0.05).unwrap(),
        ];
        let mut ledger = EvidenceLedger::new(ExecutionMode::Strict);
        let audit = audit_orientation(&deletions, 0.02, 0.1, "orientation", &mut ledger).unwrap();
        assert_eq!(audit.outcome, OrientationOutcome::Underpowered);
        assert_eq!(
            ledger.status(&CertificateGates::unresolved()),
            CertificateStatus::Abstained
        );
    }

    #[test]
    fn histogram_four_law_is_ready_on_nonproduct_quotas_and_never_certifies() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let curved = ExperimentManifest::from_json_path(
            root.join("examples/configs/four_law_nonproduct.json"),
        )
        .unwrap();
        let report = run_tabular_audit(
            &curved,
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            Some(&root),
        )
        .unwrap();
        assert_eq!(report.preflight.status, PreflightStatus::Ready);
        assert!(report.preflight.four_law_eligible);
        assert!(!report.preflight.product_factorial_eligible);
        assert_eq!(report.status(), CertificateStatus::Abstained);
        assert!(report.four_law[0].max_abs_kappa > 0.8);
        let narrative = report.narrative();
        let markdown = narrative.markdown();
        assert!(markdown.starts_with("# Mechanism Interferometry report"));
        assert!(markdown.contains("## Certificate status: `abstained`"));
        assert!(
            markdown.find("## Abstentions").unwrap()
                < markdown.find("## Informational findings").unwrap()
        );
    }

    #[test]
    fn unsupervised_survey_is_proposal_only_and_finds_the_square() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let survey = run_unsupervised_survey(
            root.join("examples/data/four_law_discrete.csv"),
            Some(&root),
            Some("cluster_id"),
            SurveyPolicy::default(),
        )
        .unwrap();
        assert_eq!(survey.authority, SurveyAuthority::ProposalOnly);
        assert!(
            survey
                .interferometers
                .iter()
                .any(|item| item.complete_square)
        );
        assert!(!survey.wall.is_empty());
    }
}
