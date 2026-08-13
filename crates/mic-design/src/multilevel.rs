#![forbid(unsafe_code)]
//! Multi-level factorial geometry and fail-closed family classification.
//!
//! Geometry alone never evaluates a nonlinear causal-completion fiber. Supplied
//! laws can refute or identify a narrowly fixed model, while rank and
//! lack-of-fit remain separate testability facts.

use crate::{
    CausalCompletionEvaluation, DesignError, DesignPoint, ObservedDesign, audit_design,
    classify_two_root_diagonal, matrix_rank,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One cell of a product of categorical mechanism families.
///
/// Level `0` is the baseline of that family. Other levels are alternative
/// guides, doses, or implementations of the same family.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct MultiLevelPoint {
    /// Level of each family, in family order.
    pub levels: Vec<u32>,
}

impl MultiLevelPoint {
    /// Constructs a point and rejects an empty coordinate list.
    pub fn new(levels: Vec<u32>) -> Result<Self, DesignError> {
        if levels.is_empty() {
            return Err(DesignError::EmptyPoint);
        }
        Ok(Self { levels })
    }

    /// Number of mechanism families.
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.levels.len()
    }
}

/// Treatment-coded intercept-plus-main-effects matrix.
///
/// For family `j` with `L_j` levels, the design contributes `L_j - 1` columns
/// `I(a_j = 1), …, I(a_j = L_j - 1)`. Level 0 is the reference.
pub fn multilevel_main_effects_matrix(
    points: &[MultiLevelPoint],
    cardinalities: &[u32],
) -> Result<Vec<Vec<f64>>, DesignError> {
    if points.is_empty() {
        return Err(DesignError::EmptyDesign);
    }
    if cardinalities.len() != points[0].dimension() {
        return Err(DesignError::DimensionMismatch {
            expected: points[0].dimension(),
            actual: cardinalities.len(),
        });
    }
    for (factor, &levels) in cardinalities.iter().enumerate() {
        if levels < 2 {
            return Err(DesignError::InvalidCardinality { factor, levels });
        }
    }
    let mut matrix = Vec::with_capacity(points.len());
    for point in points {
        if point.dimension() != cardinalities.len() {
            return Err(DesignError::DimensionMismatch {
                expected: cardinalities.len(),
                actual: point.dimension(),
            });
        }
        let mut row = vec![1.0];
        for (factor, (&level, &levels)) in point.levels.iter().zip(cardinalities).enumerate() {
            if level >= levels {
                return Err(DesignError::LevelOutOfRange {
                    factor,
                    level,
                    levels,
                });
            }
            for code in 1..levels {
                row.push(if level == code { 1.0 } else { 0.0 });
            }
        }
        matrix.push(row);
    }
    Ok(matrix)
}

/// One fully observed rectangle across two families and two levels each.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RectangleFace {
    /// First family index.
    pub first: usize,
    /// Second family index.
    pub second: usize,
    /// The two levels of the first family, lower first.
    pub first_levels: [u32; 2],
    /// The two levels of the second family, lower first.
    pub second_levels: [u32; 2],
    /// Coordinates of the remaining families, held fixed.
    pub held: Vec<(usize, u32)>,
}

