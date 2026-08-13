//! Multi-level factorial geometry and fail-closed family classification.
//!
//! Geometry never yields [`crate::ModularCompletionClass::Unique`]. That class
//! requires identified local, conditionally normalized potentials, which this
//! module does not invent from a design matrix.

use crate::{
    DesignError, DesignPoint, ModularCompletionClass, audit_design, classify_two_root_diagonal,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

/// Geometry-plus-witness classification. Never a certificate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservedFamilyClassification {
    /// Modular-completion class. Geometry alone never yields `unique`.
    pub modular_completion: ModularCompletionClass,
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
    /// Human-readable refusal or witness note.
    pub note: String,
}

/// Classifies an observed Boolean family.
///
/// Decided cases:
/// - `D = {00, 11}` with distinct roots and supplied laws → two-root diagonal witness
/// - any design with fewer than two declared same-target tilts → orientation untestable
///
/// `unique` is never returned. That class is reserved for a later theorem that
/// identifies local normalized potentials.
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
    let (modular_completion, note) = if diagonal
        && input.distinct_root_targets
        && let Some((baseline, combo)) = input.baseline_combo_laws
    {
        let class = classify_two_root_diagonal(baseline, combo, tolerance)?;
        let note = match class {
            ModularCompletionClass::Infeasible => {
                "D={00,11} with a non-product combo law cannot be realized by distinct root-mechanism replacements"
            }
            ModularCompletionClass::SetIdentified => {
                "D={00,11} product combo is feasible but primitives are unidentified"
            }
            ModularCompletionClass::Unique | ModularCompletionClass::Untestable => {
                "diagonal witness returned an unexpected class"
            }
        };
        (class, note.to_string())
    } else if diagonal {
        (
            ModularCompletionClass::Untestable,
            "D={00,11} has vacuous flatness; do not read modularity from a trivial lack-of-fit space".into(),
        )
    } else if orientation == OrientationTestability::Untestable
        && audit.square_faces.len() == 1
        && missing_primitive_corners.is_empty()
    {
        (
            ModularCompletionClass::Untestable,
            "complete catalog square with no same-target tilt family: orientation is untestable; modular completion is not unique".into(),
        )
    } else {
        (
            ModularCompletionClass::Untestable,
            "geometry does not identify a unique local normalized potential system".into(),
        )
    };
    debug_assert_ne!(modular_completion, ModularCompletionClass::Unique);
    Ok(ObservedFamilyClassification {
        modular_completion,
        orientation,
        identified_set_dimension,
        lack_of_fit_dimension: audit.lack_of_fit_dimension,
        main_effects_rank: audit.main_effects_rank,
        n_coded_columns,
        missing_primitive_corners,
        note,
    })
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
            report.modular_completion,
            ModularCompletionClass::Untestable
        );
        assert_ne!(report.modular_completion, ModularCompletionClass::Unique);
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
            report.modular_completion,
            ModularCompletionClass::Untestable
        );
        assert_ne!(report.modular_completion, ModularCompletionClass::Unique);
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
            report.modular_completion,
            ModularCompletionClass::Infeasible
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
            report.modular_completion,
            ModularCompletionClass::Untestable
        );
        assert_eq!(report.lack_of_fit_dimension, 0);
        assert!(report.note.contains("vacuous flatness"));
        assert_ne!(report.modular_completion, ModularCompletionClass::Unique);
    }
}
