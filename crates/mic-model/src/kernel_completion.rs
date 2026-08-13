#![forbid(unsafe_code)]
//! Exact conditional-kernel completion for a fixed finite DAG and distinct targets.
//!
//! This solver is deliberately separate from treatment-design rank.  A supplied
//! positive baseline fixes every level-zero kernel.  Observed positive regime laws
//! identify their DAG conditional kernels; compatibility is then exactly inactive-
//! kernel equality plus repeated family-level equality across backgrounds.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::finite_completion::{
    FiniteCompletionAuthority, FiniteCompletionError, FiniteCompletionInput, FiniteLawSemantics,
    factorizes_over_dag, fingerprint_input, validate_input,
};

const MAX_SERIALIZED_COMPLETION_GRID: usize = 4_096;
type ObservedKernels = BTreeMap<(usize, u32), (usize, Vec<f64>)>;

/// Model-relative status of the exact fixed-DAG kernel fiber.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KernelCompletionStatus {
    /// At least one exact kernel compatibility clause failed.
    ExactInfeasible,
    /// Every nonbaseline family level was observed and every missing law is determined.
    PointIdentified,
    /// At least one family level was never observed and retains a free supported kernel.
    SetIdentified,
}

/// Whether the bounded full-grid completion was serialized.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompletionGridSerialization {
    /// Missing laws were serialized when the kernel dictionary was point identified.
    Included,
    /// The categorical grid exceeded the fixed report-size budget.
    OmittedByBudget,
    /// Free unobserved level kernels prevent a unique grid serialization.
    NotApplicableSetIdentified,
    /// Incompatible observed kernels prevent any modular grid serialization.
    NotApplicableInfeasible,
}

/// Fixed authority scope of the solver.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KernelCompletionScope {
    /// Conclusions are relative to the caller-supplied DAG and target map.
    FixedDagAndTargets,
}

/// Causal identity authority of graph and target labels.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityAuthority {
    /// The solver did not establish the physical identity.
    NotEstablished,
}

/// Certificate eligibility of this exact-table diagnostic.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KernelCertificateEligibility {
    /// Exact fixed-model compatibility is not an empirical causal certificate.
    Ineligible,
}

/// Exact incompatibility retained by the kernel solver.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KernelCompletionFinding {
    /// Supplied baseline does not factorize over the fixed DAG.
    BaselineNotMarkov,
    /// One observed regime does not factorize over the fixed DAG.
    RegimeNotMarkov {
        /// Index of the incompatible observed regime.
        regime_index: usize,
    },
    /// A node with no active targeting family changed from its baseline kernel.
    InactiveKernelChanged {
        /// Index of the incompatible observed regime.
        regime_index: usize,
        /// Node whose inactive conditional changed.
        node: usize,
    },
    /// One family-level exposed different target kernels in two backgrounds.
    RepeatedLevelKernelMismatch {
        /// Mechanism-family index.
        family: usize,
        /// Repeated nonbaseline family level.
        level: u32,
        /// First regime exposing the reference kernel.
        first_regime_index: usize,
        /// Later background exposing a different kernel.
        second_regime_index: usize,
    },
}

/// One observed family-level kernel, aligned to the declared full state order.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IdentifiedConditionalKernel {
    family: usize,
    level: u32,
    conditional_probability_by_state: Vec<f64>,
}

/// One exactly completed missing factorial law.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CompletedRegimeLaw {
    levels: Vec<u32>,
    probabilities: Vec<f64>,
}

/// Serialize-only exact kernel-completion report.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelCompletionReport {
    status: KernelCompletionStatus,
    findings: Vec<KernelCompletionFinding>,
    identified_kernels: Vec<IdentifiedConditionalKernel>,
    unobserved_family_levels: Vec<(usize, u32)>,
    completed_missing_laws: Vec<CompletedRegimeLaw>,
    completion_grid_serialization: CompletionGridSerialization,
    model_input_sha256: String,
    law_semantics: FiniteLawSemantics,
    scope: KernelCompletionScope,
    graph_identity: IdentityAuthority,
    target_identity: IdentityAuthority,
    authority: FiniteCompletionAuthority,
    certificate_eligibility: KernelCertificateEligibility,
}

