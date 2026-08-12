#![forbid(unsafe_code)]
//! Unsupervised Stage 0–1 survey: column triage and a testability atlas.
//!
//! Authority is permanently `proposal_only`. Selection cannot be established
//! from rows, so this path never certifies.

use crate::EngineError;
use mic_data::{RawTable, load_raw_csv};
use mic_design::{
    DesignPoint, ObservedDesign, SamplingOddsAudit, audit_sampling_odds, observed_design_from_rows,
};
use mic_stats::simultaneous_mean_bounds;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Permanent authority of an unsupervised survey artifact.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SurveyAuthority {
    /// May decide what square to audit next. May not decide what is true.
    ProposalOnly,
}

/// How the randomization unit was chosen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClusterUnitBasis {
    /// Caller passed `--cluster`.
    Declared,
    /// Inferred from an identifier-shaped column.
    Inferred,
    /// No unit available; survey collapsed to rows and must say so.
    Row,
}

/// How a column was triaged.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnRole {
    /// Identifier-shaped; candidate randomization unit.
    ClusterCandidate,
    /// Low-cardinality assigned-looking factor; candidate design bit or encoding.
    ContextCandidate,
    /// Remaining numeric or high-cardinality features.
    StateCandidate,
    /// Constant or empty; recorded so nothing is dropped silently.
    Excluded,
}

/// One column after Stage 0 scoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColumnTriage {
    /// Header name.
    pub column: String,
    /// Assigned role.
    pub role: ColumnRole,
    /// Distinct non-empty values.
    pub n_unique: usize,
    /// Distinct values divided by row count.
    pub uniqueness: f64,
    /// Why the role was chosen.
    pub reason: String,
}

/// One discovered interferometer (a pair of context bits, or a bitstring column).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InterferometerProposal {
    /// Stable identifier.
    pub interferometer_id: String,
    /// Source columns that induce the bits.
    pub context_columns: Vec<String>,
    /// Observed design geometry.
    pub design: ObservedDesign,
    /// Product-odds audit when all four corners of a 2-factor square are present.
    pub sampling: Option<SamplingOddsAudit>,
    /// Whether a complete 2-factor square is observed.
    pub complete_square: bool,
    /// Factorial corners that never appeared in the table. Not the same as dropped.
    pub missing_corners: Vec<String>,
    /// Corners observed below `min_corner_count` and excluded from the design.
    pub dropped_corners: Vec<String>,
    /// Whether pooled odds are empirically product (estimated quotas, not known).
    pub empirically_product: bool,
    /// Smallest retained corner count.
    pub min_corner_count: usize,
    /// Ranking score: complete square first, then min corner count.
    pub priority: f64,
    /// Why this is or is not immediately auditable.
    pub note: String,
}

/// Stage 0–1 survey of a raw table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurveyReport {
    /// Schema version.
    pub schema_version: String,
    /// Always `proposal_only`.
    pub authority: SurveyAuthority,
    /// Why certification is impossible from rows alone.
    pub wall: String,
    /// File fingerprint.
    pub table_sha256: String,
    /// Resolved path.
    pub path: String,
    /// Row count.
    pub n_rows: usize,
    /// Column triage.
    pub columns: Vec<ColumnTriage>,
    /// Coarsest cluster-candidate column, if any.
    pub inferred_cluster_column: Option<String>,
    /// Whether the unit was declared, inferred, or fallen back to rows.
    pub cluster_unit_basis: ClusterUnitBasis,
    /// Ranked interferometers.
    pub interferometers: Vec<InterferometerProposal>,
    /// Suggested four-law manifest for the top complete square, if any.
    /// Authority remains `proposal_only`; selection is left `unknown`.
    pub suggested_manifest: Option<mic_data::ExperimentManifest>,
    /// Confirmation-split composability scout. Scout-only types; never a certificate.
    pub direction_scout: Option<DirectionScout>,
    /// Human-readable next action.
    pub next_step: String,
}

/// Whether a remaining-state contrast sits relative to the scout tolerance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoutInvariance {
    /// Entire interval is below tolerance.
    Below,
    /// Entire interval is above tolerance.
    Above,
    /// Interval overlaps the tolerance.
    Overlaps,
}

/// One state coordinate's remaining-contrast interval. Not a deletion certificate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoutContrast {
    /// State column that was left out of the remaining contrast.
    pub coordinate: String,
    /// Point relative discrepancy.
    pub relative_discrepancy: f64,
    /// Simultaneous lower bound.
    pub lower: f64,
    /// Simultaneous upper bound.
    pub upper: f64,
    /// Position versus the scout tolerance.
    pub status: ScoutInvariance,
}

/// Scout-only summary. Does not establish same-target family membership.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoutOutcome {
    /// One remaining contrast is below tolerance. Family membership is unproven.
    SingleCoordinateBelowTolerance {
        /// The coordinate.
        coordinate: String,
    },
    /// More than one remaining contrast is below tolerance.
    MultipleCoordinatesBelowTolerance {
        /// Those coordinates.
        coordinates: Vec<String>,
    },
    /// No remaining contrast is below tolerance.
    NoneBelowTolerance,
    /// Confirmation half is too small or the full contrast is too small.
    Underpowered,
    /// At least one interval overlaps the tolerance.
    IntervalOverlap {
        /// Unresolved coordinates.
        unresolved: Vec<String>,
    },
}

