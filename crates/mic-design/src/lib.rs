#![forbid(unsafe_code)]
//! Factorial design geometry, sampling-odds gates, and estimability audits.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Design-audit errors.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum DesignError {
    /// Bit strings in a design had inconsistent dimensions.
    #[error("design point dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected design dimension.
        expected: usize,
        /// Observed design dimension.
        actual: usize,
    },
    /// A design probability was invalid.
    #[error("corner probability {index} must be finite and positive, got {value}")]
    InvalidProbability {
        /// Corner index in the canonical ordering.
        index: usize,
        /// Rejected probability value.
        value: f64,
    },
    /// A numerical tolerance was invalid.
    #[error("tolerance must be finite and nonnegative, got {0}")]
    InvalidTolerance(f64),
    /// A bit-string parser found an invalid character.
    #[error("invalid design bit {ch:?} at position {index}")]
    InvalidBit {
        /// Rejected character.
        ch: char,
        /// Position of the character in the bit string.
        index: usize,
    },
    /// A design point contained no coordinates.
    #[error("design point must contain at least one bit")]
    EmptyPoint,
    /// The design is empty.
    #[error("design must not be empty")]
    EmptyDesign,
    /// The same corner occurred more than once.
    #[error("duplicate design corner {0}")]
    DuplicateCorner(String),
    /// A supplied matrix was ragged.
    #[error("matrix rows must have equal length")]
    RaggedMatrix,
}

/// One Boolean factorial design corner.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct DesignPoint {
    /// Intervention activation bits.
    pub bits: Vec<bool>,
}

impl DesignPoint {
    /// Constructs a point from bits.
    pub fn new(bits: Vec<bool>) -> Result<Self, DesignError> {
        if bits.is_empty() {
            return Err(DesignError::EmptyPoint);
        }
        Ok(Self { bits })
    }

    /// Parses strings such as `0101`.
    pub fn parse(value: &str) -> Result<Self, DesignError> {
        if value.is_empty() {
            return Err(DesignError::EmptyPoint);
        }
        let mut bits = Vec::with_capacity(value.len());
        for (index, ch) in value.chars().enumerate() {
            match ch {
                '0' => bits.push(false),
                '1' => bits.push(true),
                _ => return Err(DesignError::InvalidBit { ch, index }),
            }
        }
        Ok(Self { bits })
    }

    /// Returns the number of design coordinates.
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.bits.len()
    }

    /// Returns a compact bit string.
    #[must_use]
    pub fn bit_string(&self) -> String {
        self.bits
            .iter()
            .map(|bit| if *bit { '1' } else { '0' })
            .collect()
    }

    /// Returns a copy with one coordinate flipped.
    #[must_use]
    pub fn flipped(&self, coordinate: usize) -> Self {
        let mut bits = self.bits.clone();
        bits[coordinate] = !bits[coordinate];
        Self { bits }
    }
}

/// A fully observed two-dimensional face.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SquareFace {
    /// Corner with both selected coordinates set to zero.
    pub base: DesignPoint,
    /// First varying coordinate.
    pub first: usize,
    /// Second varying coordinate.
    pub second: usize,
}

impl SquareFace {
    /// Returns corners in order `00, 10, 01, 11` relative to the face coordinates.
    #[must_use]
    pub fn corners(&self) -> [DesignPoint; 4] {
        let p00 = self.base.clone();
        let p10 = p00.flipped(self.first);
        let p01 = p00.flipped(self.second);
        let p11 = p10.flipped(self.second);
        [p00, p10, p01, p11]
    }
}

/// Product-odds audit for a four-corner pooled design.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SamplingOddsAudit {
    /// Input positive masses in order `00, 10, 01, 11`.
    pub probabilities: [f64; 4],
    /// Sum of the supplied masses.
    pub probability_sum: f64,
    /// `log((rho11 rho00)/(rho10 rho01))`.
    pub log_odds_ratio: f64,
    /// Whether the log odds ratio is within tolerance of zero.
    pub is_product: bool,
    /// Absolute tolerance used for the decision.
    pub tolerance: f64,
}