impl KernelCompletionReport {
    /// Exact model-relative kernel-fiber status.
    #[must_use]
    pub fn status(&self) -> KernelCompletionStatus {
        self.status
    }

    /// Every evaluable exact incompatibility, not only the first.
    #[must_use]
    pub fn findings(&self) -> &[KernelCompletionFinding] {
        &self.findings
    }

    /// Compatible observed family-level kernels. Empty when the fixed model is infeasible.
    #[must_use]
    pub fn identified_kernels(&self) -> &[IdentifiedConditionalKernel] {
        &self.identified_kernels
    }

    /// Why the completed grid is present or deliberately absent.
    #[must_use]
    pub fn completion_grid_serialization(&self) -> CompletionGridSerialization {
        self.completion_grid_serialization
    }

    /// Family levels absent from every supplied regime.
    #[must_use]
    pub fn unobserved_family_levels(&self) -> &[(usize, u32)] {
        &self.unobserved_family_levels
    }

    /// Exactly completed missing laws when the full kernel dictionary is identified.
    #[must_use]
    pub fn completed_missing_laws(&self) -> &[CompletedRegimeLaw] {
        &self.completed_missing_laws
    }
}

/// Solves exact partial-design completion in conditional-kernel coordinates.
///
/// The conclusion is conditional on the supplied positive baseline, fixed DAG,
/// and distinct target assignment.  It does not identify the graph, physical
/// intervention semantics, selection mechanism, or targets from rows.
pub fn solve_finite_kernel_completion(
    input: &FiniteCompletionInput,
) -> Result<KernelCompletionReport, FiniteCompletionError> {
    validate_input(input)?;
    if input.law_semantics == FiniteLawSemantics::EstimatedPointTables {
        return Err(FiniteCompletionError::EstimatedTablesRequireUncertaintyModel);
    }

    let mut findings = Vec::new();
    if !factorizes_over_dag(
        &input.baseline_probabilities,
        &input.states,
        &input.parents_by_node,
        input.tolerance,
    ) {
        findings.push(KernelCompletionFinding::BaselineNotMarkov);
    }

    let baseline_kernels = (0..input.parents_by_node.len())
        .map(|node| conditional_kernel(input, &input.baseline_probabilities, node))
        .collect::<Vec<_>>();
    let target_to_family = input
        .families
        .iter()
        .enumerate()
        .map(|(family, spec)| (spec.target, family))
        .collect::<BTreeMap<_, _>>();
    let (mut regime_findings, observed) =
        audit_regime_kernels(input, &baseline_kernels, &target_to_family);
    findings.append(&mut regime_findings);

    findings.sort();
    findings.dedup();
    let all_levels = input
        .families
        .iter()
        .enumerate()
        .flat_map(|(family, spec)| (1..spec.cardinality).map(move |level| (family, level)))
        .collect::<Vec<_>>();
    let unobserved_family_levels = all_levels
        .iter()
        .copied()
        .filter(|key| !observed.contains_key(key))
        .collect::<Vec<_>>();
    let status = if findings.is_empty() {
        if unobserved_family_levels.is_empty() {
            KernelCompletionStatus::PointIdentified
        } else {
            KernelCompletionStatus::SetIdentified
        }
    } else {
        KernelCompletionStatus::ExactInfeasible
    };
    let identified_kernels = if status == KernelCompletionStatus::ExactInfeasible {
        Vec::new()
    } else {
        observed
            .iter()
            .map(
                |(&(family, level), (_, values))| IdentifiedConditionalKernel {
                    family,
                    level,
                    conditional_probability_by_state: values.clone(),
                },
            )
            .collect::<Vec<_>>()
    };

    let grid_size = input.families.iter().try_fold(1usize, |size, family| {
        usize::try_from(family.cardinality)
            .ok()
            .and_then(|cardinality| size.checked_mul(cardinality))
    });
    let completion_grid_omitted_by_budget =
        grid_size.is_none_or(|size| size > MAX_SERIALIZED_COMPLETION_GRID);
    let completed_missing_laws = if status == KernelCompletionStatus::PointIdentified
        && !completion_grid_omitted_by_budget
    {
        complete_missing_laws(input, &baseline_kernels, &observed)
    } else {
        Vec::new()
    };

    Ok(KernelCompletionReport {
        status,
        findings,
        identified_kernels,
        unobserved_family_levels,
        completed_missing_laws,
        completion_grid_serialization: completion_grid_serialization(
            status,
            completion_grid_omitted_by_budget,
        ),
        model_input_sha256: fingerprint_input(input),
        law_semantics: input.law_semantics,
        scope: KernelCompletionScope::FixedDagAndTargets,
        graph_identity: IdentityAuthority::NotEstablished,
        target_identity: IdentityAuthority::NotEstablished,
        authority: FiniteCompletionAuthority::DiagnosticOnly,
        certificate_eligibility: KernelCertificateEligibility::Ineligible,
    })
}

