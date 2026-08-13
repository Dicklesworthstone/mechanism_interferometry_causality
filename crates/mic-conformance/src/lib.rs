#![forbid(unsafe_code)]
//! Cross-crate conformance tests. The crate intentionally exposes no runtime API.

#[cfg(test)]
mod tests {
    use mic_audit::{CertificateGates, CertificateStatus, EvidenceLedger, ExecutionMode, code};
    use mic_core::{DensitySquare, RatioSquare, covariance, four_law_moment};
    use mic_data::{
        DataSource, ExperimentManifest, InferenceTrack, RegimeSpec, SelectionContract,
        fold_for_cluster,
    };
    use mic_design::{
        DesignPoint, PeelingOutcome, audit_design, audit_sampling_odds, peel_families,
    };
    use mic_engine::{
        FourLawPolicy, LensEstimate, PreflightPolicy, PreflightStatus, SurveyAuthority,
        SurveyPolicy, audit_lens_battery, audit_orientation, run_preflight, run_tabular_audit,
        run_unsupervised_survey,
    };
    use mic_model::{
        ClosureCrossFitConfig, ClosureFitConfig, ClosureModelKind, ClusteredMultinomialSample,
        CombinationConfirmationSample, CompletionFailure, CompletionStatus,
        FiniteCompletionAuthority, FiniteCompletionInput, FiniteLawSemantics,
        FiniteMechanismFamily, FiniteObservedRegime, FitConfig, FourCornerClosureModel,
        FrozenFeatureContract, MultinomialSample, PosteriorSquare, PrimitiveArm,
        PrimitiveTransportConfig, PrimitiveTransportSample, compare_held_out_closure_models,
        cross_fit_closure_models, freeze_primitive_transport, predict_combination_law,
        score_combination_confirmation, solve_finite_modular_completion,
    };
    use mic_sim::{
        HiddenSensorTomography, causal_tomography_chain, exact_suite, flat_noncausal_cube,
        hidden_sensor_tomography, identification_twins,
    };
    use mic_stats::{
        CandidateSupport, OrientationOutcome, classify_deletion, parsimony_frontier,
        simultaneous_mean_bounds,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn clustered_hidden_sensor_samples(
        exact_counts: [[usize; 2]; 4],
        seed: u64,
        n_folds: usize,
    ) -> Vec<ClusteredMultinomialSample> {
        let mut clustered = Vec::new();
        for (class, counts) in exact_counts.into_iter().enumerate() {
            for fold in 0..n_folds {
                let cluster_id = (0_u64..10_000)
                    .map(|candidate| format!("hidden-{class}-{fold}-{candidate}"))
                    .find(|candidate| fold_for_cluster(seed, candidate, n_folds) == Some(fold))
                    .expect("10,000 deterministic candidates cover each of two folds");
                for (state, count) in counts.into_iter().enumerate() {
                    let y = if state == 0 { -1.0 } else { 1.0 };
                    clustered.extend((0..count).map(|_| ClusteredMultinomialSample {
                        features: vec![y],
                        class,
                        cluster_id: cluster_id.clone(),
                    }));
                }
            }
        }
        clustered
    }

    fn assert_hidden_sensor_law_prediction(example: &HiddenSensorTomography) {
        let complete_prediction = predict_combination_law(
            &example.laws[0].complete_probabilities,
            &example.laws[1].complete_probabilities,
            &example.laws[2].complete_probabilities,
            &example.laws[3].complete_probabilities,
            40,
        )
        .unwrap();
        assert!(complete_prediction.heldout_total_variation < 1e-14);
        let observed_prediction = predict_combination_law(
            &example.laws[0].observed_probabilities,
            &example.laws[1].observed_probabilities,
            &example.laws[2].observed_probabilities,
            &example.laws[3].observed_probabilities,
            40,
        )
        .unwrap();
        assert!(observed_prediction.normalizer_residual.abs() < 1e-14);
        assert!((observed_prediction.heldout_total_variation - 0.1).abs() < 1e-14);
    }

    fn fitted_tomography_samples(
        example: &HiddenSensorTomography,
        complete_state: bool,
    ) -> (
        Vec<PrimitiveTransportSample>,
        Vec<CombinationConfirmationSample>,
    ) {
        let complete_counts = [
            [10, 10, 10, 10],
            [6, 6, 14, 14],
            [5, 15, 5, 15],
            [3, 9, 7, 21],
        ];
        let observed_counts = [[20, 20], [20, 20], [20, 20], [16, 24]];
        let primitive_arms = [
            PrimitiveArm::Baseline,
            PrimitiveArm::First,
            PrimitiveArm::Second,
        ];
        let mut primitive = Vec::new();
        let mut confirmation = Vec::new();
        for cluster in 0..2 {
            for (law, arm) in example.laws[..3].iter().zip(primitive_arms) {
                if complete_state {
                    for (state, count) in complete_counts[arm as usize].into_iter().enumerate() {
                        let features = match state {
                            0 => vec![-1.0, -1.0],
                            1 => vec![-1.0, 1.0],
                            2 => vec![1.0, -1.0],
                            _ => vec![1.0, 1.0],
                        };
                        primitive.extend((0..count).map(|_| PrimitiveTransportSample {
                            features: features.clone(),
                            arm,
                            cluster_id: format!("complete-{arm:?}-{cluster}"),
                        }));
                    }
                } else {
                    for (state, count) in observed_counts[arm as usize].into_iter().enumerate() {
                        let features = vec![if state == 0 { -1.0 } else { 1.0 }];
                        primitive.extend((0..count).map(|_| PrimitiveTransportSample {
                            features: features.clone(),
                            arm,
                            cluster_id: format!("observed-{arm:?}-{cluster}"),
                        }));
                    }
                }
                assert!(!law.complete_probabilities.is_empty());
            }
            if complete_state {
                for (state, count) in complete_counts[3].into_iter().enumerate() {
                    let features = match state {
                        0 => vec![-1.0, -1.0],
                        1 => vec![-1.0, 1.0],
                        2 => vec![1.0, -1.0],
                        _ => vec![1.0, 1.0],
                    };
                    confirmation.extend((0..count).map(|_| CombinationConfirmationSample {
                        features: features.clone(),
                        cluster_id: format!("complete-combination-{cluster}"),
                    }));
                }
            } else {
                for (state, count) in observed_counts[3].into_iter().enumerate() {
                    let features = vec![if state == 0 { -1.0 } else { 1.0 }];
                    confirmation.extend((0..count).map(|_| CombinationConfirmationSample {
                        features: features.clone(),
                        cluster_id: format!("observed-combination-{cluster}"),
                    }));
                }
            }
        }
        (primitive, confirmation)
    }

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
    fn hidden_sensor_world_links_exact_curvature_to_joint_model_diagnostic() {
        let example = hidden_sensor_tomography();
        for state in 0..4 {
            let complete_curvature = DensitySquare {
                p0: example.laws[0].complete_probabilities[state],
                pa: example.laws[1].complete_probabilities[state],
                pb: example.laws[2].complete_probabilities[state],
                pab: example.laws[3].complete_probabilities[state],
            }
            .curvature()
            .unwrap();
            assert!(complete_curvature.abs() < 1e-14);
        }
        for state in 0..2 {
            let observed_curvature = DensitySquare {
                p0: example.laws[0].observed_probabilities[state],
                pa: example.laws[1].observed_probabilities[state],
                pb: example.laws[2].observed_probabilities[state],
                pab: example.laws[3].observed_probabilities[state],
            }
            .curvature()
            .unwrap();
            assert!((observed_curvature - example.observed_curvature[state]).abs() < 1e-14);
        }
        assert_hidden_sensor_law_prediction(&example);

        let mut samples = Vec::new();
        let exact_counts = [[5_usize, 5], [5, 5], [5, 5], [4, 6]];
        let exact_masses = [[0.5, 0.5], [0.5, 0.5], [0.5, 0.5], [0.4, 0.6]];
        for (class, ((law, counts), masses)) in example
            .laws
            .iter()
            .zip(exact_counts)
            .zip(exact_masses)
            .enumerate()
        {
            for (state, count) in counts.into_iter().enumerate() {
                assert!((law.observed_probabilities[state] - masses[state]).abs() < 1e-14);
                let y = if state == 0 { -1.0 } else { 1.0 };
                samples.extend((0..count).map(|_| MultinomialSample {
                    features: vec![y],
                    class,
                }));
            }
        }
        let config = ClosureFitConfig {
            l2_penalty: 0.1,
            max_iterations: 5_000,
            gradient_tolerance: 1e-6,
            ..ClosureFitConfig::default()
        };
        let restricted = FourCornerClosureModel::fit(
            &samples,
            [0.25; 4],
            ClosureModelKind::MainEffectsOnly,
            config,
        )
        .unwrap();
        let saturated = FourCornerClosureModel::fit(
            &samples,
            [0.25; 4],
            ClosureModelKind::MainEffectsPlusInteraction,
            config,
        )
        .unwrap();
        let comparison =
            compare_held_out_closure_models(&restricted, &saturated, &samples).unwrap();
        assert!(comparison.saturated_advantage > 0.0);
        assert!(!comparison.calibrated_test);
        assert!(saturated.curvature_field(&[-1.0]).unwrap() < 0.0);
        assert!(saturated.curvature_field(&[1.0]).unwrap() > 0.0);

        let seed = 41;
        let n_folds = 2;
        let clustered = clustered_hidden_sensor_samples(exact_counts, seed, n_folds);
        let cross_fitted = cross_fit_closure_models(
            &clustered,
            [0.25; 4],
            ClosureCrossFitConfig {
                seed,
                n_folds,
                fit: config,
            },
        )
        .unwrap();
        assert_eq!(cross_fitted.seed, seed);
        assert_eq!(cross_fitted.n_clusters, 8);
        assert!(cross_fitted.saturated_advantage > 0.0);
        assert!(!cross_fitted.calibrated_test);
    }

    #[test]
    fn fitted_combination_prediction_exposes_hidden_state_but_never_certifies() {
        let example = hidden_sensor_tomography();
        let feature_contract = FrozenFeatureContract {
            feature_schema_sha256: "a".repeat(64),
            feature_transform_sha256: "b".repeat(64),
        };
        let config = PrimitiveTransportConfig {
            seed: 41,
            n_folds: 2,
            fit: FitConfig {
                n_classes: 3,
                l2_penalty: 0.1,
                max_iterations: 5_000,
                gradient_tolerance: 1e-6,
                initial_step: 1.0,
            },
        };

        let (complete_primitive, complete_confirmation) = fitted_tomography_samples(&example, true);
        let complete = freeze_primitive_transport(
            &complete_primitive,
            [1.0 / 3.0; 3],
            "experimental_run",
            feature_contract.clone(),
            config,
        )
        .unwrap();
        let complete_report = score_combination_confirmation(
            &complete,
            "experimental_run",
            &feature_contract,
            &complete_confirmation,
        )
        .unwrap();
        assert!(
            (complete.receipt().raw_normalizer() - 1.0).abs() < 0.05,
            "raw normalizer was {}",
            complete.receipt().raw_normalizer()
        );
        assert!(complete_report.heldout_proper_score_gain() > 0.01);

        let (observed_primitive, observed_confirmation) =
            fitted_tomography_samples(&example, false);
        let observed = freeze_primitive_transport(
            &observed_primitive,
            [1.0 / 3.0; 3],
            "experimental_run",
            feature_contract.clone(),
            config,
        )
        .unwrap();
        let observed_report = score_combination_confirmation(
            &observed,
            "experimental_run",
            &feature_contract,
            &observed_confirmation,
        )
        .unwrap();
        assert!((observed.receipt().raw_normalizer() - 1.0).abs() < 1e-8);
        assert!(observed_report.heldout_proper_score_gain().abs() < 1e-8);
        let json = serde_json::to_value(observed_report).unwrap();
        assert_eq!(json["authority"], "diagnostic_only");
        assert_eq!(json["certificate_eligible"], false);
        assert_eq!(json["contracts"]["independent_unit"], "unverified");
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
    fn flat_noncausal_cube_fails_fixed_model_potential_checks() {
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
        let report = solve_finite_modular_completion(&FiniteCompletionInput {
            law_semantics: FiniteLawSemantics::ExactOrSimulatedPopulation,
            state_cardinalities: vec![2, 2],
            states: vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]],
            baseline_probabilities: cube.p0.to_vec(),
            parents_by_node: vec![vec![], vec![0]],
            families: vec![
                FiniteMechanismFamily {
                    cardinality: 2,
                    target: 0,
                },
                FiniteMechanismFamily {
                    cardinality: 2,
                    target: 1,
                },
            ],
            regimes: vec![
                FiniteObservedRegime {
                    levels: vec![1, 0],
                    probabilities: cube.p10.to_vec(),
                },
                FiniteObservedRegime {
                    levels: vec![0, 1],
                    probabilities: cube.p01.to_vec(),
                },
            ],
            tolerance: 1e-12,
        })
        .unwrap();
        assert_eq!(report.status(), CompletionStatus::Infeasible);
        assert_eq!(report.failure(), Some(CompletionFailure::NonlocalPotential));
        assert_eq!(report.algebraic_rank(), 2);
        assert_eq!(report.lack_of_fit_dimension(), 0);
        assert!(!report.additive_lack_of_fit_testable());
        assert!(report.causal_potentials_evaluated());
        assert!(report.potentials().is_empty());
        assert_eq!(
            report.authority(),
            FiniteCompletionAuthority::DiagnosticOnly
        );
        assert!(!report.certificate_eligible());
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