/// Proposal-only scout. Does not reuse certificate orientation types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DirectionScout {
    /// Always `proposal_only`.
    pub authority: SurveyAuthority,
    /// Interferometer that supplied the two corners compared.
    pub interferometer_id: String,
    /// Context bits compared. Not assumed to be two tilts of one mechanism.
    pub compared_columns: Vec<String>,
    /// State columns scored.
    pub state_columns: Vec<String>,
    /// Clusters used only to find the square.
    pub discovery_clusters: usize,
    /// Clusters used only to score remaining contrasts.
    pub confirmation_clusters: usize,
    /// Seed for the split and multiplier bounds.
    pub seed: u64,
    /// Preregistered tolerance on relative remaining contrast.
    pub epsilon: f64,
    /// Two-sample discrepancy between the compared corners on full state.
    pub full_discrepancy: f64,
    /// Scout-only outcome. Not `unique_target`.
    pub outcome: ScoutOutcome,
    /// Per-coordinate remaining contrasts.
    pub contrasts: Vec<ScoutContrast>,
    /// Why this is not an arrow.
    pub note: String,
}

/// Policy for the unsupervised survey.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SurveyPolicy {
    /// Maximum unique values for a context candidate.
    pub max_context_uniques: usize,
    /// Minimum unique values for a context candidate.
    pub min_context_uniques: usize,
    /// Minimum rows (or clusters) to retain a corner.
    pub min_corner_count: usize,
    /// Maximum context pairs to emit.
    pub max_pairs: usize,
}

impl Default for SurveyPolicy {
    fn default() -> Self {
        Self {
            max_context_uniques: 16,
            min_context_uniques: 2,
            min_corner_count: 2,
            max_pairs: 32,
        }
    }
}

/// Triages columns and lists testable squares. Does not estimate κ and does not certify.
pub fn run_unsupervised_survey(
    path: impl AsRef<Path>,
    base_dir: Option<&Path>,
    declared_cluster: Option<&str>,
    policy: SurveyPolicy,
) -> Result<SurveyReport, EngineError> {
    if policy.min_corner_count == 0 || policy.max_pairs == 0 {
        return Err(EngineError::InvalidTabular(
            "survey policy min_corner_count and max_pairs must be positive".into(),
        ));
    }
    let table = load_raw_csv(path, base_dir)?;
    let columns = triage_columns(&table, policy);
    let (cluster_column, cluster_unit_basis) = match declared_cluster {
        Some(name) => (Some(name.to_owned()), ClusterUnitBasis::Declared),
        None => match coarsest_cluster(&columns) {
            Some(name) => (Some(name), ClusterUnitBasis::Inferred),
            None => (None, ClusterUnitBasis::Row),
        },
    };
    let interferometers =
        discover_interferometers(&table, &columns, cluster_column.as_deref(), policy)?;
    let suggested_manifest = interferometers
        .iter()
        .find(|item| item.complete_square)
        .and_then(|item| suggested_four_law_manifest(&table, item, cluster_column.as_deref()));
    let direction_scout = interferometers
        .iter()
        .find(|item| item.complete_square)
        .and_then(|item| {
            scout_direction(
                &table,
                &columns,
                item,
                cluster_column.as_deref(),
                &table.content_sha256,
            )
            .ok()
            .flatten()
        });
    let next_step = survey_next_step(&interferometers, direction_scout.as_ref());
    Ok(SurveyReport {
        schema_version: "1.0.0".into(),
        authority: SurveyAuthority::ProposalOnly,
        wall: "State-independent within-regime selection cannot be established from observed rows. This survey cannot issue a certificate.".into(),
        table_sha256: table.content_sha256,
        path: table.path.display().to_string(),
        n_rows: table.rows.len(),
        columns,
        inferred_cluster_column: cluster_column,
        cluster_unit_basis,
        interferometers,
        suggested_manifest,
        direction_scout,
        next_step,
    })
}

fn triage_columns(table: &RawTable, policy: SurveyPolicy) -> Vec<ColumnTriage> {
    let n = table.rows.len().max(1) as f64;
    table
        .headers
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let uniques = unique_values(table, index);
            let n_unique = uniques.len();
            let uniqueness = n_unique as f64 / n;
            let lower = name.to_ascii_lowercase();
            let token_shaped = !column_is_numeric(&uniques);
            let looks_like_id = lower.contains("id")
                || lower.ends_with("_key")
                || lower.contains("uuid")
                || (uniqueness >= 0.98 && token_shaped);
            let constant = n_unique <= 1;
            let bitstring = looks_like_bitstring(&uniques);
            let (role, reason) = if constant {
                (
                    ColumnRole::Excluded,
                    "constant or empty; recorded so it is not silently dropped".into(),
                )
            } else if looks_like_id {
                (
                    ColumnRole::ClusterCandidate,
                    "identifier-shaped or nearly unique; candidate randomization unit".into(),
                )
            } else if bitstring {
                (
                    ColumnRole::ContextCandidate,
                    "values are equal-length bit strings; candidate encoded factorial design"
                        .into(),
                )
            } else if !token_shaped {
                (
                    ColumnRole::StateCandidate,
                    "numeric column; treated as state unless a human declares it as context. A 0/1 measurement can be an outcome or collider, not an assigned factor".into(),
                )
            } else if n_unique >= policy.min_context_uniques
                && n_unique <= policy.max_context_uniques
                && n_unique < table.rows.len()
            {
                (
                    ColumnRole::ContextCandidate,
                    format!("{n_unique} repeating token values; candidate assigned context factor"),
                )
            } else {
                (
                    ColumnRole::StateCandidate,
                    "high-cardinality remainder; treated as state".into(),
                )
            };
            ColumnTriage {
                column: name.clone(),
                role,
                n_unique,
                uniqueness,
                reason,
            }
        })
        .collect()
}