fn completion_grid_serialization(
    status: KernelCompletionStatus,
    omitted_by_budget: bool,
) -> CompletionGridSerialization {
    match status {
        KernelCompletionStatus::ExactInfeasible => {
            CompletionGridSerialization::NotApplicableInfeasible
        }
        KernelCompletionStatus::SetIdentified => {
            CompletionGridSerialization::NotApplicableSetIdentified
        }
        KernelCompletionStatus::PointIdentified if omitted_by_budget => {
            CompletionGridSerialization::OmittedByBudget
        }
        KernelCompletionStatus::PointIdentified => CompletionGridSerialization::Included,
    }
}

fn audit_regime_kernels(
    input: &FiniteCompletionInput,
    baseline_kernels: &[Vec<f64>],
    target_to_family: &BTreeMap<usize, usize>,
) -> (Vec<KernelCompletionFinding>, ObservedKernels) {
    let mut findings = Vec::new();
    let mut observed = ObservedKernels::new();
    for (regime_index, regime) in input.regimes.iter().enumerate() {
        if !factorizes_over_dag(
            &regime.probabilities,
            &input.states,
            &input.parents_by_node,
            input.tolerance,
        ) {
            findings.push(KernelCompletionFinding::RegimeNotMarkov { regime_index });
        }
        for (node, baseline_kernel) in baseline_kernels.iter().enumerate() {
            let kernel = conditional_kernel(input, &regime.probabilities, node);
            let active = target_to_family.get(&node).copied().and_then(|family| {
                (regime.levels[family] != 0).then_some((family, regime.levels[family]))
            });
            let Some(key) = active else {
                if !vectors_equal(&kernel, baseline_kernel, input.tolerance) {
                    findings.push(KernelCompletionFinding::InactiveKernelChanged {
                        regime_index,
                        node,
                    });
                }
                continue;
            };
            if let Some((first_regime_index, first)) = observed.get(&key) {
                if !vectors_equal(&kernel, first, input.tolerance) {
                    findings.push(KernelCompletionFinding::RepeatedLevelKernelMismatch {
                        family: key.0,
                        level: key.1,
                        first_regime_index: *first_regime_index,
                        second_regime_index: regime_index,
                    });
                }
            } else {
                observed.insert(key, (regime_index, kernel));
            }
        }
    }
    (findings, observed)
}

fn conditional_kernel(input: &FiniteCompletionInput, law: &[f64], node: usize) -> Vec<f64> {
    let parents = &input.parents_by_node[node];
    input
        .states
        .iter()
        .map(|state| {
            let denominator = input
                .states
                .iter()
                .zip(law)
                .filter(|(candidate, _)| same_coordinates(candidate, state, parents))
                .map(|(_, probability)| probability)
                .sum::<f64>();
            let numerator = input
                .states
                .iter()
                .zip(law)
                .filter(|(candidate, _)| {
                    candidate[node] == state[node] && same_coordinates(candidate, state, parents)
                })
                .map(|(_, probability)| probability)
                .sum::<f64>();
            numerator / denominator
        })
        .collect()
}

fn same_coordinates(left: &[u32], right: &[u32], coordinates: &[usize]) -> bool {
    coordinates
        .iter()
        .all(|coordinate| left[*coordinate] == right[*coordinate])
}

fn vectors_equal(left: &[f64], right: &[f64], tolerance: f64) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| (left - right).abs() <= tolerance)
}

