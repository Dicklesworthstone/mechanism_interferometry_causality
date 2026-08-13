#![forbid(unsafe_code)]
//! Unsupervised Stage 0–1 survey: column triage and a testability atlas.
//!
//! Authority is permanently `proposal_only`. Selection cannot be established
//! from rows, so this path never certifies.

use crate::EngineError;
use mic_data::{RawTable, load_raw_csv};
use mic_design::{
    DesignPoint, FamilyClassificationInput, ModularCompletionClass, ObservedDesign,
    OrientationTestability, SamplingOddsAudit, audit_design, audit_sampling_odds,
    classify_observed_family, observed_design_from_rows, orientation_testability,
};
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
    /// Pointwise lack-of-fit dimension of the retained corners, when defined.
    pub lack_of_fit_dimension: Option<usize>,
    /// Additive identified-set dimension on the retained corners.
    pub identified_set_dimension: Option<usize>,
    /// Highest-ranked unobserved corner for shrinking that set, if any.
    pub recommended_next_corner: Option<String>,
    /// Integer cost of that recommended corner (default 1000).
    pub recommended_next_corner_cost: Option<u32>,
    /// Modular-completion class. Survey never supplies laws, so this is untestable.
    pub modular_completion: ModularCompletionClass,
    /// Orientation testability. Catalog squares with no tilt family are untestable.
    pub orientation: OrientationTestability,
    /// Ranking score: complete square first, then min corner count.
    pub priority: f64,
    /// Why this is or is not immediately auditable.
    pub note: String,
}

/// Rows-versus-units header. A million cells with three replicates is three units.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CausalInformationContent {
    /// Raw table rows.
    pub n_rows: usize,
    /// Distinct values of the cluster column, or `n_rows` when the unit is a row.
    pub n_independent_units: usize,
    /// How the independent unit was chosen.
    pub unit_basis: ClusterUnitBasis,
    /// Retained corners on the highest-priority interferometer.
    pub n_distinct_supported_regimes: usize,
    /// Number of complete two-factor squares in the atlas.
    pub n_complete_testable_squares: usize,
    /// Smallest retained corner count on the headline interferometer.
    pub confirmatory_units_per_corner_min: Option<usize>,
    /// Largest retained corner count on the headline interferometer.
    pub confirmatory_units_per_corner_max: Option<usize>,
    /// Ranked next cell on the highest-priority incomplete design, if any.
    pub recommended_next_corner: Option<String>,
    /// Identified-set dimension of that incomplete design.
    pub identified_set_dimension: Option<usize>,
    /// Integer cost of the recommended cell (default 1000).
    pub recommended_next_corner_cost: Option<u32>,
    /// Reminder that this header is not an arrow.
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
    /// Rows, independent units, regimes, and complete squares.
    pub information_content: CausalInformationContent,
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
    /// Human-readable next action.
    pub next_step: String,
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
    let next_step = survey_next_step(&interferometers);
    let information_content = information_content(
        &table,
        cluster_column.as_deref(),
        cluster_unit_basis,
        &interferometers,
    )?;
    Ok(SurveyReport {
        schema_version: "1.1.0".into(),
        authority: SurveyAuthority::ProposalOnly,
        wall: "State-independent within-regime selection cannot be established from observed rows. This survey cannot issue a certificate.".into(),
        table_sha256: table.content_sha256,
        path: table.path.display().to_string(),
        n_rows: table.rows.len(),
        information_content,
        columns,
        inferred_cluster_column: cluster_column,
        cluster_unit_basis,
        interferometers,
        suggested_manifest,
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
            let looks_like_id = identifier_shaped_name(&lower)
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

fn identifier_shaped_name(lower: &str) -> bool {
    lower == "id"
        || lower.starts_with("id_")
        || lower.ends_with("_id")
        || lower.contains("_id_")
        || lower.ends_with("_key")
        || lower.contains("uuid")
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
    let lack_of_fit_dimension = audit_design(&design.points, 1e-10)
        .ok()
        .map(|audit| audit.lack_of_fit_dimension);
    let family = classify_observed_family(
        FamilyClassificationInput {
            points: &design.points,
            same_target_tilt_count: 0,
            distinct_root_targets: true,
            baseline_combo_laws: None,
        },
        1e-10,
    )
    .ok();
    let modular_completion = family
        .as_ref()
        .map_or(ModularCompletionClass::Untestable, |item| {
            item.modular_completion
        });
    let orientation = family
        .as_ref()
        .map_or_else(|| orientation_testability(0), |item| item.orientation);
    debug_assert_ne!(modular_completion, ModularCompletionClass::Unique);
    debug_assert_eq!(orientation, OrientationTestability::Untestable);
    let identified_set_dimension = family.as_ref().map(|item| item.identified_set_dimension);
    let recommended_next_corner = family
        .as_ref()
        .and_then(|item| item.recommended_next_corner.clone());
    let recommended_next_corner_cost = family.as_ref().and_then(|item| item.next_corner_cost);
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
        lack_of_fit_dimension,
        identified_set_dimension,
        recommended_next_corner,
        recommended_next_corner_cost,
        modular_completion,
        orientation,
        priority,
        note,
    }))
}