/// Audits whether four positive corner masses have product odds.
pub fn audit_sampling_odds(
    probabilities: [f64; 4],
    tolerance: f64,
) -> Result<SamplingOddsAudit, DesignError> {
    validate_tolerance(tolerance)?;
    for (index, value) in probabilities.iter().copied().enumerate() {
        if !value.is_finite() || value <= 0.0 {
            return Err(DesignError::InvalidProbability { index, value });
        }
    }
    let [p00, p10, p01, p11] = probabilities;
    let log_or = (p11 * p00 / (p10 * p01)).ln();
    Ok(SamplingOddsAudit {
        probabilities,
        probability_sum: probabilities.iter().sum(),
        log_odds_ratio: log_or,
        is_product: log_or.abs() <= tolerance,
        tolerance,
    })
}

/// Summary of main-effects estimability on an observed design.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesignAudit {
    /// Number of observed corners.
    pub corner_count: usize,
    /// Number of intervention coordinates.
    pub dimension: usize,
    /// Rank of the intercept-plus-main-effects matrix.
    pub main_effects_rank: usize,
    /// Dimension of the pointwise lack-of-fit space.
    pub lack_of_fit_dimension: usize,
    /// Deterministic basis of contrasts orthogonal to all main effects.
    pub lack_of_fit_basis: Vec<Vec<f64>>,
    /// Fully observed square faces.
    pub square_faces: Vec<SquareFace>,
    /// Rank of the observed square-contrast vectors.
    pub square_contrast_rank: usize,
    /// Whether observed square contrasts span every testable flatness restriction.
    pub squares_span_lack_of_fit: bool,
}

/// Audits an arbitrary observed Boolean design.
pub fn audit_design(points: &[DesignPoint], tolerance: f64) -> Result<DesignAudit, DesignError> {
    validate_tolerance(tolerance)?;
    validate_points(points)?;
    let dimension = points[0].dimension();
    let matrix = main_effects_matrix(points);
    let rank = matrix_rank(matrix.clone(), tolerance)?;
    let lack_of_fit_basis = null_space_basis(transpose(&matrix)?, tolerance)?;
    let faces = enumerate_square_faces(points)?;
    let square_vectors = square_contrast_vectors(points, &faces);
    let square_rank = matrix_rank(square_vectors, tolerance)?;
    let lack = points.len().saturating_sub(rank);
    debug_assert_eq!(lack, lack_of_fit_basis.len());
    Ok(DesignAudit {
        corner_count: points.len(),
        dimension,
        main_effects_rank: rank,
        lack_of_fit_dimension: lack,
        lack_of_fit_basis,
        square_faces: faces,
        square_contrast_rank: square_rank,
        squares_span_lack_of_fit: square_rank == lack,
    })
}

/// Builds the intercept-plus-main-effects design matrix.
#[must_use]
pub fn main_effects_matrix(points: &[DesignPoint]) -> Vec<Vec<f64>> {
    points
        .iter()
        .map(|point| {
            let mut row = Vec::with_capacity(point.dimension() + 1);
            row.push(1.0);
            row.extend(point.bits.iter().map(|bit| if *bit { 1.0 } else { 0.0 }));
            row
        })
        .collect()
}

/// Enumerates each fully observed square exactly once.
pub fn enumerate_square_faces(points: &[DesignPoint]) -> Result<Vec<SquareFace>, DesignError> {
    if points.is_empty() {
        return Ok(Vec::new());
    }
    validate_points(points)?;
    let dimension = points[0].dimension();
    let observed: BTreeSet<DesignPoint> = points.iter().cloned().collect();
    let mut faces = BTreeMap::<(DesignPoint, usize, usize), SquareFace>::new();
    for point in points {
        for first in 0..dimension {
            if point.bits[first] {
                continue;
            }
            for second in (first + 1)..dimension {
                if point.bits[second] {
                    continue;
                }
                let face = SquareFace {
                    base: point.clone(),
                    first,
                    second,
                };
                if face
                    .corners()
                    .iter()
                    .all(|corner| observed.contains(corner))
                {
                    faces.insert((point.clone(), first, second), face);
                }
            }
        }
    }
    Ok(faces.into_values().collect())
}