fn complete_missing_laws(
    input: &FiniteCompletionInput,
    baseline_kernels: &[Vec<f64>],
    observed: &BTreeMap<(usize, u32), (usize, Vec<f64>)>,
) -> Vec<CompletedRegimeLaw> {
    let observed_codes = input
        .regimes
        .iter()
        .map(|regime| regime.levels.clone())
        .collect::<BTreeSet<_>>();
    let target_to_family = input
        .families
        .iter()
        .enumerate()
        .map(|(family, spec)| (spec.target, family))
        .collect::<BTreeMap<_, _>>();
    let mut codes = Vec::new();
    enumerate_codes(&input.families, 0, &mut Vec::new(), &mut codes);
    codes
        .into_iter()
        .filter(|levels| levels.iter().any(|level| *level != 0))
        .filter(|levels| !observed_codes.contains(levels))
        .map(|levels| {
            let mut probabilities = input
                .states
                .iter()
                .enumerate()
                .map(|(state_index, _)| {
                    (0..input.parents_by_node.len())
                        .map(|node| {
                            target_to_family.get(&node).map_or_else(
                                || baseline_kernels[node][state_index],
                                |family| {
                                    let level = levels[*family];
                                    if level == 0 {
                                        baseline_kernels[node][state_index]
                                    } else {
                                        observed[&(*family, level)].1[state_index]
                                    }
                                },
                            )
                        })
                        .product::<f64>()
                })
                .collect::<Vec<_>>();
            let total = probabilities.iter().sum::<f64>();
            for value in &mut probabilities {
                *value /= total;
            }
            CompletedRegimeLaw {
                levels,
                probabilities,
            }
        })
        .collect()
}