fn information_content(
    table: &RawTable,
    cluster_column: Option<&str>,
    unit_basis: ClusterUnitBasis,
    interferometers: &[InterferometerProposal],
) -> Result<CausalInformationContent, EngineError> {
    let n_rows = table.rows.len();
    let n_independent_units = match cluster_column {
        Some(name) => unique_values(table, header_index(table, name)?).len(),
        None => n_rows,
    };
    let headline = interferometers
        .iter()
        .find(|item| item.complete_square)
        .or_else(|| interferometers.first());
    let n_distinct_supported_regimes = headline.map_or(0, |item| item.design.points.len());
    let n_complete_testable_squares = interferometers
        .iter()
        .filter(|item| item.complete_square)
        .count();
    let (confirmatory_units_per_corner_min, confirmatory_units_per_corner_max) =
        headline.map_or((None, None), |item| {
            (
                item.design.counts.iter().copied().min(),
                item.design.counts.iter().copied().max(),
            )
        });
    let incomplete = interferometers
        .iter()
        .find(|item| item.recommended_next_corner.is_some());
    Ok(CausalInformationContent {
        n_rows,
        n_independent_units,
        unit_basis,
        n_distinct_supported_regimes,
        n_complete_testable_squares,
        confirmatory_units_per_corner_min,
        confirmatory_units_per_corner_max,
        recommended_next_corner: incomplete
            .and_then(|item| item.recommended_next_corner.clone()),
        identified_set_dimension: incomplete.and_then(|item| item.identified_set_dimension),
        recommended_next_corner_cost: incomplete.and_then(|item| item.recommended_next_corner_cost),
        note: "Independent units are clusters when a unit column is declared or inferred, otherwise rows. Complete squares are design facts, not arrows.".into(),
    })
}