fn coarsest_cluster(columns: &[ColumnTriage]) -> Option<String> {
    columns
        .iter()
        .filter(|column| column.role == ColumnRole::ClusterCandidate)
        .min_by(|left, right| {
            left.n_unique
                .cmp(&right.n_unique)
                .then_with(|| left.column.cmp(&right.column))
        })
        .map(|column| column.column.clone())
}

fn discover_interferometers(
    table: &RawTable,
    columns: &[ColumnTriage],
    cluster_column: Option<&str>,
    policy: SurveyPolicy,
) -> Result<Vec<InterferometerProposal>, EngineError> {
    let context: Vec<&ColumnTriage> = columns
        .iter()
        .filter(|column| column.role == ColumnRole::ContextCandidate)
        .collect();
    let mut proposals = Vec::new();

    for column in &context {
        let values = unique_values(table, header_index(table, &column.column)?);
        if looks_like_bitstring(&values)
            && let Some(proposal) =
                bitstring_interferometer(table, &column.column, cluster_column, policy)?
        {
            proposals.push(proposal);
        }
    }

    let binary: Vec<&ColumnTriage> = context
        .iter()
        .copied()
        .filter(|column| column.n_unique == 2)
        .collect();
    for i in 0..binary.len() {
        for j in (i + 1)..binary.len() {
            if proposals.len() >= policy.max_pairs {
                break;
            }
            if let Some(proposal) = pair_interferometer(
                table,
                &binary[i].column,
                &binary[j].column,
                cluster_column,
                policy,
            )? {
                proposals.push(proposal);
            }
        }
    }

    proposals.sort_by(|left, right| {
        right
            .priority
            .partial_cmp(&left.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.interferometer_id.cmp(&right.interferometer_id))
    });
    proposals.truncate(policy.max_pairs);
    Ok(proposals)
}

fn bitstring_interferometer(
    table: &RawTable,
    column: &str,
    cluster_column: Option<&str>,
    policy: SurveyPolicy,
) -> Result<Option<InterferometerProposal>, EngineError> {
    let index = header_index(table, column)?;
    let assignments = cluster_collapsed_bits(table, cluster_column, |row| {
        DesignPoint::parse(&row[index]).ok().map(|point| point.bits)
    })?;
    propose(
        format!("bitstring:{column}"),
        vec![column.to_string()],
        &assignments,
        policy,
        "encoded factorial column; treat bits as candidate primitives, not as a certified design",
    )
}

fn pair_interferometer(
    table: &RawTable,
    first: &str,
    second: &str,
    cluster_column: Option<&str>,
    policy: SurveyPolicy,
) -> Result<Option<InterferometerProposal>, EngineError> {
    let i = header_index(table, first)?;
    let j = header_index(table, second)?;
    let first_levels = sorted_uniques(table, i);
    let second_levels = sorted_uniques(table, j);
    if first_levels.len() != 2 || second_levels.len() != 2 {
        return Ok(None);
    }
    let assignments = cluster_collapsed_bits(table, cluster_column, |row| {
        Some(vec![row[i] == first_levels[1], row[j] == second_levels[1]])
    })?;
    propose(
        format!("pair:{first}+{second}"),
        vec![first.to_string(), second.to_string()],
        &assignments,
        policy,
        "two binary context columns; a complete square is a candidate four-law interferometer",
    )
}