fn enumerate_codes(
    families: &[crate::FiniteMechanismFamily],
    family: usize,
    prefix: &mut Vec<u32>,
    output: &mut Vec<Vec<u32>>,
) {
    if family == families.len() {
        output.push(prefix.clone());
        return;
    }
    for level in 0..families[family].cardinality {
        prefix.push(level);
        enumerate_codes(families, family + 1, prefix, output);
        prefix.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FiniteMechanismFamily, FiniteObservedRegime};

    fn input(regimes: Vec<FiniteObservedRegime>) -> FiniteCompletionInput {
        FiniteCompletionInput {
            law_semantics: FiniteLawSemantics::ExactOrSimulatedPopulation,
            state_cardinalities: vec![2, 2],
            states: vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]],
            baseline_probabilities: vec![0.25; 4],
            parents_by_node: vec![vec![], vec![]],
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
            regimes,
            tolerance: 1e-10,
        }
    }

    #[test]
    fn product_diagonal_is_point_identified_despite_rank_deficiency() {
        let report = solve_finite_kernel_completion(&input(vec![FiniteObservedRegime {
            levels: vec![1, 1],
            probabilities: vec![0.12, 0.28, 0.18, 0.42],
        }]))
        .unwrap();
        assert_eq!(report.status(), KernelCompletionStatus::PointIdentified);
        assert!(report.unobserved_family_levels().is_empty());
        assert_eq!(report.completed_missing_laws().len(), 2);
    }

    #[test]
    fn unseen_level_is_set_identified() {
        let report = solve_finite_kernel_completion(&input(vec![FiniteObservedRegime {
            levels: vec![1, 0],
            probabilities: vec![0.125, 0.125, 0.375, 0.375],
        }]))
        .unwrap();
        assert_eq!(report.status(), KernelCompletionStatus::SetIdentified);
        assert_eq!(report.unobserved_family_levels(), &[(1, 1)]);
        assert_eq!(
            report.completion_grid_serialization(),
            CompletionGridSerialization::NotApplicableSetIdentified
        );
    }

    #[test]
    fn changed_inactive_kernel_is_infeasible() {
        let report = solve_finite_kernel_completion(&input(vec![FiniteObservedRegime {
            levels: vec![1, 0],
            probabilities: vec![0.1, 0.2, 0.2, 0.5],
        }]))
        .unwrap();
        assert_eq!(report.status(), KernelCompletionStatus::ExactInfeasible);
        assert!(report.identified_kernels().is_empty());
        assert_eq!(
            report.completion_grid_serialization(),
            CompletionGridSerialization::NotApplicableInfeasible
        );
        assert!(report.findings().iter().any(|finding| matches!(
            finding,
            KernelCompletionFinding::InactiveKernelChanged { node: 1, .. }
        )));
    }

    #[test]
    fn repeated_level_mismatch_is_retained_with_every_finding() {
        let report = solve_finite_kernel_completion(&input(vec![
            FiniteObservedRegime {
                levels: vec![1, 0],
                probabilities: vec![0.125, 0.125, 0.375, 0.375],
            },
            FiniteObservedRegime {
                levels: vec![1, 1],
                probabilities: vec![0.08, 0.12, 0.48, 0.32],
            },
        ]))
        .unwrap();
        assert_eq!(report.status(), KernelCompletionStatus::ExactInfeasible);
        assert!(report.findings().iter().any(|finding| matches!(
            finding,
            KernelCompletionFinding::RepeatedLevelKernelMismatch { family: 0, .. }
        )));
    }

    #[test]
    fn deterministic_partial_product_families_match_level_coverage() {
        // Deterministic property battery; the seed is part of the fixture contract.
        let mut state = 0x5eed_cafe_f00d_u64;
        for _case in 0..64 {
            let baseline_x = 0.15 + 0.7 * next_unit(&mut state);
            let baseline_y = 0.15 + 0.7 * next_unit(&mut state);
            let x_levels = [
                baseline_x,
                0.15 + 0.7 * next_unit(&mut state),
                0.15 + 0.7 * next_unit(&mut state),
            ];
            let y_levels = [baseline_y, 0.15 + 0.7 * next_unit(&mut state)];
            let mut regimes = Vec::new();
            for levels in [[1, 0], [2, 0], [0, 1], [1, 1], [2, 1]] {
                if next_u64(&mut state) & 1 == 1 {
                    regimes.push(FiniteObservedRegime {
                        levels: levels.to_vec(),
                        probabilities: independent_binary_law(
                            x_levels[usize::try_from(levels[0]).unwrap()],
                            y_levels[usize::try_from(levels[1]).unwrap()],
                        ),
                    });
                }
            }
            if regimes.is_empty() {
                regimes.push(FiniteObservedRegime {
                    levels: vec![1, 0],
                    probabilities: independent_binary_law(x_levels[1], y_levels[0]),
                });
            }
            let observed = regimes
                .iter()
                .flat_map(|regime| {
                    regime
                        .levels
                        .iter()
                        .copied()
                        .enumerate()
                        .filter(|(_, level)| *level != 0)
                })
                .collect::<BTreeSet<_>>();
            let input = FiniteCompletionInput {
                law_semantics: FiniteLawSemantics::ExactOrSimulatedPopulation,
                state_cardinalities: vec![2, 2],
                states: vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]],
                baseline_probabilities: independent_binary_law(baseline_x, baseline_y),
                parents_by_node: vec![vec![], vec![]],
                families: vec![
                    FiniteMechanismFamily {
                        cardinality: 3,
                        target: 0,
                    },
                    FiniteMechanismFamily {
                        cardinality: 2,
                        target: 1,
                    },
                ],
                regimes,
                tolerance: 1e-10,
            };
            let report = solve_finite_kernel_completion(&input).unwrap();
            let expected = if [(0, 1), (0, 2), (1, 1)]
                .into_iter()
                .all(|level| observed.contains(&level))
            {
                KernelCompletionStatus::PointIdentified
            } else {
                KernelCompletionStatus::SetIdentified
            };
            assert_eq!(report.status(), expected);
            assert!(report.findings().is_empty());
        }
    }

    fn independent_binary_law(probability_x_one: f64, probability_y_one: f64) -> Vec<f64> {
        vec![
            (1.0 - probability_x_one) * (1.0 - probability_y_one),
            (1.0 - probability_x_one) * probability_y_one,
            probability_x_one * (1.0 - probability_y_one),
            probability_x_one * probability_y_one,
        ]
    }

    fn next_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    fn next_unit(state: &mut u64) -> f64 {
        let bits = next_u64(state) >> 11;
        bits as f64 / ((1_u64 << 53) as f64)
    }
}
