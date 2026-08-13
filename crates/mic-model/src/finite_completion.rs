#![forbid(unsafe_code)]
//! Finite-state modular-completion feasibility for a fixed DAG and targets.
//!
//! This reference solver is exact in design structure and numerical in the
//! supplied probability tables. It fully evaluates causal feasibility only
//! when the observed treatment-coded design identifies every factor-level log
//! potential. Rank-deficient feasible designs remain conservatively set
//! identified; nonlinear constraints can sometimes restore uniqueness, but
//! this solver does not claim that general result.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One categorical mechanism family and its fixed target node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FiniteMechanismFamily {
    /// Number of mutually exclusive levels, including baseline level zero.
    pub cardinality: u32,
    /// Target node in the fixed DAG.
    pub target: usize,
}

/// One observed nonbaseline regime law.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FiniteObservedRegime {
    /// One categorical level per mechanism family.
    pub levels: Vec<u32>,
    /// Strictly positive probabilities in the declared state order.
    pub probabilities: Vec<f64>,
}

/// Complete finite-state fixed-model input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FiniteCompletionInput {
    /// Cardinality of every observed state coordinate.
    pub state_cardinalities: Vec<u32>,
    /// Complete Cartesian state enumeration.
    pub states: Vec<Vec<u32>>,
    /// Baseline law in the same state order.
    pub baseline_probabilities: Vec<f64>,
    /// Parent indices for every node in the proposed DAG.
    pub parents_by_node: Vec<Vec<usize>>,
    /// Categorical mechanism families with distinct fixed targets.
    pub families: Vec<FiniteMechanismFamily>,
    /// Observed nonbaseline regimes.
    pub regimes: Vec<FiniteObservedRegime>,
    /// Positive numerical equality tolerance.
    pub tolerance: f64,
}

/// Completion classification relative to the fixed DAG and target assignment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompletionStatus {
    /// No compatible modular completion exists for a tested necessary clause.
    Infeasible,
    /// Every potential is algebraically identified and passes causal checks.
    IdentifiedFeasible,
    /// Algebraic potentials are underdetermined; the nonlinear causal fiber was not classified.
    CausalCompletionUnresolved,
}

/// Clause that refuted a proposed completion or prevented full evaluation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompletionFailure {
    /// Baseline law does not factorize over the proposed DAG.
    BaselineNotMarkov,
    /// An observed intervention law does not factorize over the proposed DAG.
    RegimeNotMarkov,
    /// Observed log transports are outside the main-effects design image.
    AlgebraicallyInconsistent,
    /// An identified potential depends on coordinates outside target and parents.
    NonlocalPotential,
    /// An identified local potential is not conditionally normalized.
    ConditionalNormalization,
    /// Design rank does not identify every factor-level potential.
    RankDeficient,
}

/// One identified factor-level log density ratio.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IdentifiedPotential {
    /// Mechanism-family index.
    pub family: usize,
    /// Nonbaseline categorical level.
    pub level: u32,
    /// Log density ratio in the declared state order.
    pub log_ratio: Vec<f64>,
}

/// Two-axis finite-state solver output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FiniteCompletionReport {
    /// Feasibility/identification status relative to the fixed model.
    pub status: CompletionStatus,
    /// First clause preventing an identified feasible completion, if any.
    pub failure: Option<CompletionFailure>,
    /// Rank of the observed treatment-coded design.
    pub algebraic_rank: usize,
    /// Number of factor-level potentials in a full completion.
    pub n_potentials: usize,
    /// Left-null lack-of-fit dimension of the observed design.
    pub lack_of_fit_dimension: usize,
    /// Whether the observed design contains any additive lack-of-fit contrast.
    pub additive_lack_of_fit_testable: bool,
    /// Whether locality and conditional normalization were evaluated.
    pub causal_potentials_evaluated: bool,
    /// Identified potentials, present only for `identified_feasible`.
    pub potentials: Vec<IdentifiedPotential>,
}