/// Builds one coefficient vector per observed square in point order.
#[must_use]
pub fn square_contrast_vectors(points: &[DesignPoint], faces: &[SquareFace]) -> Vec<Vec<f64>> {
    let index: BTreeMap<DesignPoint, usize> = points
        .iter()
        .cloned()
        .enumerate()
        .map(|(position, point)| (point, position))
        .collect();
    faces
        .iter()
        .map(|face| {
            let mut contrast = vec![0.0; points.len()];
            let [p00, p10, p01, p11] = face.corners();
            contrast[index[&p00]] = 1.0;
            contrast[index[&p10]] = -1.0;
            contrast[index[&p01]] = -1.0;
            contrast[index[&p11]] = 1.0;
            contrast
        })
        .collect()
}

/// Evaluates a square contrast in order `h11 + h00 - h10 - h01`.
#[must_use]
pub fn square_contrast(values: [f64; 4]) -> f64 {
    let [h00, h10, h01, h11] = values;
    h11 + h00 - h10 - h01
}

/// Deterministic row rank for small dense matrices.
pub fn matrix_rank(matrix: Vec<Vec<f64>>, tolerance: f64) -> Result<usize, DesignError> {
    validate_tolerance(tolerance)?;
    let (_, pivots) = rref(matrix, tolerance)?;
    Ok(pivots.len())
}

/// Deterministic basis for the right null space of a small dense matrix.
pub fn null_space_basis(
    matrix: Vec<Vec<f64>>,
    tolerance: f64,
) -> Result<Vec<Vec<f64>>, DesignError> {
    validate_tolerance(tolerance)?;
    if matrix.is_empty() {
        return Ok(Vec::new());
    }
    let columns = matrix[0].len();
    let (reduced, pivots) = rref(matrix, tolerance)?;
    let pivot_set: BTreeSet<usize> = pivots.iter().copied().collect();
    let free_columns: Vec<usize> = (0..columns)
        .filter(|column| !pivot_set.contains(column))
        .collect();
    let mut basis = Vec::with_capacity(free_columns.len());
    for free in free_columns {
        let mut vector = vec![0.0; columns];
        vector[free] = 1.0;
        for (row, &pivot) in pivots.iter().enumerate() {
            vector[pivot] = -reduced[row][free];
        }
        canonicalize_vector(&mut vector, tolerance);
        basis.push(vector);
    }
    Ok(basis)
}

fn rref(
    mut matrix: Vec<Vec<f64>>,
    tolerance: f64,
) -> Result<(Vec<Vec<f64>>, Vec<usize>), DesignError> {
    if matrix.is_empty() {
        return Ok((matrix, Vec::new()));
    }
    let columns = matrix[0].len();
    if matrix.iter().any(|row| row.len() != columns) {
        return Err(DesignError::RaggedMatrix);
    }
    let rows = matrix.len();
    let mut pivot_row = 0;
    let mut pivots = Vec::new();
    for column in 0..columns {
        if pivot_row >= rows {
            break;
        }
        let candidate = (pivot_row..rows)
            .max_by(|&left, &right| {
                matrix[left][column]
                    .abs()
                    .total_cmp(&matrix[right][column].abs())
            })
            .expect("nonempty pivot range");
        if matrix[candidate][column].abs() <= tolerance {
            continue;
        }
        matrix.swap(pivot_row, candidate);
        let pivot = matrix[pivot_row][column];
        for value in &mut matrix[pivot_row][column..] {
            *value /= pivot;
        }
        for row in 0..rows {
            if row == pivot_row {
                continue;
            }
            let factor = matrix[row][column];
            if factor.abs() <= tolerance {
                continue;
            }
            // Reads pivot_row while mutating row, so an iterator form would need split_at_mut.
            #[allow(clippy::needless_range_loop)]
            for current in column..columns {
                matrix[row][current] -= factor * matrix[pivot_row][current];
                if matrix[row][current].abs() <= tolerance {
                    matrix[row][current] = 0.0;
                }
            }
        }
        pivots.push(column);
        pivot_row += 1;
    }
    Ok((matrix, pivots))
}