fn propose(
    interferometer_id: String,
    context_columns: Vec<String>,
    assignments: &[Vec<bool>],
    policy: SurveyPolicy,
    note: &str,
) -> Result<Option<InterferometerProposal>, EngineError> {
    if assignments.is_empty() {
        return Ok(None);
    }
    let design = match observed_design_from_rows(assignments, policy.min_corner_count) {
        Ok(design) => design,
        Err(mic_design::DesignError::EmptyDesign) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let complete_square =
        design.points.len() == 4 && design.points.iter().all(|point| point.dimension() == 2);
    let (missing_corners, dropped_corners) = classify_factorial_corners(&design);
    let sampling = if complete_square {
        let mut rho = [0.0; 4];
        for (point, proportion) in design.points.iter().zip(&design.proportions) {
            let slot = corner_slot(point);
            if let Some(index) = slot {
                rho[index] = *proportion;
            }
        }
        if rho.iter().all(|value| *value > 0.0) {
            Some(audit_sampling_odds(rho, 1e-10)?)
        } else {
            None
        }
    } else {
        None
    };
    let empirically_product = sampling.is_some_and(|audit| audit.is_product);
    let min_corner_count = design.counts.iter().copied().min().unwrap_or(0);
    let near_square = design
        .points
        .first()
        .is_some_and(|point| point.dimension() == 2)
        && design.points.len() == 3;
    // Do not rank on empirically_product: estimated quotas test the wrong null.
    let priority = f64::from(u32::from(complete_square)) * 1_000.0
        + f64::from(u32::from(near_square)) * 200.0
        + min_corner_count as f64
        - missing_corners.len() as f64
        - dropped_corners.len() as f64;
    let note = extension_note(note, &missing_corners, &dropped_corners);
    Ok(Some(InterferometerProposal {
        interferometer_id,
        context_columns,
        design,
        sampling,
        complete_square,
        missing_corners,
        dropped_corners,
        empirically_product,
        min_corner_count,
        priority,
        note,
    }))
}

fn survey_next_step(
    interferometers: &[InterferometerProposal],
    scout: Option<&DirectionScout>,
) -> String {
    if let Some(scout) = scout {
        return match &scout.outcome {
            ScoutOutcome::SingleCoordinateBelowTolerance { coordinate } => format!(
                "Confirmation clusters put remaining contrast for `{coordinate}` below tolerance when comparing {}. That does **not** prove those corners are two tilts of one mechanism, and it is not an arrow. Family membership is unproven.",
                scout.compared_columns.join(" vs ")
            ),
            ScoutOutcome::MultipleCoordinatesBelowTolerance { coordinates } => format!(
                "Multiple remaining contrasts sit below tolerance ({}). Do not force a direction.",
                coordinates.join(",")
            ),
            ScoutOutcome::NoneBelowTolerance => {
                "Compared corners do not become exchangeable after leaving out any one state column. Do not invent an arrow.".into()
            }
            ScoutOutcome::Underpowered => {
                "Confirmation half is underpowered for a remaining-contrast scout.".into()
            }
            ScoutOutcome::IntervalOverlap { unresolved } => format!(
                "Remaining-contrast intervals overlap the scout tolerance for {}. Undetermined, not absent.",
                unresolved.join(",")
            ),
        };
    }
    if interferometers.iter().any(|item| item.complete_square) {
        return "Freeze a complete square as a four_law manifest, assign a selection contract you actually know, and run mic-tabular four-law on confirmation clusters. Do not treat this atlas as orientation.".into();
    }
    if let Some(item) = interferometers
        .iter()
        .find(|item| !item.missing_corners.is_empty() || !item.dropped_corners.is_empty())
    {
        return format!(
            "No complete square was observed. Highest-ranked incomplete design `{}` has never-seen corners [{}] and under-supported dropped corners [{}]. Collect the never-seen arms; do not impute either class. The atlas is a design proposal, not a graph.",
            item.interferometer_id,
            item.missing_corners.join(","),
            item.dropped_corners.join(",")
        );
    }
    "No complete square was observed. Add the missing corners or collect a factorial follow-up. The atlas is a design proposal, not a graph.".into()
}

const SCOUT_EPSILON: f64 = 0.25;
const SCOUT_MIN_FULL: f64 = 1e-3;
const SCOUT_MIN_CLUSTERS_PER_ARM: usize = 4;
const SCOUT_REPLICATES: usize = 199;

fn scout_direction(
    table: &RawTable,
    columns: &[ColumnTriage],
    interferometer: &InterferometerProposal,
    cluster_column: Option<&str>,
    table_sha256: &str,
) -> Result<Option<DirectionScout>, EngineError> {
    let Some(cluster_name) = cluster_column else {
        return Ok(None);
    };
    let state_columns: Vec<String> = columns
        .iter()
        .filter(|column| column.role == ColumnRole::StateCandidate)
        .map(|column| column.column.clone())
        .collect();
    if state_columns.len() < 2 {
        return Ok(None);
    }
    let by_cluster =
        collect_compared_clusters(table, interferometer, cluster_name, &state_columns)?;
    let seed = seed_from_sha256(table_sha256);
    let (discovery, confirm_a, confirm_b) = split_confirmation(&by_cluster, seed);
    if confirm_a.len() < SCOUT_MIN_CLUSTERS_PER_ARM || confirm_b.len() < SCOUT_MIN_CLUSTERS_PER_ARM
    {
        return Ok(Some(scout_report(
            interferometer,
            state_columns,
            discovery,
            confirm_a.len() + confirm_b.len(),
            seed,
            0.0,
            ScoutOutcome::Underpowered,
            Vec::new(),
            "confirmation half has too few clusters per compared corner for bounds",
        )));
    }
    let full = mean_l2(&column_means(&confirm_a), &column_means(&confirm_b));
    if full < SCOUT_MIN_FULL {
        return Ok(Some(scout_report(
            interferometer,
            state_columns,
            discovery,
            confirm_a.len() + confirm_b.len(),
            seed,
            full,
            ScoutOutcome::Underpowered,
            Vec::new(),
            "full-support contrast on confirmation state is below the power floor",
        )));
    }
    let dim = state_columns.len();
    let mut contributions = Vec::with_capacity(confirm_a.len() + confirm_b.len());
    for point in &confirm_a {
        contributions.push(signed_remaining(point, 1.0, dim));
    }
    for point in &confirm_b {
        contributions.push(signed_remaining(point, -1.0, dim));
    }
    let bounds = match simultaneous_mean_bounds(&contributions, SCOUT_REPLICATES, 0.9, seed) {
        Ok(bounds) => bounds,
        Err(mic_stats::StatsError::DegenerateColumn { .. }) => {
            return Ok(Some(scout_report(
                interferometer,
                state_columns.clone(),
                discovery,
                confirm_a.len() + confirm_b.len(),
                seed,
                full,
                ScoutOutcome::IntervalOverlap {
                    unresolved: state_columns.clone(),
                },
                Vec::new(),
                "a contrast column was degenerate; refusing a zero-width interval",
            )));
        }
        Err(error) => return Err(error.into()),
    };
    let mut contrasts = Vec::with_capacity(dim);
    for (index, name) in state_columns.iter().enumerate() {
        let (point, lower, upper) = relative_from_signed(
            bounds.means[index],
            bounds.lower[index],
            bounds.upper[index],
            full,
        );
        contrasts.push(ScoutContrast {
            coordinate: name.clone(),
            relative_discrepancy: point,
            lower,
            upper,
            status: scout_status(lower, upper),
        });
    }
    let outcome = summarize_scout(&contrasts);
    Ok(Some(scout_report(
        interferometer,
        state_columns,
        discovery,
        confirm_a.len() + confirm_b.len(),
        seed,
        full,
        outcome,
        contrasts,
        "remaining-contrast scout only; compared corners are not assumed to be one family",
    )))
}

fn scout_status(lower: f64, upper: f64) -> ScoutInvariance {
    if upper < SCOUT_EPSILON {
        ScoutInvariance::Below
    } else if lower > SCOUT_EPSILON {
        ScoutInvariance::Above
    } else {
        ScoutInvariance::Overlaps
    }
}

fn summarize_scout(contrasts: &[ScoutContrast]) -> ScoutOutcome {
    let below: Vec<String> = contrasts
        .iter()
        .filter(|item| item.status == ScoutInvariance::Below)
        .map(|item| item.coordinate.clone())
        .collect();
    let overlap: Vec<String> = contrasts
        .iter()
        .filter(|item| item.status == ScoutInvariance::Overlaps)
        .map(|item| item.coordinate.clone())
        .collect();
    if below.len() > 1 {
        return ScoutOutcome::MultipleCoordinatesBelowTolerance { coordinates: below };
    }
    if !overlap.is_empty() {
        return ScoutOutcome::IntervalOverlap {
            unresolved: overlap,
        };
    }
    match below.into_iter().next() {
        Some(coordinate) => ScoutOutcome::SingleCoordinateBelowTolerance { coordinate },
        None => ScoutOutcome::NoneBelowTolerance,
    }
}

#[allow(clippy::too_many_arguments)]
fn scout_report(
    interferometer: &InterferometerProposal,
    state_columns: Vec<String>,
    discovery_clusters: usize,
    confirmation_clusters: usize,
    seed: u64,
    full_discrepancy: f64,
    outcome: ScoutOutcome,
    contrasts: Vec<ScoutContrast>,
    note: &str,
) -> DirectionScout {
    DirectionScout {
        authority: SurveyAuthority::ProposalOnly,
        interferometer_id: interferometer.interferometer_id.clone(),
        compared_columns: interferometer.context_columns.clone(),
        state_columns,
        discovery_clusters,
        confirmation_clusters,
        seed,
        epsilon: SCOUT_EPSILON,
        full_discrepancy,
        outcome,
        contrasts,
        note: note.into(),
    }
}

type ClusterArmRows = BTreeMap<String, (u8, Vec<Vec<f64>>)>;

fn collect_compared_clusters(
    table: &RawTable,
    interferometer: &InterferometerProposal,
    cluster_name: &str,
    state_columns: &[String],
) -> Result<ClusterArmRows, EngineError> {
    let cluster_index = header_index(table, cluster_name)?;
    let state_indexes: Vec<usize> = state_columns
        .iter()
        .map(|name| header_index(table, name))
        .collect::<Result<Vec<_>, _>>()?;
    let mut by_cluster = ClusterArmRows::new();
    for row in &table.rows {
        let Some(bits) = row_bits(table, interferometer, row)? else {
            continue;
        };
        if bits.len() != 2 {
            continue;
        }
        let arm = match (bits[0], bits[1]) {
            (true, false) => 1,
            (false, true) => 2,
            _ => continue,
        };
        let values: Option<Vec<f64>> = state_indexes
            .iter()
            .map(|&index| row[index].parse::<f64>().ok())
            .collect();
        let Some(values) = values else {
            continue;
        };
        if values.iter().any(|value| !value.is_finite()) {
            continue;
        }
        let cluster_id = row[cluster_index].clone();
        if cluster_id.is_empty() {
            continue;
        }
        let entry = by_cluster
            .entry(cluster_id)
            .or_insert_with(|| (arm, Vec::new()));
        if entry.0 != arm {
            return Err(EngineError::InvalidTabular(
                "direction scout: a cluster spans both compared corners".into(),
            ));
        }
        entry.1.push(values);
    }
    Ok(by_cluster)
}

fn split_confirmation(
    by_cluster: &ClusterArmRows,
    seed: u64,
) -> (usize, Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let mut discovery = 0usize;
    let mut confirm_a = Vec::new();
    let mut confirm_b = Vec::new();
    for (cluster_id, (arm, rows)) in by_cluster {
        if fold_bit(cluster_id, seed) == 0 {
            discovery += 1;
            continue;
        }
        let mean = column_means(rows);
        if *arm == 1 {
            confirm_a.push(mean);
        } else {
            confirm_b.push(mean);
        }
    }
    (discovery, confirm_a, confirm_b)
}

fn row_bits(
    table: &RawTable,
    interferometer: &InterferometerProposal,
    row: &[String],
) -> Result<Option<Vec<bool>>, EngineError> {
    if interferometer.context_columns.len() == 1 {
        let index = header_index(table, &interferometer.context_columns[0])?;
        return Ok(DesignPoint::parse(&row[index]).ok().map(|point| point.bits));
    }
    if interferometer.context_columns.len() == 2 {
        let i = header_index(table, &interferometer.context_columns[0])?;
        let j = header_index(table, &interferometer.context_columns[1])?;
        let first_levels = sorted_uniques(table, i);
        let second_levels = sorted_uniques(table, j);
        if first_levels.len() != 2 || second_levels.len() != 2 {
            return Ok(None);
        }
        return Ok(Some(vec![
            row[i] == first_levels[1],
            row[j] == second_levels[1],
        ]));
    }
    Ok(None)
}

fn column_means(rows: &[Vec<f64>]) -> Vec<f64> {
    if rows.is_empty() {
        return Vec::new();
    }
    let dim = rows[0].len();
    let mut sums = vec![0.0; dim];
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            sums[index] += *value;
        }
    }
    let n = rows.len() as f64;
    sums.iter().map(|sum| sum / n).collect()
}

