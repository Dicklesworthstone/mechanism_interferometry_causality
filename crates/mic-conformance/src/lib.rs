#![forbid(unsafe_code)]
//! Cross-crate conformance tests. The crate intentionally exposes no runtime API.

#[cfg(test)]
mod tests {
    use mic_audit::{CertificateGates, CertificateStatus, EvidenceLedger, ExecutionMode, code};
    use mic_core::{DensitySquare, RatioSquare, covariance, four_law_moment};
    use mic_data::{DataSource, ExperimentManifest, InferenceTrack, RegimeSpec, SelectionContract};
    use mic_design::{
        DesignPoint, PeelingOutcome, audit_design, audit_sampling_odds, peel_families,
    };
    use mic_engine::{
        FourLawPolicy, LensEstimate, PreflightPolicy, PreflightStatus, SurveyAuthority,
        SurveyPolicy, audit_lens_battery, audit_orientation, run_preflight, run_tabular_audit,
        run_unsupervised_survey,
    };
    use mic_model::PosteriorSquare;
    use mic_sim::{
        causal_tomography_chain, exact_suite, flat_noncausal_cube, identification_twins,
    };
    use mic_stats::{
        CandidateSupport, OrientationOutcome, classify_deletion, parsimony_frontier,
        simultaneous_mean_bounds,
    };
    use std::collections::BTreeSet;
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
    fn exact_tomography_cube_recovers_chain_families_and_response_order() {
        let example = causal_tomography_chain();
        let law = |design: &str| {
            example
                .laws
                .iter()
                .find(|law| law.design == design)
                .expect("complete cube contains every design")
        };

        for first in 0..3 {
            for second in (first + 1)..3 {
                let background = 3 - first - second;
                for background_on in [false, true] {
                    let mut base = [false; 3];
                    base[background] = background_on;
                    let mut first_only = base;
                    first_only[first] = true;
                    let mut second_only = base;
                    second_only[second] = true;
                    let mut both = first_only;
                    both[second] = true;
                    let labels = [base, first_only, second_only, both].map(design_label);
                    for state in 0..8 {
                        let curvature = DensitySquare {
                            p0: law(&labels[0]).probabilities[state],
                            pa: law(&labels[1]).probabilities[state],
                            pb: law(&labels[2]).probabilities[state],
                            pab: law(&labels[3]).probabilities[state],
                        }
                        .curvature()
                        .unwrap();
                        assert!(curvature.abs() < 1e-13);
                    }
                }
            }
        }

        let families: Vec<BTreeSet<String>> = example
            .primitive_families
            .iter()
            .map(|family| family.iter().cloned().collect())
            .collect();
        let PeelingOutcome::Complete { families } = peel_families(&families).unwrap() else {
            panic!("one exact rich family per node must peel completely");
        };
        let recovered_targets: Vec<String> = families
            .iter()
            .map(|family| family.target.clone())
            .collect();
        assert_eq!(recovered_targets, example.primitive_targets);

        let baseline = law("000");
        for (design, expected) in [
            ("100", &example.response_sets[0]),
            ("010", &example.response_sets[1]),
            ("001", &example.response_sets[2]),
        ] {
            let shifted = law(design);
            let response: Vec<String> = ["A", "B", "C"]
                .iter()
                .enumerate()
                .filter(|(coordinate, _)| {
                    (binary_marginal(&shifted.probabilities, *coordinate)
                        - binary_marginal(&baseline.probabilities, *coordinate))
                    .abs()
                        > 1e-14
                })
                .map(|(_, name)| (*name).to_string())
                .collect();
            assert_eq!(&response, expected);
        }
    }

    #[test]
    fn flat_low_rank_cube_still_fails_causal_conditional_normalization() {
        let cube = flat_noncausal_cube();
        let independent_minor =
            cube.r1[0].ln() * cube.r2[1].ln() - cube.r1[1].ln() * cube.r2[0].ln();
        assert!(independent_minor.abs() > 1e-3);
        for state in 0..4 {
            let curvature = DensitySquare {
                p0: cube.p0[state],
                pa: cube.p10[state],
                pb: cube.p01[state],
                pab: cube.p11[state],
            }
            .curvature()
            .unwrap();
            assert!(curvature.abs() < 1e-14);
        }
        assert_eq!(conditional_ratio_means(cube.r1, 0), [1.25, 0.75]);
        assert_eq!(conditional_ratio_means(cube.r1, 1), [1.25, 0.75]);
        assert_eq!(conditional_ratio_means(cube.r2, 0), [1.25, 0.75]);
        assert_eq!(conditional_ratio_means(cube.r2, 1), [0.75, 1.25]);
    }

    #[test]
    fn natural_experiment_rows_do_not_identify_the_effect_sign() {
        let twins = identification_twins();
        for twin in [
            twins.regression_discontinuity,
            twins.instrumental_variable,
            twins.difference_in_differences,
        ] {
            assert_eq!(twin.premise_model_effect, 1.0);
            assert_eq!(twin.twin_model_effect, -1.0);
            assert!((twin.observed_masses.iter().sum::<f64>() - 1.0).abs() < 1e-14);
        }

        let rd = identification_twins().regression_discontinuity;
        for row in rd.observed_rows {
            let running = row[0];
            let treatment = row[1];
            let positive_model_outcome = treatment;
            let threshold = if running >= 0.0 { 1.0 } else { 0.0 };
            let negative_model_outcome = 2.0 * threshold - treatment;
            assert_eq!(positive_model_outcome, row[2]);
            assert_eq!(negative_model_outcome, row[2]);
        }

        let iv = identification_twins().instrumental_variable;
        for row in iv.observed_rows {
            let instrument = row[0];
            let treatment = row[1];
            assert_eq!(treatment, row[2]);
            assert_eq!(-treatment + 2.0 * instrument, row[2]);
        }

        let did = identification_twins().difference_in_differences;
        for row in did.observed_rows {
            let group = row[0];
            let post = row[1];
            let treatment = row[2];
            assert_eq!(treatment, row[3]);
            assert_eq!(2.0 * group * post - treatment, row[3]);
        }
    }

    fn design_label(bits: [bool; 3]) -> String {
        bits.into_iter()
            .map(|bit| if bit { '1' } else { '0' })
            .collect()
    }

    fn binary_marginal(law: &[f64; 8], coordinate: usize) -> f64 {
        let mask = 1_usize << (2 - coordinate);
        law.iter()
            .enumerate()
            .filter(|(state, _)| state & mask != 0)
            .map(|(_, probability)| probability)
            .sum()
    }

    fn conditional_ratio_means(ratio: [f64; 4], conditioning_coordinate: usize) -> [f64; 2] {
        let mask = 1_usize << (1 - conditioning_coordinate);
        std::array::from_fn(|value| {
            ratio
                .iter()
                .enumerate()
                .filter(|(state, _)| usize::from(state & mask != 0) == value)
                .map(|(_, ratio)| ratio)
                .sum::<f64>()
                / 2.0
        })
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
        assert_eq!(report.status(), PreflightStatus::Blocked);
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
                .findings()
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
                .findings()
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
        assert_eq!(report.preflight().status(), PreflightStatus::Ready);
        assert!(report.preflight().four_law_eligible());
        assert!(!report.preflight().product_factorial_eligible());
        assert_eq!(report.status(), CertificateStatus::Abstained);
        assert!(report.four_law()[0].max_abs_kappa > 0.8);
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