fn survey_next_step(interferometers: &[InterferometerProposal]) -> String {
    if interferometers.iter().any(|item| item.complete_square) {
        return "Freeze a complete square as a four_law manifest, assign a selection contract you actually know, and run mic-tabular four-law on confirmation clusters. Orientation is untestable: this atlas has no same-target tilt family. Do not treat this atlas as orientation.".into();
    }
    if let Some(item) = interferometers
        .iter()
        .find(|item| !item.missing_corners.is_empty() || !item.dropped_corners.is_empty())
    {
        let next = match (
            item.recommended_next_corner.as_deref(),
            item.identified_set_dimension,
        ) {
            (Some(corner), Some(idim)) => {
                format!(" Ranked next corner `{corner}` (identified-set dimension {idim}).")
            }
            (Some(corner), None) => format!(" Ranked next corner `{corner}`."),
            _ => String::new(),
        };
        return format!(
            "No complete square was observed. Highest-ranked incomplete design `{}` has never-seen corners [{}] and under-supported dropped corners [{}]. Collect the never-seen arms; do not impute either class.{} The atlas is a design proposal, not a graph.",
            item.interferometer_id,
            item.missing_corners.join(","),
            item.dropped_corners.join(","),
            next
        );
    }
    "No complete square was observed. Add the missing corners or collect a factorial follow-up. The atlas is a design proposal, not a graph.".into()
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
        assert_eq!(report.schema_version, "1.1.0");
        assert!(
            report
                .interferometers
                .iter()
                .any(|item| item.complete_square && item.context_columns == ["regime"])
        );
        assert!(report.wall.contains("cannot issue a certificate"));
        assert_eq!(report.cluster_unit_basis, ClusterUnitBasis::Declared);
        assert_eq!(report.information_content.n_rows, report.n_rows);
        assert!(report.information_content.n_independent_units > 0);
        assert!(report.information_content.n_independent_units <= report.n_rows);
        assert!(report.information_content.n_complete_testable_squares >= 1);
        assert_eq!(report.information_content.n_distinct_supported_regimes, 4);
        assert!(report.information_content.note.contains("not arrows"));
        let suggested = report
            .suggested_manifest
            .expect("complete square should emit a draft");
        assert_eq!(suggested.selection, mic_data::SelectionContract::Unknown);
        assert_eq!(suggested.inference_track, mic_data::InferenceTrack::FourLaw);
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
    fn state_name_containing_id_is_not_an_identifier() {
        let dir = std::env::temp_dir().join("mic-survey-humidity");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("humidity.csv");
        std::fs::write(
            &path,
            "cluster_id,regime,humidity\n\
             c0,00,40.0\n\
             c1,10,41.0\n\
             c2,01,42.0\n\
             c3,11,43.0\n",
        )
        .unwrap();
        let report =
            run_unsupervised_survey(&path, None, Some("cluster_id"), SurveyPolicy::default())
                .unwrap();
        let humidity = report
            .columns
            .iter()
            .find(|column| column.column == "humidity")
            .unwrap();
        assert_eq!(humidity.role, ColumnRole::StateCandidate);
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
        assert!(report.next_step.contains("Ranked next corner `11`"));
        assert_eq!(
            report
                .information_content
                .recommended_next_corner
                .as_deref(),
            Some("11")
        );
        assert_eq!(
            report.information_content.recommended_next_corner_cost,
            Some(1000)
        );
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
    fn diagonal_two_corner_table_has_vacuous_flatness_and_no_complete_square() {
        let dir = std::env::temp_dir().join("mic-survey-diagonal");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("diagonal.csv");
        std::fs::write(
            &path,
            "cluster_id,regime,x\n\
             c00a,00,0\nc00b,00,1\n\
             c11a,11,0\nc11b,11,1\n",
        )
        .unwrap();
        let report =
            run_unsupervised_survey(&path, None, Some("cluster_id"), SurveyPolicy::default())
                .unwrap();
        assert_eq!(report.authority, SurveyAuthority::ProposalOnly);
        assert_eq!(report.information_content.n_independent_units, 4);
        assert_eq!(report.information_content.n_complete_testable_squares, 0);
        let square = report
            .interferometers
            .iter()
            .find(|item| item.context_columns == ["regime"])
            .expect("bitstring regime should still be proposed");
        assert!(!square.complete_square);
        assert_eq!(square.missing_corners, ["10", "01"]);
        assert_eq!(square.lack_of_fit_dimension, Some(0));
        assert_eq!(square.identified_set_dimension, Some(1));
        assert!(matches!(
            square.recommended_next_corner.as_deref(),
            Some("01" | "10")
        ));
        assert_eq!(square.recommended_next_corner_cost, Some(1000));
        assert_eq!(report.information_content.n_distinct_supported_regimes, 2);
    }

    /// S2a season tautology as an atlas fixture, not a direction scout.
    /// elev × season is a complete square; the survey must not mint an arrow.
    #[test]
    fn season_elevation_world_is_a_complete_square_and_stays_proposal_only() {
        let table =
            load_raw_csv("examples/data/s2a_season_trap.csv", Some(&workspace_root())).unwrap();
        let moisture = values_by_context(&table, "moisture", "elev", "season");
        let reference = moisture.values().next().unwrap();
        assert!(moisture.values().all(|values| values == reference));
        let report = run_unsupervised_survey(
            "examples/data/s2a_season_trap.csv",
            Some(&workspace_root()),
            Some("cluster_id"),
            SurveyPolicy::default(),
        )
        .unwrap();
        assert_atlas_only_complete_pair(&report, "elev", "season");
        assert!(report.information_content.recommended_next_corner.is_none());
    }

    /// S2b: season moves temperature and moisture. Still atlas-only.
    #[test]
    fn coordinated_season_world_is_a_complete_square_and_stays_proposal_only() {
        let table =
            load_raw_csv("examples/data/s2b_coordinated.csv", Some(&workspace_root())).unwrap();
        let moisture = values_by_context(&table, "moisture", "elev", "season");
        assert_eq!(
            moisture.get(&("low".into(), "summer".into())),
            moisture.get(&("high".into(), "summer".into()))
        );
        assert_eq!(
            moisture.get(&("low".into(), "winter".into())),
            moisture.get(&("high".into(), "winter".into()))
        );
        assert_ne!(
            moisture.get(&("low".into(), "summer".into())),
            moisture.get(&("low".into(), "winter".into()))
        );
        let report = run_unsupervised_survey(
            "examples/data/s2b_coordinated.csv",
            Some(&workspace_root()),
            Some("cluster_id"),
            SurveyPolicy::default(),
        )
        .unwrap();
        assert_atlas_only_complete_pair(&report, "elev", "season");
    }

    /// S3: voltage-driven and current-driven copies of V = I. Atlas-only.
    #[test]
    fn mirrored_vir_world_is_a_complete_square_and_stays_proposal_only() {
        let table =
            load_raw_csv("examples/data/s3_vir_mirror.csv", Some(&workspace_root())).unwrap();
        let voltage = header_index(&table, "voltage").unwrap();
        let current = header_index(&table, "current").unwrap();
        assert!(
            table.rows.iter().all(|row| row[voltage] == row[current]),
            "mirrored constitutive fixture must preserve V = I row by row"
        );
        let report = run_unsupervised_survey(
            "examples/data/s3_vir_mirror.csv",
            Some(&workspace_root()),
            Some("cluster_id"),
            SurveyPolicy::default(),
        )
        .unwrap();
        assert_atlas_only_complete_pair(&report, "drive", "level");
    }

    fn assert_atlas_only_complete_pair(report: &SurveyReport, first: &str, second: &str) {
        assert_eq!(report.authority, SurveyAuthority::ProposalOnly);
        let square = report
            .interferometers
            .iter()
            .find(|item| item.complete_square)
            .expect("token pair must form a complete square");
        assert!(square.context_columns.contains(&first.to_string()));
        assert!(square.context_columns.contains(&second.to_string()));
        assert!(
            report
                .next_step
                .contains("Do not treat this atlas as orientation"),
            "atlas next_step must refuse orientation: {}",
            report.next_step
        );
        assert!(report.next_step.contains("Orientation is untestable"));
        assert_eq!(square.orientation, OrientationTestability::Untestable);
        assert_eq!(
            square.modular_completion,
            ModularCompletionClass::Untestable
        );
        let wire = serde_json::to_value(report).expect("survey report must serialize");
        let actual: BTreeSet<&str> = wire
            .as_object()
            .expect("survey report must serialize as an object")
            .keys()
            .map(String::as_str)
            .collect();
        let expected: BTreeSet<&str> = [
            "schema_version",
            "authority",
            "wall",
            "table_sha256",
            "path",
            "n_rows",
            "information_content",
            "columns",
            "inferred_cluster_column",
            "cluster_unit_basis",
            "interferometers",
            "suggested_manifest",
            "next_step",
        ]
        .into_iter()
        .collect();
        assert_eq!(
            actual, expected,
            "survey wire shape gained an unreviewed field"
        );
        if let Some(manifest) = &report.suggested_manifest {
            assert_eq!(manifest.selection, mic_data::SelectionContract::Unknown);
        }
    }

    fn values_by_context(
        table: &RawTable,
        value: &str,
        first: &str,
        second: &str,
    ) -> BTreeMap<(String, String), Vec<String>> {
        let value_index = header_index(table, value).unwrap();
        let first_index = header_index(table, first).unwrap();
        let second_index = header_index(table, second).unwrap();
        let mut groups = BTreeMap::new();
        for row in &table.rows {
            groups
                .entry((row[first_index].clone(), row[second_index].clone()))
                .or_insert_with(Vec::new)
                .push(row[value_index].clone());
        }
        groups
    }
}