/// Invalid finite-state input contract.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum FiniteCompletionError {
    /// Equality tolerance was unusable.
    #[error("tolerance must be finite and positive")]
    InvalidTolerance,
    /// State space was empty, ragged, incomplete, duplicated, or out of range.
    #[error("states must enumerate one complete finite Cartesian product")]
    InvalidStateSpace,
    /// DAG parent lists were malformed or cyclic.
    #[error("parents_by_node must define one acyclic graph over the state coordinates")]
    InvalidDag,
    /// Family cardinalities or distinct-target semantics were invalid.
    #[error("families require cardinality >= 2 and distinct in-range targets")]
    InvalidFamilies,
    /// A regime code was malformed, duplicated, or all baseline.
    #[error("regime levels must be unique, in range, nonbaseline family codes")]
    InvalidRegimes,
    /// A probability table did not match the state space or positive simplex.
    #[error("{law} must be a strictly positive finite simplex over the state space")]
    InvalidLaw {
        /// Invalid law label.
        law: String,
    },
}

enum LinearSolution {
    Inconsistent,
    Underdetermined,
    Unique(Vec<f64>),
}

enum TransportSolutions {
    Inconsistent,
    Underdetermined,
    Identified(Vec<Vec<f64>>),
}

/// Classifies finite-state modular completions for a fixed DAG and target map.
pub fn solve_finite_modular_completion(
    input: &FiniteCompletionInput,
) -> Result<FiniteCompletionReport, FiniteCompletionError> {
    validate_input(input)?;
    let columns = potential_columns(&input.families);
    let design = treatment_design(&input.regimes, &columns);
    let algebraic_rank = matrix_rank(&design, input.tolerance);
    let lack_of_fit_dimension = input.regimes.len().saturating_sub(algebraic_rank);
    let base_report = |status, failure, evaluated, potentials| FiniteCompletionReport {
        status,
        failure,
        algebraic_rank,
        n_potentials: columns.len(),
        lack_of_fit_dimension,
        additive_lack_of_fit_testable: lack_of_fit_dimension > 0,
        causal_potentials_evaluated: evaluated,
        potentials,
    };

    if !factorizes_over_dag(
        &input.baseline_probabilities,
        &input.states,
        &input.parents_by_node,
        input.tolerance,
    ) {
        return Ok(base_report(
            CompletionStatus::Infeasible,
            Some(CompletionFailure::BaselineNotMarkov),
            false,
            Vec::new(),
        ));
    }
    if input.regimes.iter().any(|regime| {
        !factorizes_over_dag(
            &regime.probabilities,
            &input.states,
            &input.parents_by_node,
            input.tolerance,
        )
    }) {
        return Ok(base_report(
            CompletionStatus::Infeasible,
            Some(CompletionFailure::RegimeNotMarkov),
            false,
            Vec::new(),
        ));
    }

    let state_solutions = match solve_transports(input, &design) {
        TransportSolutions::Inconsistent => {
            return Ok(base_report(
                CompletionStatus::Infeasible,
                Some(CompletionFailure::AlgebraicallyInconsistent),
                false,
                Vec::new(),
            ));
        }
        TransportSolutions::Underdetermined => {
            return Ok(base_report(
                CompletionStatus::CausalCompletionUnresolved,
                Some(CompletionFailure::RankDeficient),
                false,
                Vec::new(),
            ));
        }
        TransportSolutions::Identified(solutions) => solutions,
    };

    let potentials = columns
        .iter()
        .enumerate()
        .map(|(column, &(family, level))| IdentifiedPotential {
            family,
            level,
            log_ratio: state_solutions
                .iter()
                .map(|solution| solution[column])
                .collect(),
        })
        .collect::<Vec<_>>();
    if let Some(failure) = causal_potential_failure(input, &potentials) {
        return Ok(base_report(
            CompletionStatus::Infeasible,
            Some(failure),
            true,
            Vec::new(),
        ));
    }
    Ok(base_report(
        CompletionStatus::IdentifiedFeasible,
        None,
        true,
        potentials,
    ))
}

fn causal_potential_failure(
    input: &FiniteCompletionInput,
    potentials: &[IdentifiedPotential],
) -> Option<CompletionFailure> {
    for potential in potentials {
        let target = input.families[potential.family].target;
        let parents = &input.parents_by_node[target];
        if !is_local(
            &potential.log_ratio,
            &input.states,
            target,
            parents,
            input.tolerance,
        ) {
            return Some(CompletionFailure::NonlocalPotential);
        }
        if !is_conditionally_normalized(
            &potential.log_ratio,
            &input.baseline_probabilities,
            &input.states,
            parents,
            input.tolerance,
        ) {
            return Some(CompletionFailure::ConditionalNormalization);
        }
    }
    None
}