fn mean_l2(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f64>()
        .sqrt()
}

fn signed_remaining(point: &[f64], sign: f64, dim: usize) -> Vec<f64> {
    (0..dim)
        .map(|deleted| {
            let leftover: Vec<f64> = point
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != deleted)
                .map(|(_, value)| *value)
                .collect();
            sign * leftover.iter().copied().sum::<f64>() / leftover.len().max(1) as f64
        })
        .collect()
}

fn relative_from_signed(mean: f64, lower: f64, upper: f64, full: f64) -> (f64, f64, f64) {
    let scale = if full > 0.0 { 2.0 / full } else { 0.0 };
    let point = mean.abs() * scale;
    let abs_lower = if lower > 0.0 {
        lower
    } else if upper < 0.0 {
        -upper
    } else {
        0.0
    };
    let abs_upper = lower.abs().max(upper.abs());
    (point, abs_lower * scale, abs_upper * scale)
}

fn seed_from_sha256(hex: &str) -> u64 {
    let mut bytes = [0_u8; 8];
    for (index, slot) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        *slot = hex
            .get(start..start + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .unwrap_or(0);
    }
    u64::from_be_bytes(bytes)
}

fn fold_bit(cluster_id: &str, seed: u64) -> u64 {
    let mut hash = seed ^ 0xcbf2_9ce4_8422_2325;
    for byte in cluster_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash & 1
}