fn transpose(matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, DesignError> {
    if matrix.is_empty() {
        return Ok(Vec::new());
    }
    let columns = matrix[0].len();
    if matrix.iter().any(|row| row.len() != columns) {
        return Err(DesignError::RaggedMatrix);
    }
    let mut transposed = vec![vec![0.0; matrix.len()]; columns];
    for (row_index, row) in matrix.iter().enumerate() {
        for (column_index, &value) in row.iter().enumerate() {
            transposed[column_index][row_index] = value;
        }
    }
    Ok(transposed)
}

fn canonicalize_vector(vector: &mut [f64], tolerance: f64) {
    let norm = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
    if norm > tolerance {
        for value in vector.iter_mut() {
            *value /= norm;
            if value.abs() <= tolerance {
                *value = 0.0;
            }
        }
    }
    if let Some(first) = vector.iter().copied().find(|value| value.abs() > tolerance)
        && first < 0.0
    {
        for value in vector {
            *value = -*value;
        }
    }
}

fn validate_points(points: &[DesignPoint]) -> Result<(), DesignError> {
    if points.is_empty() {
        return Err(DesignError::EmptyDesign);
    }
    let dimension = points[0].dimension();
    if dimension == 0 {
        return Err(DesignError::EmptyPoint);
    }
    let mut seen = BTreeSet::new();
    for point in points {
        if point.dimension() != dimension {
            return Err(DesignError::DimensionMismatch {
                expected: dimension,
                actual: point.dimension(),
            });
        }
        if !seen.insert(point.clone()) {
            return Err(DesignError::DuplicateCorner(point.bit_string()));
        }
    }
    Ok(())
}

fn validate_tolerance(tolerance: f64) -> Result<(), DesignError> {
    if tolerance.is_finite() && tolerance >= 0.0 {
        Ok(())
    } else {
        Err(DesignError::InvalidTolerance(tolerance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_corner_sampling_is_product() {
        let audit = audit_sampling_odds([0.25; 4], 1e-12).unwrap();
        assert!(audit.is_product);
        assert_eq!(audit.log_odds_ratio, 0.0);
        assert_eq!(audit.probability_sum, 1.0);
    }

    #[test]
    fn arbitrary_corner_quotas_can_fail_product_odds() {
        let audit = audit_sampling_odds([0.1, 0.2, 0.3, 0.4], 1e-12).unwrap();
        assert!(!audit.is_product);
    }

    #[test]
    fn complete_three_cube_has_six_square_faces_and_complete_square_span() {
        let points: Vec<_> = (0_u8..8)
            .map(|value| {
                DesignPoint::new((0..3).map(|bit| value & (1 << bit) != 0).collect()).unwrap()
            })
            .collect();
        let audit = audit_design(&points, 1e-12).unwrap();
        assert_eq!(audit.main_effects_rank, 4);
        assert_eq!(audit.lack_of_fit_dimension, 4);
        assert_eq!(audit.lack_of_fit_basis.len(), 4);
        assert_eq!(audit.square_faces.len(), 6);
        assert_eq!(audit.square_contrast_rank, 4);
        assert!(audit.squares_span_lack_of_fit);
    }

    #[test]
    fn antipodal_hole_design_has_no_squares_but_two_restrictions() {
        let labels = ["001", "010", "011", "100", "101", "110"];
        let points: Vec<_> = labels
            .iter()
            .map(|label| DesignPoint::parse(label).unwrap())
            .collect();
        let audit = audit_design(&points, 1e-12).unwrap();
        assert_eq!(audit.main_effects_rank, 4);
        assert_eq!(audit.lack_of_fit_dimension, 2);
        assert_eq!(audit.lack_of_fit_basis.len(), 2);
        assert!(audit.square_faces.is_empty());
        assert_eq!(audit.square_contrast_rank, 0);
        assert!(!audit.squares_span_lack_of_fit);
    }

    #[test]
    fn null_space_vectors_annihilate_matrix() {
        let matrix = vec![vec![1.0, 0.0, 1.0], vec![0.0, 1.0, 1.0]];
        let basis = null_space_basis(matrix.clone(), 1e-12).unwrap();
        assert_eq!(basis.len(), 1);
        for row in matrix {
            let dot: f64 = row.iter().zip(&basis[0]).map(|(a, b)| a * b).sum();
            assert!(dot.abs() < 1e-12);
        }
    }
}