fn solve_transports(input: &FiniteCompletionInput, design: &[Vec<f64>]) -> TransportSolutions {
    let mut state_solutions = Vec::with_capacity(input.states.len());
    let mut underdetermined = false;
    for state in 0..input.states.len() {
        let transport = input
            .regimes
            .iter()
            .map(|regime| {
                regime.probabilities[state].ln() - input.baseline_probabilities[state].ln()
            })
            .collect::<Vec<_>>();
        match solve_linear(design, &transport, input.tolerance) {
            LinearSolution::Inconsistent => return TransportSolutions::Inconsistent,
            LinearSolution::Underdetermined => underdetermined = true,
            LinearSolution::Unique(solution) => state_solutions.push(solution),
        }
    }
    if underdetermined {
        TransportSolutions::Underdetermined
    } else {
        TransportSolutions::Identified(state_solutions)
    }
}

fn validate_input(input: &FiniteCompletionInput) -> Result<(), FiniteCompletionError> {
    if !input.tolerance.is_finite() || input.tolerance <= 0.0 {
        return Err(FiniteCompletionError::InvalidTolerance);
    }
    validate_states(&input.states, &input.state_cardinalities)?;
    validate_dag(&input.parents_by_node, input.state_cardinalities.len())?;
    validate_families(&input.families, input.state_cardinalities.len())?;
    validate_law(
        &input.baseline_probabilities,
        input.states.len(),
        input.tolerance,
        "baseline",
    )?;
    let mut codes = BTreeSet::new();
    for (index, regime) in input.regimes.iter().enumerate() {
        if regime.levels.len() != input.families.len()
            || regime.levels.iter().all(|level| *level == 0)
            || regime
                .levels
                .iter()
                .zip(&input.families)
                .any(|(level, family)| *level >= family.cardinality)
            || !codes.insert(regime.levels.clone())
        {
            return Err(FiniteCompletionError::InvalidRegimes);
        }
        validate_law(
            &regime.probabilities,
            input.states.len(),
            input.tolerance,
            &format!("regime_{index}"),
        )?;
    }
    if input.regimes.is_empty() {
        return Err(FiniteCompletionError::InvalidRegimes);
    }
    Ok(())
}

fn validate_states(
    states: &[Vec<u32>],
    cardinalities: &[u32],
) -> Result<(), FiniteCompletionError> {
    if cardinalities.is_empty() || cardinalities.iter().any(|value| *value < 2) {
        return Err(FiniteCompletionError::InvalidStateSpace);
    }
    let expected = cardinalities
        .iter()
        .try_fold(1_usize, |product, value| {
            usize::try_from(*value)
                .ok()
                .and_then(|factor| product.checked_mul(factor))
        })
        .ok_or(FiniteCompletionError::InvalidStateSpace)?;
    let unique = states.iter().cloned().collect::<BTreeSet<_>>();
    if states.len() != expected
        || unique.len() != expected
        || states.iter().any(|state| {
            state.len() != cardinalities.len()
                || state
                    .iter()
                    .zip(cardinalities)
                    .any(|(value, cardinality)| value >= cardinality)
        })
    {
        return Err(FiniteCompletionError::InvalidStateSpace);
    }
    Ok(())
}

fn validate_dag(parents: &[Vec<usize>], n_nodes: usize) -> Result<(), FiniteCompletionError> {
    if parents.len() != n_nodes {
        return Err(FiniteCompletionError::InvalidDag);
    }
    let mut children = vec![Vec::new(); n_nodes];
    let mut indegree = vec![0_usize; n_nodes];
    for (node, node_parents) in parents.iter().enumerate() {
        let unique = node_parents.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != node_parents.len()
            || node_parents
                .iter()
                .any(|parent| *parent >= n_nodes || *parent == node)
        {
            return Err(FiniteCompletionError::InvalidDag);
        }
        indegree[node] = node_parents.len();
        for parent in node_parents {
            children[*parent].push(node);
        }
    }
    let mut queue = indegree
        .iter()
        .enumerate()
        .filter_map(|(node, degree)| (*degree == 0).then_some(node))
        .collect::<VecDeque<_>>();
    let mut visited = 0;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        for child in &children[node] {
            indegree[*child] -= 1;
            if indegree[*child] == 0 {
                queue.push_back(*child);
            }
        }
    }
    if visited == n_nodes {
        Ok(())
    } else {
        Err(FiniteCompletionError::InvalidDag)
    }
}