fn extension_note(base: &str, missing: &[String], dropped: &[String]) -> String {
    match (missing.is_empty(), dropped.is_empty()) {
        (true, true) => base.to_string(),
        (false, true) => format!(
            "{base}; never-seen corners [{}] — collect those arms, do not impute them",
            missing.join(",")
        ),
        (true, false) => format!(
            "{base}; dropped below min_corner_count [{}] — observed but under-supported, not the same as never-seen",
            dropped.join(",")
        ),
        (false, false) => format!(
            "{base}; never-seen corners [{}]; dropped below min_corner_count [{}] — do not impute either class",
            missing.join(","),
            dropped.join(",")
        ),
    }
}

fn classify_factorial_corners(design: &ObservedDesign) -> (Vec<String>, Vec<String>) {
    let Some(dimension) = design
        .points
        .first()
        .or_else(|| design.dropped.first().map(|corner| &corner.point))
        .map(DesignPoint::dimension)
    else {
        return (Vec::new(), Vec::new());
    };
    if dimension == 0 || dimension > 4 {
        return (Vec::new(), Vec::new());
    }
    let retained: BTreeSet<String> = design.points.iter().map(DesignPoint::bit_string).collect();
    let dropped: BTreeSet<String> = design
        .dropped
        .iter()
        .map(|corner| corner.point.bit_string())
        .collect();
    let n_corners = 1_usize << dimension;
    let missing = (0..n_corners)
        .map(|index| {
            (0..dimension)
                .map(|bit| if (index >> bit) & 1 == 1 { '1' } else { '0' })
                .collect::<String>()
        })
        .filter(|corner| !retained.contains(corner) && !dropped.contains(corner))
        .collect();
    (missing, dropped.into_iter().collect())
}