/// Enumerates every fully observed 2×2 rectangle in a multi-level design.
pub fn enumerate_rectangles(
    points: &[MultiLevelPoint],
    cardinalities: &[u32],
) -> Result<Vec<RectangleFace>, DesignError> {
    if points.is_empty() {
        return Ok(Vec::new());
    }
    let _ = multilevel_main_effects_matrix(points, cardinalities)?;
    let observed: BTreeSet<&[u32]> = points.iter().map(|point| point.levels.as_slice()).collect();
    let dimension = cardinalities.len();
    let mut faces = Vec::new();
    for first in 0..dimension {
        for second in (first + 1)..dimension {
            for a in 0..cardinalities[first] {
                for a_prime in (a + 1)..cardinalities[first] {
                    for b in 0..cardinalities[second] {
                        for b_prime in (b + 1)..cardinalities[second] {
                            for point in points {
                                if point.levels[first] != a || point.levels[second] != b {
                                    continue;
                                }
                                let ab = point.levels.clone();
                                let mut a_b = point.levels.clone();
                                let mut ab_ = point.levels.clone();
                                let mut a_b_ = point.levels.clone();
                                a_b[first] = a_prime;
                                ab_[second] = b_prime;
                                a_b_[first] = a_prime;
                                a_b_[second] = b_prime;
                                if observed.contains(ab.as_slice())
                                    && observed.contains(a_b.as_slice())
                                    && observed.contains(ab_.as_slice())
                                    && observed.contains(a_b_.as_slice())
                                {
                                    let held = point
                                        .levels
                                        .iter()
                                        .enumerate()
                                        .filter(|(index, _)| *index != first && *index != second)
                                        .map(|(index, &level)| (index, level))
                                        .collect();
                                    faces.push(RectangleFace {
                                        first,
                                        second,
                                        first_levels: [a, a_prime],
                                        second_levels: [b, b_prime],
                                        held,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    faces.sort_by(|left, right| {
        left.first
            .cmp(&right.first)
            .then(left.second.cmp(&right.second))
            .then(left.first_levels.cmp(&right.first_levels))
            .then(left.second_levels.cmp(&right.second_levels))
            .then(left.held.cmp(&right.held))
    });
    faces.dedup();
    Ok(faces)
}

/// Whether deletion orientation is even a well-posed question on this family.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrientationTestability {
    /// No declared same-target tilt family of size ≥ 2.
    Untestable,
    /// Caller declared at least two independent tilts of one target.
    TestableWithDeclaredTilts,
}

/// Orientation is untestable unless the caller declared a same-target tilt family.
#[must_use]
pub fn orientation_testability(same_target_tilt_count: usize) -> OrientationTestability {
    if same_target_tilt_count >= 2 {
        OrientationTestability::TestableWithDeclaredTilts
    } else {
        OrientationTestability::Untestable
    }
}

/// Inputs for the fail-closed observed-family classifier.
#[derive(Debug, Clone, Copy)]
pub struct FamilyClassificationInput<'a> {
    /// Observed Boolean corners.
    pub points: &'a [DesignPoint],
    /// Declared number of independent same-target tilts. Survey passes 0.
    pub same_target_tilt_count: usize,
    /// Whether the caller proposed distinct root-mechanism targets.
    pub distinct_root_targets: bool,
    /// Optional baseline and combo 2×2 laws in order `00, 10, 01, 11`.
    pub baseline_combo_laws: Option<([f64; 4], [f64; 4])>,
}

/// Why a ranked cell is absent from the retained design.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NextCornerKind {
    /// The cell never appeared in the table.
    NeverSeen,
    /// The cell was observed below `min_corner_count` and dropped.
    UnderSupported,
}

/// One unobserved cell scored by how much it shrinks the additive identified set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NextCornerCandidate {
    /// Bit-string or `level,level,…` label.
    pub corner: String,
    /// Never-seen arm versus under-supported already-attempted cell.
    pub kind: NextCornerKind,
    /// Drop in `n_coded − rank` after adding this cell.
    pub identified_set_reduction: usize,
    /// Identified-set dimension if this cell is collected.
    pub new_identified_set_dimension: usize,
    /// Increase in lack-of-fit dimension after adding this cell.
    pub lack_of_fit_gain: usize,
    /// Lack-of-fit dimension if this cell is collected.
    pub new_lack_of_fit_dimension: usize,
    /// Positive integer cost. Default unit cost is 1000.
    pub cost: u32,
}

/// Geometry-plus-witness classification. Never a certificate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservedFamilyClassification {
    /// Fixed-model causal-completion evaluation, separate from testability.
    pub completion_evaluation: CausalCompletionEvaluation,
    /// Orientation testability. Catalog squares with no tilt family are untestable.
    pub orientation: OrientationTestability,
    /// `n_coded_columns - main_effects_rank` on the observed corners.
    pub identified_set_dimension: usize,
    /// Lack-of-fit dimension of the Boolean main-effects model.
    pub lack_of_fit_dimension: usize,
    /// Rank of intercept-plus-main-effects.
    pub main_effects_rank: usize,
    /// Number of coded columns (`1 + dimension` for Boolean designs).
    pub n_coded_columns: usize,
    /// Missing one-bit primitive corners, if the design is two-factor.
    pub missing_primitive_corners: Vec<String>,
    /// Highest-ranked unobserved cell, if any remain.
    pub recommended_next_corner: Option<String>,
    /// Identified-set reduction delivered by that cell.
    pub next_corner_identified_set_reduction: Option<usize>,
    /// Integer cost of that cell (default 1000).
    pub next_corner_cost: Option<u32>,
    /// Human-readable refusal or witness note.
    pub note: String,
}

/// Diagnostic audit of supplied log-laws on fully observed rectangles.
///
/// Flat rectangles are not a modularity certificate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RectangleLawAudit {
    /// Number of fully observed rectangles in the design.
    pub n_observed_rectangles: usize,
    /// Rectangles for which all four log-laws were supplied.
    pub n_evaluated: usize,
    /// Rectangles skipped because a law was missing.
    pub n_missing_laws: usize,
    /// Largest absolute rectangle contrast among evaluated faces.
    pub max_abs_contrast: Option<f64>,
    /// True when at least one rectangle was evaluated and all lie within tolerance.
    pub all_flat: bool,
    /// Reminder that this is not a certificate.
    pub note: String,
}

/// Evaluates supplied log-laws on every fully observed rectangle.
pub fn audit_rectangle_laws(
    points: &[MultiLevelPoint],
    cardinalities: &[u32],
    log_laws: &BTreeMap<Vec<u32>, f64>,
    tolerance: f64,
) -> Result<RectangleLawAudit, DesignError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(DesignError::InvalidTolerance(tolerance));
    }
    let faces = enumerate_rectangles(points, cardinalities)?;
    let mut n_evaluated = 0_usize;
    let mut n_missing_laws = 0_usize;
    let mut max_abs_contrast = 0.0_f64;
    let mut any = false;
    for face in &faces {
        match rectangle_contrast(face, log_laws) {
            Ok(value) => {
                n_evaluated += 1;
                any = true;
                max_abs_contrast = max_abs_contrast.max(value.abs());
            }
            Err(DesignError::MissingLaw(_)) => n_missing_laws += 1,
            Err(error) => return Err(error),
        }
    }
    let all_flat = n_evaluated > 0 && n_missing_laws == 0 && max_abs_contrast <= tolerance;
    Ok(RectangleLawAudit {
        n_observed_rectangles: faces.len(),
        n_evaluated,
        n_missing_laws,
        max_abs_contrast: any.then_some(max_abs_contrast),
        all_flat,
        note: if all_flat {
            "observed rectangles are flat under the supplied laws; this is a diagnostic, not a modularity certificate"
        } else if n_evaluated == 0 {
            "no rectangle could be evaluated; missing laws or no observed rectangle"
        } else if n_missing_laws > 0 {
            "not all observed rectangles could be evaluated because supplied laws are missing"
        } else {
            "at least one evaluated rectangle is curved under the supplied laws"
        }
        .into(),
    })
}

/// Classifies an observed Boolean family.
///
/// Decided cases:
/// - `D = {00, 11}` with distinct roots and supplied laws → two-root diagonal witness
/// - any design with fewer than two declared same-target tilts → orientation untestable
///
/// Geometry-only cases remain `not_evaluated`; they are not a fourth fiber
/// state called `untestable`.
pub fn classify_observed_family(
    input: FamilyClassificationInput<'_>,
    tolerance: f64,
) -> Result<ObservedFamilyClassification, DesignError> {
    let audit = audit_design(input.points, tolerance)?;
    let dimension = input.points[0].dimension();
    let n_coded_columns = dimension + 1;
    let identified_set_dimension = n_coded_columns.saturating_sub(audit.main_effects_rank);
    let missing_primitive_corners = missing_two_factor_primitives(input.points);
    let orientation = orientation_testability(input.same_target_tilt_count);
    let diagonal = is_two_root_diagonal(input.points);
    let (completion_evaluation, note) = if diagonal
        && input.distinct_root_targets
        && let Some((baseline, combo)) = input.baseline_combo_laws
    {
        let class = classify_two_root_diagonal(baseline, combo, tolerance)?;
        let note = match class {
            CausalCompletionEvaluation::Infeasible => {
                "D={00,11} violates baseline or combination factorization and cannot be realized by the fixed distinct-root model"
            }
            CausalCompletionEvaluation::PointIdentified => {
                "D={00,11} point-identifies the two root replacement marginals for the fixed target mapping; composition remains untested"
            }
            CausalCompletionEvaluation::SetIdentified
            | CausalCompletionEvaluation::NotEvaluated => {
                "diagonal witness returned an unexpected class"
            }
        };
        (class, note.to_string())
    } else if diagonal {
        (
            CausalCompletionEvaluation::NotEvaluated,
            "D={00,11} has vacuous flatness; do not read modularity from a trivial lack-of-fit space".into(),
        )
    } else if orientation == OrientationTestability::Untestable
        && audit.square_faces.len() == 1
        && missing_primitive_corners.is_empty()
    {
        (
            CausalCompletionEvaluation::NotEvaluated,
            "complete catalog square with no supplied laws or fixed causal model: completion is not evaluated and orientation is untestable".into(),
        )
    } else {
        (
            CausalCompletionEvaluation::NotEvaluated,
            "geometry alone does not evaluate the nonlinear causal-completion fiber".into(),
        )
    };
    let next = rank_missing_boolean_corners(input.points, tolerance)?;
    Ok(ObservedFamilyClassification {
        completion_evaluation,
        orientation,
        identified_set_dimension,
        lack_of_fit_dimension: audit.lack_of_fit_dimension,
        main_effects_rank: audit.main_effects_rank,
        n_coded_columns,
        missing_primitive_corners,
        recommended_next_corner: next.first().map(|item| item.corner.clone()),
        next_corner_identified_set_reduction: next
            .first()
            .map(|item| item.identified_set_reduction),
        next_corner_cost: next.first().map(|item| item.cost),
        note,
    })
}

/// Ranks unobserved Boolean corners by identified-set reduction per unit cost.
///
/// Dimension is capped at 6 so the enumeration stays a design audit, not a search.
/// Every missing cell has unit cost 1000.
pub fn rank_missing_boolean_corners(
    points: &[DesignPoint],
    tolerance: f64,
) -> Result<Vec<NextCornerCandidate>, DesignError> {
    rank_missing_boolean_corners_with_costs(points, &BTreeMap::new(), tolerance)
}

/// Same ranking with optional positive integer costs (default 1000).
pub fn rank_missing_boolean_corners_with_costs(
    points: &[DesignPoint],
    costs: &BTreeMap<String, u32>,
    tolerance: f64,
) -> Result<Vec<NextCornerCandidate>, DesignError> {
    rank_missing_boolean_corners_with_kinds(points, costs, &BTreeSet::new(), tolerance)
}

/// Same ranking, tagging dropped-but-observed cells as under-supported.
pub fn rank_missing_boolean_corners_from_observed(
    design: &ObservedDesign,
    costs: &BTreeMap<String, u32>,
    tolerance: f64,
) -> Result<Vec<NextCornerCandidate>, DesignError> {
    let dropped: BTreeSet<String> = design
        .dropped
        .iter()
        .map(|corner| corner.point.bit_string())
        .collect();
    rank_missing_boolean_corners_with_kinds(&design.points, costs, &dropped, tolerance)
}

fn rank_missing_boolean_corners_with_kinds(
    points: &[DesignPoint],
    costs: &BTreeMap<String, u32>,
    under_supported: &BTreeSet<String>,
    tolerance: f64,
) -> Result<Vec<NextCornerCandidate>, DesignError> {
    if points.is_empty() {
        return Err(DesignError::EmptyDesign);
    }
    let dimension = points[0].dimension();
    if dimension == 0 || dimension > 6 {
        return Ok(Vec::new());
    }
    let current = audit_design(points, tolerance)?;
    let n_coded = dimension + 1;
    let current_idim = n_coded.saturating_sub(current.main_effects_rank);
    let seen: BTreeSet<String> = points.iter().map(DesignPoint::bit_string).collect();
    let n_corners = 1_usize << dimension;
    let mut ranked = Vec::new();
    for index in 0..n_corners {
        let label: String = (0..dimension)
            .map(|bit| if (index >> bit) & 1 == 1 { '1' } else { '0' })
            .collect();
        if seen.contains(&label) {
            continue;
        }
        let cost = *costs.get(&label).unwrap_or(&1000);
        if cost == 0 {
            return Err(DesignError::InvalidCost {
                corner: label,
                cost,
            });
        }
        let mut expanded = points.to_vec();
        expanded.push(DesignPoint::parse(&label)?);
        let next = audit_design(&expanded, tolerance)?;
        let new_idim = n_coded.saturating_sub(next.main_effects_rank);
        let kind = if under_supported.contains(&label) {
            NextCornerKind::UnderSupported
        } else {
            NextCornerKind::NeverSeen
        };
        ranked.push(NextCornerCandidate {
            corner: label,
            kind,
            identified_set_reduction: current_idim.saturating_sub(new_idim),
            new_identified_set_dimension: new_idim,
            lack_of_fit_gain: next
                .lack_of_fit_dimension
                .saturating_sub(current.lack_of_fit_dimension),
            new_lack_of_fit_dimension: next.lack_of_fit_dimension,
            cost,
        });
    }
    sort_next_corners(&mut ranked);
    Ok(ranked)
}

fn sort_next_corners(ranked: &mut [NextCornerCandidate]) {
    ranked.sort_by(|left, right| {
        // reduction/cost then lof_gain/cost, via cross-multiply; then label.
        let left_id = left.identified_set_reduction as u128 * u128::from(right.cost);
        let right_id = right.identified_set_reduction as u128 * u128::from(left.cost);
        let left_lof = left.lack_of_fit_gain as u128 * u128::from(right.cost);
        let right_lof = right.lack_of_fit_gain as u128 * u128::from(left.cost);
        right_id
            .cmp(&left_id)
            .then(right_lof.cmp(&left_lof))
            .then(left.corner.cmp(&right.corner))
    });
}

/// Rectangle contrast `h(a',b') - h(a',b) - h(a,b') + h(a,b)` on supplied log-laws.
pub fn rectangle_contrast(
    face: &RectangleFace,
    log_laws: &BTreeMap<Vec<u32>, f64>,
) -> Result<f64, DesignError> {
    let cells = rectangle_cells(face);
    let mut values = [0.0; 4];
    for (index, cell) in cells.iter().enumerate() {
        values[index] = *log_laws.get(cell).ok_or_else(|| {
            DesignError::MissingLaw(
                cell.iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            )
        })?;
        if !values[index].is_finite() {
            return Err(DesignError::InvalidTolerance(values[index]));
        }
    }
    let [hab, ha_b, hab_, ha_b_] = values;
    Ok(ha_b_ - ha_b - hab_ + hab)
}

fn rectangle_cells(face: &RectangleFace) -> [Vec<u32>; 4] {
    let dim = face
        .held
        .iter()
        .map(|(index, _)| *index)
        .chain([face.first, face.second])
        .max()
        .unwrap_or(0)
        + 1;
    let mut base = vec![0_u32; dim];
    for (index, level) in &face.held {
        base[*index] = *level;
    }
    let [a, a_prime] = face.first_levels;
    let [b, b_prime] = face.second_levels;
    let mut hab = base.clone();
    hab[face.first] = a;
    hab[face.second] = b;
    let mut ha_b = base.clone();
    ha_b[face.first] = a_prime;
    ha_b[face.second] = b;
    let mut hab_ = base.clone();
    hab_[face.first] = a;
    hab_[face.second] = b_prime;
    let mut ha_b_ = base;
    ha_b_[face.first] = a_prime;
    ha_b_[face.second] = b_prime;
    [hab, ha_b, hab_, ha_b_]
}

/// Classifies a multi-level product design without evaluating a causal fiber.
pub fn classify_multilevel_family(
    points: &[MultiLevelPoint],
    cardinalities: &[u32],
    same_target_tilt_count: usize,
    tolerance: f64,
) -> Result<ObservedFamilyClassification, DesignError> {
    let matrix = multilevel_main_effects_matrix(points, cardinalities)?;
    let n_coded_columns = matrix.first().map_or(0, Vec::len);
    let main_effects_rank = matrix_rank(matrix, tolerance)?;
    let identified_set_dimension = n_coded_columns.saturating_sub(main_effects_rank);
    let lack_of_fit_dimension = points.len().saturating_sub(main_effects_rank);
    let orientation = orientation_testability(same_target_tilt_count);
    let next = rank_missing_multilevel_cells_with_costs(
        points,
        cardinalities,
        &BTreeMap::new(),
        tolerance,
    )?;
    let note = if orientation == OrientationTestability::Untestable {
        "multi-level geometry does not identify a unique local normalized potential system; orientation is untestable without a same-target tilt family"
    } else {
        "multi-level geometry does not identify a unique local normalized potential system"
    };
    Ok(ObservedFamilyClassification {
        completion_evaluation: CausalCompletionEvaluation::NotEvaluated,
        orientation,
        identified_set_dimension,
        lack_of_fit_dimension,
        main_effects_rank,
        n_coded_columns,
        missing_primitive_corners: Vec::new(),
        recommended_next_corner: next.first().map(|item| item.corner.clone()),
        next_corner_identified_set_reduction: next
            .first()
            .map(|item| item.identified_set_reduction),
        next_corner_cost: next.first().map(|item| item.cost),
        note: note.to_string(),
    })
}

/// Ranks unobserved multi-level cells by identified-set reduction per unit cost.
pub fn rank_missing_multilevel_cells(
    points: &[MultiLevelPoint],
    cardinalities: &[u32],
    tolerance: f64,
) -> Result<Vec<NextCornerCandidate>, DesignError> {
    rank_missing_multilevel_cells_with_costs(points, cardinalities, &BTreeMap::new(), tolerance)
}

/// Same ranking with optional positive integer costs (default 1000).
pub fn rank_missing_multilevel_cells_with_costs(
    points: &[MultiLevelPoint],
    cardinalities: &[u32],
    costs: &BTreeMap<String, u32>,
    tolerance: f64,
) -> Result<Vec<NextCornerCandidate>, DesignError> {
    let n_cells: usize = cardinalities
        .iter()
        .map(|&levels| levels as usize)
        .try_fold(1_usize, usize::checked_mul)
        .unwrap_or(usize::MAX);
    if n_cells > 64 {
        return Ok(Vec::new());
    }
    let current = multilevel_main_effects_matrix(points, cardinalities)?;
    let n_coded = current.first().map_or(0, Vec::len);
    let current_rank = matrix_rank(current, tolerance)?;
    let current_idim = n_coded.saturating_sub(current_rank);
    let current_lof = points.len().saturating_sub(current_rank);
    let seen: BTreeSet<Vec<u32>> = points.iter().map(|point| point.levels.clone()).collect();
    let mut ranked = Vec::new();
    for cell in product_cells(cardinalities) {
        if seen.contains(&cell) {
            continue;
        }
        let label = cell
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let cost = *costs.get(&label).unwrap_or(&1000);
        if cost == 0 {
            return Err(DesignError::InvalidCost {
                corner: label,
                cost,
            });
        }
        let mut expanded = points.to_vec();
        expanded.push(MultiLevelPoint::new(cell)?);
        let matrix = multilevel_main_effects_matrix(&expanded, cardinalities)?;
        let rank = matrix_rank(matrix, tolerance)?;
        let new_idim = n_coded.saturating_sub(rank);
        let new_lof = expanded.len().saturating_sub(rank);
        ranked.push(NextCornerCandidate {
            corner: label,
            kind: NextCornerKind::NeverSeen,
            identified_set_reduction: current_idim.saturating_sub(new_idim),
            new_identified_set_dimension: new_idim,
            lack_of_fit_gain: new_lof.saturating_sub(current_lof),
            new_lack_of_fit_dimension: new_lof,
            cost,
        });
    }
    sort_next_corners(&mut ranked);
    Ok(ranked)
}

fn product_cells(cardinalities: &[u32]) -> Vec<Vec<u32>> {
    let mut cells = vec![Vec::new()];
    for &levels in cardinalities {
        let mut next = Vec::new();
        for prefix in &cells {
            for level in 0..levels {
                let mut row = prefix.clone();
                row.push(level);
                next.push(row);
            }
        }
        cells = next;
    }
    cells
}

fn is_two_root_diagonal(points: &[DesignPoint]) -> bool {
    if points.len() != 2 || points[0].dimension() != 2 {
        return false;
    }
    let labels: BTreeSet<String> = points.iter().map(DesignPoint::bit_string).collect();
    labels.contains("00") && labels.contains("11")
}

fn missing_two_factor_primitives(points: &[DesignPoint]) -> Vec<String> {
    if points.first().is_none_or(|point| point.dimension() != 2) {
        return Vec::new();
    }
    let seen: BTreeSet<String> = points.iter().map(DesignPoint::bit_string).collect();
    ["10", "01"]
        .into_iter()
        .filter(|corner| !seen.contains(*corner))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn bool_point(bits: &str) -> DesignPoint {
        DesignPoint::parse(bits).unwrap()
    }

    fn ml(levels: &[u32]) -> MultiLevelPoint {
        MultiLevelPoint::new(levels.to_vec()).unwrap()
    }

    #[test]
    fn two_by_three_rectangle_is_enumerated() {
        let mut points = Vec::new();
        for a in 0..2_u32 {
            for b in 0..3_u32 {
                points.push(ml(&[a, b]));
            }
        }
        let faces = enumerate_rectangles(&points, &[2, 3]).unwrap();
        assert_eq!(faces.len(), 3);
        assert!(faces.iter().all(|face| face.first == 0 && face.second == 1));
    }

    #[test]
    fn boolean_square_recovers_one_rectangle() {
        let points = [ml(&[0, 0]), ml(&[1, 0]), ml(&[0, 1]), ml(&[1, 1])];
        let faces = enumerate_rectangles(&points, &[2, 2]).unwrap();
        assert_eq!(faces.len(), 1);
        let matrix = multilevel_main_effects_matrix(&points, &[2, 2]).unwrap();
        assert_eq!(matrix.len(), 4);
        assert_eq!(matrix[0], vec![1.0, 0.0, 0.0]);
        assert_eq!(matrix[3], vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn multilevel_rejects_level_out_of_range() {
        let error = multilevel_main_effects_matrix(&[ml(&[0, 3])], &[2, 2]).unwrap_err();
        assert!(matches!(
            error,
            DesignError::LevelOutOfRange { factor: 1, .. }
        ));
    }

    #[test]
    fn multilevel_rejects_singleton_cardinality() {
        let error = multilevel_main_effects_matrix(&[ml(&[0])], &[1]).unwrap_err();
        assert!(matches!(error, DesignError::InvalidCardinality { .. }));
    }

    #[test]
    fn catalog_square_without_tilts_is_untestable_for_orientation() {
        let points = [
            bool_point("00"),
            bool_point("10"),
            bool_point("01"),
            bool_point("11"),
        ];
        let report = classify_observed_family(
            FamilyClassificationInput {
                points: &points,
                same_target_tilt_count: 0,
                distinct_root_targets: true,
                baseline_combo_laws: None,
            },
            1e-12,
        )
        .unwrap();
        assert_eq!(report.orientation, OrientationTestability::Untestable);
        assert_eq!(
            report.completion_evaluation,
            CausalCompletionEvaluation::NotEvaluated
        );
        assert!(report.missing_primitive_corners.is_empty());
        assert_eq!(report.identified_set_dimension, 0);
        assert!(report.note.contains("orientation is untestable"));
    }

    #[test]
    fn declared_tilts_make_orientation_testable_but_not_unique() {
        let points = [
            bool_point("00"),
            bool_point("10"),
            bool_point("01"),
            bool_point("11"),
        ];
        let report = classify_observed_family(
            FamilyClassificationInput {
                points: &points,
                same_target_tilt_count: 2,
                distinct_root_targets: true,
                baseline_combo_laws: None,
            },
            1e-12,
        )
        .unwrap();
        assert_eq!(
            report.orientation,
            OrientationTestability::TestableWithDeclaredTilts
        );
        assert_eq!(
            report.completion_evaluation,
            CausalCompletionEvaluation::NotEvaluated
        );
    }

    #[test]
    fn diagonal_with_correlated_combo_is_infeasible() {
        let points = [bool_point("00"), bool_point("11")];
        let report = classify_observed_family(
            FamilyClassificationInput {
                points: &points,
                same_target_tilt_count: 0,
                distinct_root_targets: true,
                baseline_combo_laws: Some(([0.25; 4], [0.4, 0.1, 0.1, 0.4])),
            },
            1e-12,
        )
        .unwrap();
        assert_eq!(
            report.completion_evaluation,
            CausalCompletionEvaluation::Infeasible
        );
        assert_eq!(report.orientation, OrientationTestability::Untestable);
        assert_eq!(report.lack_of_fit_dimension, 0);
        assert_eq!(report.missing_primitive_corners, ["10", "01"]);
    }

    #[test]
    fn diagonal_without_laws_is_not_flat_therefore_modular() {
        let points = [bool_point("00"), bool_point("11")];
        let report = classify_observed_family(
            FamilyClassificationInput {
                points: &points,
                same_target_tilt_count: 0,
                distinct_root_targets: true,
                baseline_combo_laws: None,
            },
            1e-12,
        )
        .unwrap();
        assert_eq!(
            report.completion_evaluation,
            CausalCompletionEvaluation::NotEvaluated
        );
        assert_eq!(report.lack_of_fit_dimension, 0);
        assert!(report.note.contains("vacuous flatness"));
    }

    #[test]
    fn diagonal_recommends_a_primitive_that_kills_the_identified_set() {
        let points = [bool_point("00"), bool_point("11")];
        let ranked = rank_missing_boolean_corners(&points, 1e-12).unwrap();
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].identified_set_reduction, 1);
        assert_eq!(ranked[0].new_identified_set_dimension, 0);
        assert!(
            ranked
                .iter()
                .all(|item| item.corner == "01" || item.corner == "10")
        );
        let report = classify_observed_family(
            FamilyClassificationInput {
                points: &points,
                same_target_tilt_count: 0,
                distinct_root_targets: true,
                baseline_combo_laws: None,
            },
            1e-12,
        )
        .unwrap();
        assert_eq!(report.identified_set_dimension, 1);
        assert!(matches!(
            report.recommended_next_corner.as_deref(),
            Some("01" | "10")
        ));
        assert_eq!(report.next_corner_identified_set_reduction, Some(1));
    }

    #[test]
    fn dropped_boolean_corner_is_tagged_under_supported() {
        let mut rows = Vec::new();
        rows.extend(std::iter::repeat_n(vec![false, false], 8));
        rows.extend(std::iter::repeat_n(vec![true, false], 8));
        rows.extend(std::iter::repeat_n(vec![false, true], 8));
        rows.push(vec![true, true]);
        let observed = crate::observed_design_from_rows(&rows, 5).unwrap();
        let ranked =
            rank_missing_boolean_corners_from_observed(&observed, &BTreeMap::new(), 1e-12).unwrap();
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].corner, "11");
        assert_eq!(ranked[0].kind, NextCornerKind::UnderSupported);
    }

    #[test]
    fn three_corner_ell_recommends_the_missing_interaction_cell() {
        let points = [bool_point("00"), bool_point("10"), bool_point("01")];
        let ranked = rank_missing_boolean_corners(&points, 1e-12).unwrap();
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].corner, "11");
        assert_eq!(ranked[0].identified_set_reduction, 0);
        assert_eq!(ranked[0].lack_of_fit_gain, 1);
    }

    #[test]
    fn complete_square_has_no_next_corner() {
        let points = [
            bool_point("00"),
            bool_point("10"),
            bool_point("01"),
            bool_point("11"),
        ];
        assert!(
            rank_missing_boolean_corners(&points, 1e-12)
                .unwrap()
                .is_empty()
        );
        let report = classify_observed_family(
            FamilyClassificationInput {
                points: &points,
                same_target_tilt_count: 0,
                distinct_root_targets: true,
                baseline_combo_laws: None,
            },
            1e-12,
        )
        .unwrap();
        assert_eq!(report.identified_set_dimension, 0);
        assert!(report.recommended_next_corner.is_none());
    }

    #[test]
    fn multilevel_partial_grid_recommends_a_missing_cell() {
        let points = [ml(&[0, 0]), ml(&[1, 0]), ml(&[0, 1])];
        let report = classify_multilevel_family(&points, &[2, 3], 0, 1e-12).unwrap();
        assert_eq!(
            report.completion_evaluation,
            CausalCompletionEvaluation::NotEvaluated
        );
        assert_eq!(report.orientation, OrientationTestability::Untestable);
        assert!(report.recommended_next_corner.is_some());
        assert!(report.identified_set_dimension > 0 || report.recommended_next_corner.is_some());
    }

    #[test]
    fn expensive_primitive_loses_to_the_cheap_one() {
        let points = [bool_point("00"), bool_point("11")];
        let mut costs = BTreeMap::new();
        costs.insert("01".into(), 1000);
        costs.insert("10".into(), 50_000);
        let ranked = rank_missing_boolean_corners_with_costs(&points, &costs, 1e-12).unwrap();
        assert_eq!(ranked[0].corner, "01");
        assert_eq!(ranked[0].cost, 1000);
        assert_eq!(ranked[1].corner, "10");
        let error = rank_missing_boolean_corners_with_costs(
            &points,
            &BTreeMap::from([("01".into(), 0)]),
            1e-12,
        )
        .unwrap_err();
        assert!(matches!(error, DesignError::InvalidCost { .. }));
    }

    #[test]
    fn expensive_multilevel_cell_loses_to_the_cheap_one() {
        let points = [ml(&[0, 0]), ml(&[1, 0]), ml(&[0, 1])];
        let mut costs = BTreeMap::new();
        for cell in ["1,1", "0,2", "1,2"] {
            costs.insert(cell.into(), 1000);
        }
        costs.insert("1,1".into(), 80_000);
        let ranked =
            rank_missing_multilevel_cells_with_costs(&points, &[2, 3], &costs, 1e-12).unwrap();
        assert!(!ranked.is_empty());
        assert_ne!(ranked[0].corner, "1,1");
        assert_eq!(
            ranked
                .iter()
                .find(|item| item.corner == "1,1")
                .unwrap()
                .cost,
            80_000
        );
    }

    #[test]
    fn rectangle_contrast_recovers_the_boolean_interaction() {
        let face = RectangleFace {
            first: 0,
            second: 1,
            first_levels: [0, 1],
            second_levels: [0, 1],
            held: Vec::new(),
        };
        let mut laws = BTreeMap::new();
        laws.insert(vec![0, 0], 0.0);
        laws.insert(vec![1, 0], 0.0);
        laws.insert(vec![0, 1], 0.0);
        laws.insert(vec![1, 1], 1.0);
        assert!((rectangle_contrast(&face, &laws).unwrap() - 1.0).abs() < 1e-15);
        laws.remove(&vec![1, 1]);
        assert!(matches!(
            rectangle_contrast(&face, &laws),
            Err(DesignError::MissingLaw(_))
        ));
    }

    #[test]
    fn rectangle_law_audit_is_diagnostic_and_detects_curvature() {
        let points = [ml(&[0, 0]), ml(&[1, 0]), ml(&[0, 1]), ml(&[1, 1])];
        let mut laws = BTreeMap::new();
        laws.insert(vec![0, 0], 0.0);
        laws.insert(vec![1, 0], 0.0);
        laws.insert(vec![0, 1], 0.0);
        laws.insert(vec![1, 1], 0.0);
        let flat = audit_rectangle_laws(&points, &[2, 2], &laws, 1e-12).unwrap();
        assert!(flat.all_flat);
        assert_eq!(flat.n_evaluated, 1);
        assert!(flat.note.contains("not a modularity certificate"));
        laws.insert(vec![1, 1], 1.0);
        let curved = audit_rectangle_laws(&points, &[2, 2], &laws, 1e-12).unwrap();
        assert!(!curved.all_flat);
        assert!((curved.max_abs_contrast.unwrap() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn rectangle_law_audit_cannot_call_a_partial_law_map_all_flat() {
        let points = (0..2)
            .flat_map(|first| (0..3).map(move |second| ml(&[first, second])))
            .collect::<Vec<_>>();
        let laws = [([0, 0], 0.0), ([1, 0], 0.0), ([0, 1], 0.0), ([1, 1], 0.0)]
            .into_iter()
            .map(|(levels, value)| (levels.to_vec(), value))
            .collect::<BTreeMap<_, _>>();
        let audit = audit_rectangle_laws(&points, &[2, 3], &laws, 1e-12).unwrap();
        assert_eq!(audit.n_observed_rectangles, 3);
        assert_eq!(audit.n_evaluated, 1);
        assert_eq!(audit.n_missing_laws, 2);
        assert!(!audit.all_flat);
        assert!(audit.note.contains("not all observed rectangles"));
    }
}