fn validate_families(
    families: &[FiniteMechanismFamily],
    n_nodes: usize,
) -> Result<(), FiniteCompletionError> {
    let targets = families
        .iter()
        .map(|family| family.target)
        .collect::<BTreeSet<_>>();
    if families.is_empty()
        || targets.len() != families.len()
        || families
            .iter()
            .any(|family| family.cardinality < 2 || family.target >= n_nodes)
    {
        Err(FiniteCompletionError::InvalidFamilies)
    } else {
        Ok(())
    }
}

fn validate_law(
    probabilities: &[f64],
    n_states: usize,
    tolerance: f64,
    law: &str,
) -> Result<(), FiniteCompletionError> {
    if probabilities.len() != n_states
        || probabilities
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        || (probabilities.iter().sum::<f64>() - 1.0).abs() > tolerance
    {
        Err(FiniteCompletionError::InvalidLaw {
            law: law.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn potential_columns(families: &[FiniteMechanismFamily]) -> Vec<(usize, u32)> {
    families
        .iter()
        .enumerate()
        .flat_map(|(family, spec)| (1..spec.cardinality).map(move |level| (family, level)))
        .collect()
}

fn treatment_design(regimes: &[FiniteObservedRegime], columns: &[(usize, u32)]) -> Vec<Vec<f64>> {
    regimes
        .iter()
        .map(|regime| {
            columns
                .iter()
                .map(|&(family, level)| f64::from(regime.levels[family] == level))
                .collect()
        })
        .collect()
}

fn matrix_rank(matrix: &[Vec<f64>], tolerance: f64) -> usize {
    if matrix.is_empty() {
        return 0;
    }
    let mut work = matrix.to_vec();
    let n_columns = work[0].len();
    let mut pivot_row = 0;
    for column in 0..n_columns {
        let Some(row) = (pivot_row..work.len()).max_by(|left, right| {
            work[*left][column]
                .abs()
                .total_cmp(&work[*right][column].abs())
        }) else {
            break;
        };
        if work[row][column].abs() <= tolerance {
            continue;
        }
        work.swap(pivot_row, row);
        let pivot = work[pivot_row][column];
        for value in &mut work[pivot_row][column..] {
            *value /= pivot;
        }
        let pivot_values = work[pivot_row].clone();
        for (row_index, values) in work.iter_mut().enumerate() {
            if row_index == pivot_row {
                continue;
            }
            let scale = values[column];
            for index in column..n_columns {
                values[index] -= scale * pivot_values[index];
            }
        }
        pivot_row += 1;
        if pivot_row == work.len() {
            break;
        }
    }
    pivot_row
}

fn solve_linear(matrix: &[Vec<f64>], rhs: &[f64], tolerance: f64) -> LinearSolution {
    let n_columns = matrix.first().map_or(0, Vec::len);
    let mut augmented = matrix
        .iter()
        .zip(rhs)
        .map(|(row, value)| {
            let mut combined = row.clone();
            combined.push(*value);
            combined
        })
        .collect::<Vec<_>>();
    let mut pivot_row = 0;
    let mut pivots = Vec::new();
    for column in 0..n_columns {
        let Some(row) = (pivot_row..augmented.len()).max_by(|left, right| {
            augmented[*left][column]
                .abs()
                .total_cmp(&augmented[*right][column].abs())
        }) else {
            break;
        };
        if augmented[row][column].abs() <= tolerance {
            continue;
        }
        augmented.swap(pivot_row, row);
        let pivot = augmented[pivot_row][column];
        for value in &mut augmented[pivot_row][column..=n_columns] {
            *value /= pivot;
        }
        let pivot_values = augmented[pivot_row].clone();
        for (row_index, values) in augmented.iter_mut().enumerate() {
            if row_index == pivot_row {
                continue;
            }
            let scale = values[column];
            for index in column..=n_columns {
                values[index] -= scale * pivot_values[index];
            }
        }
        pivots.push(column);
        pivot_row += 1;
    }
    if augmented.iter().any(|row| {
        row[..n_columns]
            .iter()
            .all(|value| value.abs() <= tolerance)
            && row[n_columns].abs() > tolerance
    }) {
        return LinearSolution::Inconsistent;
    }
    if pivots.len() < n_columns {
        return LinearSolution::Underdetermined;
    }
    let mut solution = vec![0.0; n_columns];
    for (row, column) in pivots.into_iter().enumerate() {
        solution[column] = augmented[row][n_columns];
    }
    LinearSolution::Unique(solution)
}

fn factorizes_over_dag(
    law: &[f64],
    states: &[Vec<u32>],
    parents: &[Vec<usize>],
    tolerance: f64,
) -> bool {
    for (state_index, state) in states.iter().enumerate() {
        let mut log_reconstructed = 0.0;
        for (node, node_parents) in parents.iter().enumerate() {
            let denominator = states
                .iter()
                .enumerate()
                .filter(|(_, candidate)| same_coordinates(candidate, state, node_parents))
                .map(|(index, _)| law[index])
                .sum::<f64>();
            let numerator = states
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    candidate[node] == state[node]
                        && same_coordinates(candidate, state, node_parents)
                })
                .map(|(index, _)| law[index])
                .sum::<f64>();
            log_reconstructed += numerator.ln() - denominator.ln();
        }
        if (log_reconstructed - law[state_index].ln()).abs() > tolerance {
            return false;
        }
    }
    true
}

fn same_coordinates(left: &[u32], right: &[u32], coordinates: &[usize]) -> bool {
    coordinates
        .iter()
        .all(|coordinate| left[*coordinate] == right[*coordinate])
}

fn is_local(
    log_ratio: &[f64],
    states: &[Vec<u32>],
    target: usize,
    parents: &[usize],
    tolerance: f64,
) -> bool {
    let mut coordinates = Vec::with_capacity(parents.len() + 1);
    coordinates.push(target);
    coordinates.extend_from_slice(parents);
    let mut values = BTreeMap::<Vec<u32>, f64>::new();
    for (state, value) in states.iter().zip(log_ratio) {
        let key = coordinates
            .iter()
            .map(|coordinate| state[*coordinate])
            .collect::<Vec<_>>();
        if values
            .insert(key.clone(), *value)
            .is_some_and(|previous| (previous - value).abs() > tolerance)
        {
            return false;
        }
    }
    true
}

fn is_conditionally_normalized(
    log_ratio: &[f64],
    baseline: &[f64],
    states: &[Vec<u32>],
    parents: &[usize],
    tolerance: f64,
) -> bool {
    let mut groups = BTreeMap::<Vec<u32>, (f64, Vec<f64>)>::new();
    for ((state, probability), potential) in states.iter().zip(baseline).zip(log_ratio) {
        let key = parents
            .iter()
            .map(|parent| state[*parent])
            .collect::<Vec<_>>();
        let entry = groups.entry(key).or_default();
        entry.0 += probability;
        entry.1.push(probability.ln() + potential);
    }
    groups.values().all(|(mass, log_terms)| {
        let log_tilted_mass = log_sum_exp(log_terms);
        let log_ratio = log_tilted_mass - mass.ln();
        log_ratio.is_finite() && log_ratio.exp_m1().abs() <= tolerance
    })
}

fn log_sum_exp(values: &[f64]) -> f64 {
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    maximum
        + values
            .iter()
            .map(|value| (value - maximum).exp())
            .sum::<f64>()
            .ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binary_states() -> Vec<Vec<u32>> {
        vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]]
    }

    fn root_input(regimes: Vec<FiniteObservedRegime>) -> FiniteCompletionInput {
        FiniteCompletionInput {
            state_cardinalities: vec![2, 2],
            states: binary_states(),
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
    fn primitive_partial_design_identifies_feasible_root_replacements() {
        let report = solve_finite_modular_completion(&root_input(vec![
            FiniteObservedRegime {
                levels: vec![1, 0],
                probabilities: vec![0.125, 0.125, 0.375, 0.375],
            },
            FiniteObservedRegime {
                levels: vec![0, 1],
                probabilities: vec![0.2, 0.3, 0.2, 0.3],
            },
        ]))
        .unwrap();
        assert_eq!(report.status, CompletionStatus::IdentifiedFeasible);
        assert_eq!(report.algebraic_rank, 2);
        assert_eq!(report.lack_of_fit_dimension, 0);
        assert!(!report.additive_lack_of_fit_testable);
        assert_eq!(report.potentials.len(), 2);
        assert!(report.causal_potentials_evaluated);
    }

    #[test]
    fn flat_identified_but_nonlocal_potentials_are_infeasible() {
        let mut input = root_input(vec![
            FiniteObservedRegime {
                levels: vec![1, 0],
                probabilities: vec![0.375, 0.25, 0.25, 0.125],
            },
            FiniteObservedRegime {
                levels: vec![0, 1],
                probabilities: vec![0.25, 0.375, 0.125, 0.25],
            },
        ]);
        input.parents_by_node = vec![vec![], vec![0]];
        let report = solve_finite_modular_completion(&input).unwrap();
        assert_eq!(report.status, CompletionStatus::Infeasible);
        assert_eq!(report.failure, Some(CompletionFailure::NonlocalPotential));
    }

    #[test]
    fn rank_deficient_correlated_root_regime_fails_markov_first() {
        let report = solve_finite_modular_completion(&root_input(vec![FiniteObservedRegime {
            levels: vec![1, 1],
            probabilities: vec![0.4, 0.1, 0.1, 0.4],
        }]))
        .unwrap();
        assert_eq!(report.status, CompletionStatus::Infeasible);
        assert_eq!(report.failure, Some(CompletionFailure::RegimeNotMarkov));
        assert_eq!(report.algebraic_rank, 1);
    }

    #[test]
    fn rank_deficient_product_regime_leaves_causal_fiber_unresolved() {
        let report = solve_finite_modular_completion(&root_input(vec![FiniteObservedRegime {
            levels: vec![1, 1],
            probabilities: vec![0.12, 0.28, 0.18, 0.42],
        }]))
        .unwrap();
        assert_eq!(report.status, CompletionStatus::CausalCompletionUnresolved);
        assert_eq!(report.failure, Some(CompletionFailure::RankDeficient));
        assert!(!report.causal_potentials_evaluated);
    }

    #[test]
    fn full_rank_nonflat_family_is_algebraically_infeasible() {
        let mut input = root_input(vec![
            FiniteObservedRegime {
                levels: vec![1, 0],
                probabilities: vec![0.25; 4],
            },
            FiniteObservedRegime {
                levels: vec![0, 1],
                probabilities: vec![0.25; 4],
            },
            FiniteObservedRegime {
                levels: vec![1, 1],
                probabilities: vec![0.4, 0.1, 0.1, 0.4],
            },
        ]);
        input.parents_by_node = vec![vec![], vec![0]];
        let report = solve_finite_modular_completion(&input).unwrap();
        assert_eq!(report.status, CompletionStatus::Infeasible);
        assert_eq!(
            report.failure,
            Some(CompletionFailure::AlgebraicallyInconsistent)
        );
        assert_eq!(report.algebraic_rank, 2);
        assert_eq!(report.lack_of_fit_dimension, 1);
        assert!(report.additive_lack_of_fit_testable);
    }

    #[test]
    fn local_global_normalization_does_not_replace_conditional_normalization() {
        let mut input = root_input(vec![FiniteObservedRegime {
            levels: vec![1, 0],
            probabilities: vec![0.3, 0.3, 0.2, 0.2],
        }]);
        input.parents_by_node = vec![vec![], vec![0]];
        input.families = vec![FiniteMechanismFamily {
            cardinality: 2,
            target: 1,
        }];
        input.regimes[0].levels = vec![1];
        let report = solve_finite_modular_completion(&input).unwrap();
        assert_eq!(report.status, CompletionStatus::Infeasible);
        assert_eq!(
            report.failure,
            Some(CompletionFailure::ConditionalNormalization)
        );
    }

    #[test]
    fn extreme_but_finite_ratios_are_solved_in_the_log_domain() {
        let report = solve_finite_modular_completion(&FiniteCompletionInput {
            state_cardinalities: vec![2],
            states: vec![vec![0], vec![1]],
            baseline_probabilities: vec![1e-320, 1.0],
            parents_by_node: vec![vec![]],
            families: vec![FiniteMechanismFamily {
                cardinality: 2,
                target: 0,
            }],
            regimes: vec![FiniteObservedRegime {
                levels: vec![1],
                probabilities: vec![0.5, 0.5],
            }],
            tolerance: 1e-12,
        })
        .unwrap();
        assert_eq!(report.status, CompletionStatus::IdentifiedFeasible);
        assert!(
            report.potentials[0]
                .log_ratio
                .iter()
                .all(|value| value.is_finite())
        );
    }
}