fn cluster_collapsed_bits(
    table: &RawTable,
    cluster_column: Option<&str>,
    bits_for_row: impl Fn(&[String]) -> Option<Vec<bool>>,
) -> Result<Vec<Vec<bool>>, EngineError> {
    if let Some(cluster) = cluster_column {
        let cluster_index = header_index(table, cluster)?;
        let mut seen: BTreeMap<String, Vec<bool>> = BTreeMap::new();
        let mut conflicts = BTreeSet::new();
        for row in &table.rows {
            let Some(bits) = bits_for_row(row) else {
                continue;
            };
            let id = row[cluster_index].clone();
            if let Some(existing) = seen.get(&id) {
                if existing != &bits {
                    conflicts.insert(id);
                }
            } else {
                seen.insert(id, bits);
            }
        }
        if !conflicts.is_empty() {
            return Err(EngineError::InvalidTabular(format!(
                "cluster column {cluster} spans multiple design corners for {} clusters",
                conflicts.len()
            )));
        }
        Ok(seen.into_values().collect())
    } else {
        Ok(table
            .rows
            .iter()
            .filter_map(|row| bits_for_row(row))
            .collect())
    }
}

fn unique_values(table: &RawTable, index: usize) -> BTreeSet<String> {
    table
        .rows
        .iter()
        .map(|row| row[index].clone())
        .filter(|value| !value.is_empty())
        .collect()
}

fn sorted_uniques(table: &RawTable, index: usize) -> Vec<String> {
    unique_values(table, index).into_iter().collect()
}

fn looks_like_bitstring(values: &BTreeSet<String>) -> bool {
    if values.is_empty() {
        return false;
    }
    let width = values.iter().next().map_or(0, String::len);
    width >= 2
        && values
            .iter()
            .all(|value| value.len() == width && value.chars().all(|ch| ch == '0' || ch == '1'))
}

fn header_index(table: &RawTable, name: &str) -> Result<usize, EngineError> {
    table
        .headers
        .iter()
        .position(|header| header == name)
        .ok_or_else(|| EngineError::InvalidTabular(format!("column {name} is not in the table")))
}

fn corner_slot(point: &DesignPoint) -> Option<usize> {
    if point.dimension() != 2 {
        return None;
    }
    Some(usize::from(point.bits[0]) + 2 * usize::from(point.bits[1]))
}

fn column_is_numeric(values: &BTreeSet<String>) -> bool {
    !values.is_empty() && values.iter().all(|value| value.parse::<f64>().is_ok())
}

fn suggested_four_law_manifest(
    table: &RawTable,
    interferometer: &InterferometerProposal,
    cluster_column: Option<&str>,
) -> Option<mic_data::ExperimentManifest> {
    if !interferometer.complete_square || interferometer.design.points.len() != 4 {
        return None;
    }
    let state_columns: Vec<String> = table
        .headers
        .iter()
        .filter(|header| {
            Some(header.as_str()) != cluster_column
                && !interferometer
                    .context_columns
                    .iter()
                    .any(|col| col == *header)
                && *header != "included"
                && *header != "row_id"
        })
        .cloned()
        .collect();
    if state_columns.is_empty() {
        return None;
    }
    let mut regimes = Vec::new();
    for (point, proportion) in interferometer
        .design
        .points
        .iter()
        .zip(&interferometer.design.proportions)
    {
        regimes.push(mic_data::RegimeSpec {
            id: point.bit_string(),
            design: point.clone(),
            sampling_proportion: *proportion,
            perturbations: Vec::new(),
        });
    }
    Some(mic_data::ExperimentManifest {
        schema_version: "1.0.0".into(),
        experiment_id: format!(
            "survey-{}",
            interferometer.interferometer_id.replace(':', "-")
        ),
        strict: true,
        inference_track: mic_data::InferenceTrack::FourLaw,
        selection: mic_data::SelectionContract::Unknown,
        cluster_column: cluster_column.unwrap_or("row").to_string(),
        regime_column: interferometer
            .context_columns
            .first()
            .cloned()
            .unwrap_or_else(|| "regime".into()),
        state_columns,
        candidate_state_blocks: Vec::new(),
        regimes,
        data: mic_data::DataSource {
            format: "csv".into(),
            path: table.path.clone(),
        },
        seed: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn discrete_fixture_survey_finds_the_encoded_square() {
        let report = run_unsupervised_survey(
            "examples/data/four_law_discrete.csv",
            Some(&workspace_root()),
            Some("cluster_id"),
            SurveyPolicy::default(),
        )
        .unwrap();
        assert_eq!(report.authority, SurveyAuthority::ProposalOnly);
        assert!(
            report
                .interferometers
                .iter()
                .any(|item| item.complete_square && item.context_columns == ["regime"])
        );
        assert!(report.wall.contains("cannot issue a certificate"));
        assert_eq!(report.cluster_unit_basis, ClusterUnitBasis::Declared);
        let suggested = report
            .suggested_manifest
            .expect("complete square should emit a draft");
        assert_eq!(suggested.selection, mic_data::SelectionContract::Unknown);
        assert_eq!(suggested.inference_track, mic_data::InferenceTrack::FourLaw);
        assert!(
            report.direction_scout.is_none(),
            "a single state column cannot license deletion orientation"
        );
        assert!(report.interferometers.iter().any(|item| {
            item.complete_square
                && item.missing_corners.is_empty()
                && item.dropped_corners.is_empty()
        }));
    }

    #[test]
    fn unique_numeric_column_is_state_not_a_cluster() {
        let dir = std::env::temp_dir().join("mic-survey-numeric");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("numeric.csv");
        std::fs::write(
            &path,
            "cluster_id,regime,outcome\n\
             c0,00,0.11\n\
             c1,10,0.22\n\
             c2,01,0.33\n\
             c3,11,0.44\n",
        )
        .unwrap();
        let report = run_unsupervised_survey(&path, None, None, SurveyPolicy::default()).unwrap();
        let outcome = report
            .columns
            .iter()
            .find(|column| column.column == "outcome")
            .unwrap();
        assert_eq!(
            outcome.role,
            ColumnRole::StateCandidate,
            "numeric columns are state unless declared as context"
        );
        assert_ne!(report.inferred_cluster_column.as_deref(), Some("outcome"));
    }

    #[test]
    fn binary_numeric_column_is_not_an_interferometer_factor() {
        let dir = std::env::temp_dir().join("mic-survey-binary-numeric");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("binary.csv");
        std::fs::write(
            &path,
            "cluster_id,regime,y\n\
             c0,00,0\n\
             c1,00,1\n\
             c2,10,0\n\
             c3,10,1\n\
             c4,01,0\n\
             c5,01,1\n\
             c6,11,0\n\
             c7,11,1\n",
        )
        .unwrap();
        let report =
            run_unsupervised_survey(&path, None, Some("cluster_id"), SurveyPolicy::default())
                .unwrap();
        let y = report
            .columns
            .iter()
            .find(|column| column.column == "y")
            .unwrap();
        assert_eq!(y.role, ColumnRole::StateCandidate);
        assert!(
            report
                .interferometers
                .iter()
                .all(|item| !item.context_columns.iter().any(|column| column == "y"))
        );
    }

    #[test]
    fn three_corner_table_names_the_missing_arm_and_stays_proposal_only() {
        let dir = std::env::temp_dir().join("mic-survey-missing-corner");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("three_corners.csv");
        std::fs::write(
            &path,
            "cluster_id,regime,x\n\
             c00a,00,0\nc00b,00,1\n\
             c10a,10,0\nc10b,10,1\n\
             c01a,01,0\nc01b,01,1\n",
        )
        .unwrap();
        let report =
            run_unsupervised_survey(&path, None, Some("cluster_id"), SurveyPolicy::default())
                .unwrap();
        assert_eq!(report.authority, SurveyAuthority::ProposalOnly);
        assert!(report.suggested_manifest.is_none());
        let square = report
            .interferometers
            .iter()
            .find(|item| item.context_columns == ["regime"])
            .expect("bitstring regime should still be proposed as an incomplete design");
        assert!(!square.complete_square);
        assert_eq!(square.missing_corners, ["11"]);
        assert!(square.dropped_corners.is_empty());
        assert!(square.note.contains("never-seen corners [11]"));
        assert!(report.next_step.contains("11"));
        assert!(report.next_step.contains("do not impute"));
    }

    #[test]
    fn under_supported_corner_is_dropped_not_missing() {
        let dir = std::env::temp_dir().join("mic-survey-dropped-corner");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dropped.csv");
        std::fs::write(
            &path,
            "cluster_id,regime,x\n\
             c00a,00,0\nc00b,00,1\n\
             c10a,10,0\nc10b,10,1\n\
             c01a,01,0\nc01b,01,1\n\
             c11only,11,0\n",
        )
        .unwrap();
        let report =
            run_unsupervised_survey(&path, None, Some("cluster_id"), SurveyPolicy::default())
                .unwrap();
        let square = report
            .interferometers
            .iter()
            .find(|item| item.context_columns == ["regime"])
            .expect("bitstring regime should be proposed");
        assert!(!square.complete_square);
        assert!(square.missing_corners.is_empty());
        assert_eq!(square.dropped_corners, ["11"]);
        assert!(square.note.contains("dropped below min_corner_count [11]"));
        assert!(!square.note.contains("never-seen corners [11]"));
        assert!(report.suggested_manifest.is_none());
    }

    #[test]
    fn two_tilts_of_one_target_name_that_state_on_confirmation_only() {
        let dir = std::env::temp_dir().join("mic-survey-direction-scout");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("two_tilts.csv");
        let mut rows = String::from("cluster_id,regime,target,distractor\n");
        for index in 0..24 {
            let jitter = 0.02 * f64::from(index % 5);
            let _ = writeln!(rows, "a{index},10,{t},{d}", t = 2.0 + jitter, d = jitter);
            let _ = writeln!(rows, "b{index},01,{t},{d}", t = -2.0 + jitter, d = jitter);
            let _ = writeln!(rows, "c{index},00,{t},{d}", t = jitter, d = jitter);
            let _ = writeln!(rows, "d{index},11,{t},{d}", t = jitter, d = jitter);
        }
        std::fs::write(&path, rows).unwrap();
        let report =
            run_unsupervised_survey(&path, None, Some("cluster_id"), SurveyPolicy::default())
                .unwrap();
        assert_eq!(report.authority, SurveyAuthority::ProposalOnly);
        let scout = report
            .direction_scout
            .expect("complete square with two state columns should emit a scout");
        assert_eq!(scout.authority, SurveyAuthority::ProposalOnly);
        match scout.outcome {
            ScoutOutcome::SingleCoordinateBelowTolerance { ref coordinate } => {
                assert_eq!(coordinate, "target");
            }
            other => panic!("expected single coordinate below tolerance, got {other:?}"),
        }
        assert!(report.next_step.contains("target"));
        assert!(report.next_step.contains("not an arrow"));
    }
}
