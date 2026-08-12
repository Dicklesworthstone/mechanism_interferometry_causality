#![forbid(unsafe_code)]
//! Cluster-weighted histogram four-law audit.
//!
//! This is a projection of the four regime laws onto a coarse state. It is a
//! diagnostic, never a certificate: locality and deletion orientation are not
//! established here.

use crate::{
    EngineError, OverlapAudit, PreflightPolicy, PreflightReport, audit_overlap,
    finding_with_context, run_preflight,
};
use mic_audit::{
    CertificateGates, CertificateStatus, EvidenceLedger, ExecutionMode, NarrativeReport, Severity,
    render_narrative,
};
use mic_core::DensitySquare;
use mic_data::{ExperimentManifest, IngestReport, load_csv_table};
use mic_design::{DesignPoint, SquareFace};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Policy for the histogram four-law projection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FourLawPolicy {
    /// Quantile bins per continuous column. Discrete columns keep their labels.
    pub bins_per_column: usize,
    /// A column with at most this many unique values is treated as discrete.
    pub discrete_unique_limit: usize,
    /// Minimum cluster-count in a cell for that cell to enter common support.
    pub min_cell_clusters: usize,
    /// Declared-versus-empirical quota gap that raises a warning.
    pub quota_gap_warning: f64,
    /// Number of cluster-level folds recorded for later confirmation splits.
    pub n_folds: usize,
}

impl Default for FourLawPolicy {
    fn default() -> Self {
        Self {
            bins_per_column: 4,
            discrete_unique_limit: 16,
            min_cell_clusters: 1,
            quota_gap_warning: 0.05,
            n_folds: 5,
        }
    }
}

/// Empirical masses and curvature at one histogram cell.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CellCurvature {
    /// Bin-tuple key, `|`-separated.
    pub cell: String,
    /// Baseline mass.
    pub p0: f64,
    /// Primitive-A mass.
    pub pa: f64,
    /// Primitive-B mass.
    pub pb: f64,
    /// Joint-AB mass.
    pub pab: f64,
    /// `p_a / p_0`.
    pub ra: f64,
    /// `p_b / p_0`.
    pub rb: f64,
    /// `p_ab / p_0`.
    pub rab: f64,
    /// Gauge-invariant curvature at the cell.
    pub kappa: f64,
}

/// Histogram four-law result for one observed square face.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FourLawFaceAudit {
    /// Face corners in order `00, 10, 01, 11` as bit strings.
    pub corners: [String; 4],
    /// Manifest regime identifiers in the same order.
    pub regime_ids: [String; 4],
    /// Cells on common positive support.
    pub cells: Vec<CellCurvature>,
    /// Cells occupied by at least one corner but missing another.
    pub incomplete_cells: usize,
    /// Baseline cluster mass on cells that were dropped for incomplete support.
    pub omitted_baseline_mass: f64,
    /// `E_0[r_A]` on common support. Must be reported raw, never silently renormalized away.
    pub normalizer_a: f64,
    /// `E_0[r_B]` on common support.
    pub normalizer_b: f64,
    /// `E_0[r_AB]` on common support.
    pub normalizer_ab: f64,
    /// `E_0[r_AB - r_A r_B]` with witness 1. Can be exactly blind.
    pub scalar_moment: f64,
    /// Same moment with witness `2 * bin_index / (n-1) - 1`.
    pub signed_moment: f64,
    /// Maximum `|kappa|` on common support.
    pub max_abs_kappa: f64,
    /// Mean `|kappa|` under empirical `P_0`.
    pub mean_abs_kappa: f64,
}

/// Complete tabular audit: preflight, ingest, four-law projection, overlap.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TabularAuditReport {
    /// Schema version.
    schema_version: String,
    /// Experiment identifier.
    experiment_id: String,
    /// Conservative certificate status. Histogram four-law never issues `passed`.
    status: CertificateStatus,
    /// Complete typed inputs from which `status` was derived.
    gates: CertificateGates,
    /// Preflight design and sampling gate.
    preflight: PreflightReport,
    /// Table fingerprints and realized quotas.
    ingest: TabularIngestSummary,
    /// Four-law faces. Empty when ingest or preflight blocked the projection.
    four_law: Vec<FourLawFaceAudit>,
    /// Overlap audit on baseline ratio weights, if they could be formed.
    overlap: Option<OverlapAudit>,
    /// Binning description recorded for reproducibility.
    projection: ProjectionSpec,
    /// Evidence ledger for the whole tabular run.
    ledger: EvidenceLedger,
}

impl TabularAuditReport {
    /// Returns the report schema version.
    #[must_use]
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    /// Returns the experiment identifier.
    #[must_use]
    pub fn experiment_id(&self) -> &str {
        &self.experiment_id
    }

    /// Returns the internally derived certificate status.
    #[must_use]
    pub const fn status(&self) -> CertificateStatus {
        self.status
    }

    /// Returns the complete typed gate summary used to derive `status`.
    #[must_use]
    pub const fn gates(&self) -> CertificateGates {
        self.gates
    }

    /// Returns the evidence ledger bound to the derived status.
    #[must_use]
    pub const fn ledger(&self) -> &EvidenceLedger {
        &self.ledger
    }

    /// Returns the immutable preflight report bound to this audit.
    #[must_use]
    pub const fn preflight(&self) -> &PreflightReport {
        &self.preflight
    }

    /// Returns the table fingerprint and realized-quota summary.
    #[must_use]
    pub const fn ingest(&self) -> &TabularIngestSummary {
        &self.ingest
    }

    /// Returns the projected four-law faces.
    #[must_use]
    pub fn four_law(&self) -> &[FourLawFaceAudit] {
        &self.four_law
    }

    /// Returns the overlap summary when ratio weights were available.
    #[must_use]
    pub const fn overlap(&self) -> Option<&OverlapAudit> {
        self.overlap.as_ref()
    }

    /// Returns the recorded projection definition.
    #[must_use]
    pub const fn projection(&self) -> &ProjectionSpec {
        &self.projection
    }

    /// Markdown report that leads with status and abstentions.
    #[must_use]
    pub fn narrative(&self) -> NarrativeReport {
        let mut extra = Vec::new();
        extra.push((
            "Ingest",
            format!(
                "rows={}, included_clusters={}, table_sha256=`{}`, cluster_fingerprint=`{}`.",
                self.ingest.fingerprint.n_rows,
                self.ingest.fingerprint.n_included_clusters,
                self.ingest.fingerprint.content_sha256,
                self.ingest.fingerprint.cluster_fingerprint
            ),
        ));
        if let Some(face) = self.four_law.first() {
            extra.push((
                "Histogram four-law projection",
                format!(
                    "common-support cells={}, max_|κ|={:.6}, E_0[r_A]={:.6} (raw residual {:.6}), scalar moment={:.6}, signed moment={:.6}. The scalar moment can be exactly blind to curvature.",
                    face.cells.len(),
                    face.max_abs_kappa,
                    face.normalizer_a,
                    face.normalizer_a - 1.0,
                    face.scalar_moment,
                    face.signed_moment
                ),
            ));
        } else {
            extra.push((
                "Histogram four-law projection",
                "No face was projected.".into(),
            ));
        }
        extra.push((
            "What this is not",
            "This run does not localize a family, does not orient a target, and does not certify modularity. Use it to inspect design eligibility, unit discipline, overlap, and a coarse κ field.".into(),
        ));
        render_narrative(
            &self.experiment_id,
            &self.gates,
            &self.ledger,
            &extra
                .iter()
                .map(|(title, body)| (*title, body.clone()))
                .collect::<Vec<_>>(),
        )
    }
}

/// Serializable ingest slice without the raw rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TabularIngestSummary {
    /// Content and unit fingerprints.
    pub fingerprint: mic_data::TableFingerprint,
    /// Per-regime realized quotas.
    pub regime_counts: Vec<mic_data::RegimeCount>,
    /// Clusters that appeared in more than one regime.
    pub clusters_spanning_regimes: Vec<String>,
    /// Declared regimes with no included clusters.
    pub missing_regimes: Vec<String>,
    /// Maximum declared-versus-row-quota gap.
    pub max_row_quota_gap: f64,
    /// Maximum declared-versus-cluster-quota gap.
    pub max_cluster_quota_gap: f64,
    /// Cluster-level fold plan size.
    pub n_folds: usize,
}

/// Recorded histogram projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectionSpec {
    /// Per-column bin edges or discrete labels.
    pub columns: Vec<ColumnProjection>,
    /// Policy used to construct the projection.
    pub policy: FourLawPolicy,
}

/// One state column's recorded projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColumnProjection {
    /// Column name.
    pub column: String,
    /// `discrete` or `quantile`.
    pub kind: String,
    /// Sorted unique values, or quantile edges.
    pub knots: Vec<f64>,
}

/// Runs preflight, loads the CSV, and evaluates the histogram four-law projection.
pub fn run_tabular_audit(
    manifest: &ExperimentManifest,
    four_law: FourLawPolicy,
    preflight: PreflightPolicy,
    base_dir: Option<&Path>,
) -> Result<TabularAuditReport, EngineError> {
    if four_law.bins_per_column < 2 {
        return Err(EngineError::InvalidTabular(
            "bins_per_column must be at least 2".into(),
        ));
    }
    if four_law.n_folds == 0 {
        return Err(EngineError::InvalidTabular(
            "n_folds must be positive".into(),
        ));
    }
    let preflight_report = run_preflight(manifest, preflight)?;
    let mut ledger = preflight_report.ledger.clone();
    ledger.provenance("tabular_reader", "std_csv");
    ledger.provenance("seed", manifest.seed.to_string());

    let ingest = load_csv_table(manifest, base_dir, four_law.n_folds)?;
    record_ingest(manifest, &ingest, four_law.quota_gap_warning, &mut ledger);

    let ingest_summary = TabularIngestSummary {
        fingerprint: ingest.fingerprint.clone(),
        regime_counts: ingest.regime_counts.clone(),
        clusters_spanning_regimes: ingest.clusters_spanning_regimes.clone(),
        missing_regimes: ingest.missing_regimes.clone(),
        max_row_quota_gap: ingest.max_row_quota_gap,
        max_cluster_quota_gap: ingest.max_cluster_quota_gap,
        n_folds: four_law.n_folds,
    };

    let blocked_before_projection = !preflight_report.four_law_eligible
        || !ingest.clusters_spanning_regimes.is_empty()
        || !ingest.missing_regimes.is_empty();

    if blocked_before_projection {
        return Ok(finish(
            manifest,
            preflight_report,
            ingest_summary,
            Vec::new(),
            None,
            ProjectionSpec {
                columns: Vec::new(),
                policy: four_law,
            },
            ledger,
        ));
    }

    let (projection, labeled) = project_state(manifest, &ingest, four_law);
    let mut faces = Vec::new();
    let mut overlap = None;
    for face in &preflight_report.design.square_faces {
        let corners = face.corners();
        let audit = match audit_face(manifest, &ingest, &labeled, &corners, four_law) {
            Ok(audit) => audit,
            Err(EngineError::InvalidTabular(message))
                if message.contains("empty common support") =>
            {
                ledger.note(Severity::Error, "four_law", "empty_common_support", message);
                continue;
            }
            Err(error) => return Err(error),
        };
        audit_face_overlap(
            &labeled,
            &audit,
            face,
            &corners,
            &preflight,
            &mut overlap,
            &mut ledger,
        )?;
        record_face(&audit, &mut ledger);
        faces.push(audit);
    }
    if faces.is_empty() {
        ledger.note(
            Severity::Error,
            "four_law",
            "no_observed_square",
            "the observed design has no complete square face, so histogram four-law curvature is not defined",
        );
    }
    Ok(finish(
        manifest,
        preflight_report,
        ingest_summary,
        faces,
        overlap,
        projection,
        ledger,
    ))
}

fn finish(
    manifest: &ExperimentManifest,
    preflight: PreflightReport,
    ingest: TabularIngestSummary,
    four_law: Vec<FourLawFaceAudit>,
    overlap: Option<OverlapAudit>,
    projection: ProjectionSpec,
    mut ledger: EvidenceLedger,
) -> TabularAuditReport {
    ledger.provenance("projection", "histogram_cluster_weighted");
    ledger.note(
        Severity::Info,
        "certificate",
        "histogram_not_a_certificate",
        "histogram four-law is a projection diagnostic; locality and deletion orientation were not established, so the run abstains from a modularity certificate",
    );
    // The histogram projection is diagnostic: it does not establish locality,
    // conditional normalization, square-flatness inference, or orientation.
    let gates = CertificateGates::unresolved();
    let status = ledger.status(&gates);
    TabularAuditReport {
        schema_version: "2.0.0".into(),
        experiment_id: manifest.experiment_id.clone(),
        status,
        gates,
        preflight,
        ingest,
        four_law,
        overlap,
        projection,
        ledger,
    }
}

fn record_ingest(
    manifest: &ExperimentManifest,
    ingest: &IngestReport,
    quota_gap_warning: f64,
    ledger: &mut EvidenceLedger,
) {
    ledger.provenance("table_sha256", ingest.fingerprint.content_sha256.clone());
    ledger.provenance(
        "cluster_fingerprint",
        ingest.fingerprint.cluster_fingerprint.clone(),
    );
    ledger.provenance(
        "n_included_clusters",
        ingest.fingerprint.n_included_clusters.to_string(),
    );
    if !ingest.clusters_spanning_regimes.is_empty() {
        let mut context = BTreeMap::new();
        context.insert(
            "clusters".into(),
            ingest.clusters_spanning_regimes.join(","),
        );
        ledger.push(finding_with_context(
            Severity::Error,
            "ingest",
            "cluster_spans_regimes",
            "at least one cluster appears under more than one regime; the assignment unit is not a regime unit",
            context,
        ));
    }
    if !ingest.missing_regimes.is_empty() {
        let mut context = BTreeMap::new();
        context.insert("regimes".into(), ingest.missing_regimes.join(","));
        ledger.push(finding_with_context(
            Severity::Error,
            "ingest",
            "missing_regime_data",
            "a declared regime has no included clusters",
            context,
        ));
    }
    if ingest.max_cluster_quota_gap > quota_gap_warning {
        let mut context = BTreeMap::new();
        context.insert(
            "max_cluster_quota_gap".into(),
            format!("{:.6}", ingest.max_cluster_quota_gap),
        );
        ledger.push(finding_with_context(
            Severity::Warning,
            "ingest",
            "declared_empirical_quota_mismatch",
            "realized cluster quotas differ from the declared sampling proportions; four-law uses the declared quotas only for the sampling-odds gate",
            context,
        ));
    }
    let _ = manifest;
}

/// One observation paired with its per-column bin indices under the recorded projection.
type BinnedObservation = (ObservationLabel, Vec<usize>);

fn project_state(
    manifest: &ExperimentManifest,
    ingest: &IngestReport,
    policy: FourLawPolicy,
) -> (ProjectionSpec, Vec<BinnedObservation>) {
    let dim = manifest.state_columns.len();
    let mut values: Vec<Vec<f64>> = vec![Vec::new(); dim];
    for row in &ingest.rows {
        if !row.included {
            continue;
        }
        for (index, value) in row.state.iter().enumerate() {
            values[index].push(*value);
        }
    }
    let mut columns = Vec::new();
    let mut assigners: Vec<Box<dyn Fn(f64) -> usize>> = Vec::new();
    for (index, column) in manifest.state_columns.iter().enumerate() {
        let mut unique = values[index].clone();
        unique.sort_by(|left, right| cmp_f64(*left, *right));
        unique.dedup_by(|left, right| (*left - *right).abs() <= 0.0);
        if unique.len() <= policy.discrete_unique_limit {
            let knots = unique.clone();
            columns.push(ColumnProjection {
                column: column.clone(),
                kind: "discrete".into(),
                knots: knots.clone(),
            });
            assigners.push(Box::new(move |value| {
                knots
                    .iter()
                    .position(|knot| (*knot - value).abs() <= 0.0)
                    .unwrap_or(0)
            }));
        } else {
            let edges = quantile_edges(&values[index], policy.bins_per_column);
            let captured = edges.clone();
            columns.push(ColumnProjection {
                column: column.clone(),
                kind: "quantile".into(),
                knots: edges,
            });
            assigners.push(Box::new(move |value| bin_index(value, &captured)));
        }
    }
    let mut labeled = Vec::new();
    for row in &ingest.rows {
        if !row.included {
            continue;
        }
        let bins = row
            .state
            .iter()
            .enumerate()
            .map(|(index, value)| assigners[index](*value))
            .collect();
        labeled.push((
            ObservationLabel {
                cluster_id: row.cluster_id.clone(),
                regime_id: row.regime_id.clone(),
            },
            bins,
        ));
    }
    (ProjectionSpec { columns, policy }, labeled)
}

#[derive(Clone)]
struct ObservationLabel {
    cluster_id: String,
    regime_id: String,
}

#[allow(clippy::too_many_lines)]
fn audit_face(
    manifest: &ExperimentManifest,
    ingest: &IngestReport,
    labeled: &[(ObservationLabel, Vec<usize>)],
    corners: &[DesignPoint; 4],
    policy: FourLawPolicy,
) -> Result<FourLawFaceAudit, EngineError> {
    let regime_ids = corner_regime_ids(manifest, corners)?;
    let mut cluster_cells: BTreeMap<(String, String), BTreeMap<Vec<usize>, usize>> =
        BTreeMap::new();
    for (label, bins) in labeled {
        if !regime_ids.contains(&label.regime_id) {
            continue;
        }
        *cluster_cells
            .entry((label.cluster_id.clone(), label.regime_id.clone()))
            .or_default()
            .entry(bins.clone())
            .or_insert(0) += 1;
    }
    let mut regime_cell_mass: [BTreeMap<Vec<usize>, f64>; 4] = [
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    ];
    let mut regime_cluster_count = [0_usize; 4];
    let mut cell_cluster_count: [BTreeMap<Vec<usize>, usize>; 4] = [
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    ];
    for ((_, regime_id), counts) in &cluster_cells {
        let Some(slot) = regime_ids.iter().position(|id| id == regime_id) else {
            continue;
        };
        let total = counts.values().sum::<usize>() as f64;
        if total <= 0.0 {
            continue;
        }
        regime_cluster_count[slot] += 1;
        for (cell, count) in counts {
            *regime_cell_mass[slot].entry(cell.clone()).or_insert(0.0) += *count as f64 / total;
            *cell_cluster_count[slot].entry(cell.clone()).or_insert(0) += 1;
        }
    }
    for (slot, masses) in regime_cell_mass.iter_mut().enumerate() {
        let n = regime_cluster_count[slot].max(1) as f64;
        for mass in masses.values_mut() {
            *mass /= n;
        }
    }
    let mut all_cells = BTreeSet::new();
    for masses in &regime_cell_mass {
        all_cells.extend(masses.keys().cloned());
    }
    let mut cells = Vec::new();
    let mut incomplete_cells = 0usize;
    for cell in all_cells {
        let counts = [
            *cell_cluster_count[0].get(&cell).unwrap_or(&0),
            *cell_cluster_count[1].get(&cell).unwrap_or(&0),
            *cell_cluster_count[2].get(&cell).unwrap_or(&0),
            *cell_cluster_count[3].get(&cell).unwrap_or(&0),
        ];
        if counts.iter().any(|count| *count < policy.min_cell_clusters) {
            incomplete_cells += 1;
            continue;
        }
        let p0 = *regime_cell_mass[0].get(&cell).unwrap_or(&0.0);
        let pa = *regime_cell_mass[1].get(&cell).unwrap_or(&0.0);
        let pb = *regime_cell_mass[2].get(&cell).unwrap_or(&0.0);
        let pab = *regime_cell_mass[3].get(&cell).unwrap_or(&0.0);
        let square = DensitySquare { p0, pa, pb, pab };
        let kappa = square.curvature().map_err(|error| {
            EngineError::InvalidTabular(format!("cell {} curvature: {error}", format_cell(&cell)))
        })?;
        cells.push(CellCurvature {
            cell: format_cell(&cell),
            p0,
            pa,
            pb,
            pab,
            ra: pa / p0,
            rb: pb / p0,
            rab: pab / p0,
            kappa,
        });
    }
    cells.sort_by(|left, right| left.cell.cmp(&right.cell));
    if cells.is_empty() {
        return Err(EngineError::InvalidTabular(format!(
            "empty common support on face {}/{}: {incomplete_cells} incomplete cells and no four-corner cell",
            corners[0].bit_string(),
            corners[3].bit_string()
        )));
    }
    let p0: Vec<f64> = cells.iter().map(|cell| cell.p0).collect();
    let ra: Vec<f64> = cells.iter().map(|cell| cell.ra).collect();
    let rb: Vec<f64> = cells.iter().map(|cell| cell.rb).collect();
    let rab: Vec<f64> = cells.iter().map(|cell| cell.rab).collect();
    let ones = vec![1.0; cells.len()];
    let n_cells = cells.len();
    let signed: Vec<f64> = (0..n_cells)
        .map(|index| {
            if n_cells == 1 {
                0.0
            } else {
                2.0 * index as f64 / (n_cells - 1) as f64 - 1.0
            }
        })
        .collect();
    let normalizer_a = weighted_mean(&ra, &p0);
    let normalizer_b = weighted_mean(&rb, &p0);
    let normalizer_ab = weighted_mean(&rab, &p0);
    let scalar_moment = weighted_moment(&ones, &ra, &rb, &rab, &p0);
    let signed_moment = weighted_moment(&signed, &ra, &rb, &rab, &p0);
    let max_abs_kappa = cells
        .iter()
        .map(|cell| cell.kappa.abs())
        .fold(0.0, f64::max);
    let mean_abs_kappa = weighted_mean(
        &cells
            .iter()
            .map(|cell| cell.kappa.abs())
            .collect::<Vec<_>>(),
        &p0,
    );
    let omitted_baseline_mass = (1.0 - p0.iter().sum::<f64>()).max(0.0);
    let _ = ingest;
    Ok(FourLawFaceAudit {
        corners: corners.each_ref().map(DesignPoint::bit_string),
        regime_ids,
        cells,
        incomplete_cells,
        omitted_baseline_mass,
        normalizer_a,
        normalizer_b,
        normalizer_ab,
        scalar_moment,
        signed_moment,
        max_abs_kappa,
        mean_abs_kappa,
    })
}

fn weighted_moment(witness: &[f64], ra: &[f64], rb: &[f64], rab: &[f64], p0: &[f64]) -> f64 {
    let terms: Vec<f64> = witness
        .iter()
        .zip(ra)
        .zip(rb)
        .zip(rab)
        .map(|(((&w, &a), &b), &ab)| w * (ab - a * b))
        .collect();
    weighted_mean(&terms, p0)
}

fn weighted_mean(values: &[f64], weights: &[f64]) -> f64 {
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (&value, &weight) in values.iter().zip(weights) {
        numerator += value * weight;
        denominator += weight;
    }
    if denominator > 0.0 {
        numerator / denominator
    } else {
        f64::NAN
    }
}

fn overlap_stage(name: &str, face: &SquareFace) -> String {
    format!(
        "overlap_{name}@{}:{}:{}",
        face.base.bit_string(),
        face.first,
        face.second
    )
}

fn audit_face_overlap(
    labeled: &[(ObservationLabel, Vec<usize>)],
    audit: &FourLawFaceAudit,
    face: &SquareFace,
    corners: &[DesignPoint; 4],
    preflight: &PreflightPolicy,
    stored: &mut Option<OverlapAudit>,
    ledger: &mut EvidenceLedger,
) -> Result<(), EngineError> {
    for (name, pick) in [
        (
            "r_A",
            (|cell: &CellCurvature| cell.ra) as fn(&CellCurvature) -> f64,
        ),
        ("r_B", |cell| cell.rb),
        ("r_AB", |cell| cell.rab),
    ] {
        if let Some(weights) = primitive_ratio_weights(labeled, audit, &corners[0], pick) {
            let face_overlap =
                audit_overlap(&weights, preflight, &overlap_stage(name, face), ledger)?;
            if stored.is_none() {
                *stored = Some(face_overlap);
            }
        }
    }
    Ok(())
}

fn primitive_ratio_weights(
    labeled: &[(ObservationLabel, Vec<usize>)],
    face: &FourLawFaceAudit,
    baseline: &DesignPoint,
    pick: fn(&CellCurvature) -> f64,
) -> Option<Vec<f64>> {
    let baseline_id = face.regime_ids[0].clone();
    if baseline.bit_string() != face.corners[0] {
        return None;
    }
    let kappa: BTreeMap<&str, &CellCurvature> = face
        .cells
        .iter()
        .map(|cell| (cell.cell.as_str(), cell))
        .collect();
    let mut cluster_weights: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for (label, bins) in labeled {
        if label.regime_id != baseline_id {
            continue;
        }
        let key = format_cell(bins);
        if let Some(cell) = kappa.get(key.as_str()) {
            let entry = cluster_weights
                .entry(label.cluster_id.clone())
                .or_insert((0.0, 0.0));
            entry.0 += pick(cell);
            entry.1 += 1.0;
        }
    }
    if cluster_weights.is_empty() {
        return None;
    }
    Some(
        cluster_weights
            .values()
            .map(|(sum, count)| if *count > 0.0 { sum / count } else { 0.0 })
            .collect(),
    )
}

fn record_face(face: &FourLawFaceAudit, ledger: &mut EvidenceLedger) {
    if face.cells.is_empty() {
        ledger.note(
            Severity::Error,
            "four_law",
            "empty_common_support",
            "no histogram cell has positive mass in all four corners",
        );
        return;
    }
    let mut context = BTreeMap::new();
    context.insert("cells".into(), face.cells.len().to_string());
    context.insert("max_abs_kappa".into(), format!("{:.6}", face.max_abs_kappa));
    context.insert("normalizer_a".into(), format!("{:.6}", face.normalizer_a));
    context.insert(
        "normalizer_residual_a".into(),
        format!("{:.6}", face.normalizer_a - 1.0),
    );
    context.insert("scalar_moment".into(), format!("{:.6}", face.scalar_moment));
    context.insert("incomplete_cells".into(), face.incomplete_cells.to_string());
    context.insert(
        "omitted_baseline_mass".into(),
        format!("{:.6}", face.omitted_baseline_mass),
    );
    if face.incomplete_cells > 0 || face.omitted_baseline_mass > 1e-12 {
        let severity = if ledger.mode() == ExecutionMode::Strict {
            Severity::Error
        } else {
            Severity::Warning
        };
        ledger.push(finding_with_context(
            severity,
            "four_law",
            "incomplete_common_support",
            "some histogram cells lack all four corners; moments are renormalized onto the surviving common-support mass and do not represent omitted cells",
            context.clone(),
        ));
    }
    ledger.push(finding_with_context(
        Severity::Info,
        "four_law",
        "histogram_projection",
        "cluster-weighted histogram four-law projection computed; this is a diagnostic, not a certificate",
        context,
    ));
    ledger.provenance(
        "ratio_a_raw_normalizer",
        format!("{:.6}", face.normalizer_a),
    );
    ledger.provenance(
        "ratio_a_normalizer_residual",
        format!("{:.6}", face.normalizer_a - 1.0),
    );
}

fn corner_regime_ids(
    manifest: &ExperimentManifest,
    corners: &[DesignPoint; 4],
) -> Result<[String; 4], EngineError> {
    let mut ids = [String::new(), String::new(), String::new(), String::new()];
    for (index, corner) in corners.iter().enumerate() {
        let regime = manifest
            .regimes
            .iter()
            .find(|regime| regime.design == *corner)
            .ok_or_else(|| EngineError::MissingCorner(corner.bit_string()))?;
        ids[index].clone_from(&regime.id);
    }
    Ok(ids)
}

fn quantile_edges(values: &[f64], bins: usize) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| cmp_f64(*left, *right));
    let n = sorted.len();
    let mut edges = Vec::with_capacity(bins + 1);
    for bin in 0..=bins {
        let rank = (bin * (n.saturating_sub(1))) / bins;
        edges.push(sorted[rank]);
    }
    edges
}

fn bin_index(value: f64, edges: &[f64]) -> usize {
    if edges.len() < 2 {
        return 0;
    }
    for (index, window) in edges.windows(2).enumerate() {
        if value <= window[1] {
            return index;
        }
    }
    edges.len() - 2
}

fn format_cell(bins: &[usize]) -> String {
    bins.iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join("|")
}

fn cmp_f64(left: f64, right: f64) -> std::cmp::Ordering {
    left.partial_cmp(&right)
        .unwrap_or(std::cmp::Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PreflightStatus;
    use mic_audit::code;
    use mic_data::{DataSource, InferenceTrack, RegimeSpec, SelectionContract};
    use std::fmt::Write as _;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn load(path: &str) -> ExperimentManifest {
        ExperimentManifest::from_json_path(workspace_root().join(path)).unwrap()
    }

    #[test]
    fn curved_fixture_has_nonzero_kappa_and_abstains() {
        let report = run_tabular_audit(
            &load("examples/configs/four_law_discrete.json"),
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            Some(&workspace_root()),
        )
        .unwrap();
        assert_eq!(report.preflight.status, PreflightStatus::Ready);
        assert_eq!(report.status(), CertificateStatus::Abstained);
        let face = &report.four_law[0];
        assert_eq!(face.cells.len(), 2);
        let kappa0 = face
            .cells
            .iter()
            .find(|cell| cell.cell == "0")
            .unwrap()
            .kappa;
        let kappa1 = face
            .cells
            .iter()
            .find(|cell| cell.cell == "1")
            .unwrap()
            .kappa;
        assert!((kappa0 - 1.6_f64.ln()).abs() < 1e-12);
        assert!((kappa1 - 0.4_f64.ln()).abs() < 1e-12);
        assert!(face.max_abs_kappa > 0.8);
        let narrative = report.narrative();
        assert!(
            narrative
                .markdown()
                .contains("## Certificate status: `abstained`")
        );
        assert!(
            narrative.markdown().find("## Abstentions").unwrap()
                < narrative
                    .markdown()
                    .find("## Informational findings")
                    .unwrap()
        );
    }

    #[test]
    fn flat_fixture_has_zero_kappa() {
        let report = run_tabular_audit(
            &load("examples/configs/four_law_flat.json"),
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            Some(&workspace_root()),
        )
        .unwrap();
        let face = &report.four_law[0];
        assert!(face.max_abs_kappa.abs() < 1e-12);
        assert!(face.scalar_moment.abs() < 1e-12);
        assert!((face.normalizer_a - 1.0).abs() < 1e-12);
        assert_eq!(report.status(), CertificateStatus::Abstained);
    }

    #[test]
    fn four_law_track_survives_nonproduct_quotas() {
        let report = run_tabular_audit(
            &load("examples/configs/four_law_nonproduct.json"),
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            Some(&workspace_root()),
        )
        .unwrap();
        assert_eq!(report.preflight.status, PreflightStatus::Ready);
        assert!(report.preflight.four_law_eligible);
        assert!(!report.preflight.product_factorial_eligible);
        assert!(!report.four_law.is_empty());
    }

    #[test]
    fn declared_state_dependent_selection_blocks_four_law() {
        let report = run_tabular_audit(
            &load("examples/configs/selection_dependent.json"),
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            Some(&workspace_root()),
        )
        .unwrap();
        assert_eq!(report.preflight.status, PreflightStatus::Blocked);
        assert!(!report.preflight.four_law_eligible);
        assert!(report.four_law.is_empty());
        assert_eq!(report.status(), CertificateStatus::Abstained);
    }

    #[test]
    fn both_track_on_nonproduct_blocks_before_projection_when_requested() {
        let mut manifest = load("examples/configs/four_law_nonproduct.json");
        manifest.inference_track = InferenceTrack::Both;
        let report = run_tabular_audit(
            &manifest,
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            Some(&workspace_root()),
        )
        .unwrap();
        assert_eq!(report.preflight.status, PreflightStatus::Blocked);
        assert!(!report.four_law.is_empty());
        assert_eq!(report.status(), CertificateStatus::Abstained);
    }

    #[test]
    fn spanning_cluster_blocks() {
        let dir = std::env::temp_dir().join("mic-engine-span");
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("span.csv");
        std::fs::write(
            &csv,
            "cluster_id,regime,x,included\nshared,00,0,1\nshared,10,1,1\nc1,01,0,1\nc2,11,1,1\n",
        )
        .unwrap();
        let manifest = ExperimentManifest {
            schema_version: "1.0.0".into(),
            experiment_id: "span".into(),
            strict: true,
            inference_track: InferenceTrack::FourLaw,
            selection: SelectionContract::StateIndependentWithinRegime,
            cluster_column: "cluster_id".into(),
            regime_column: "regime".into(),
            state_columns: vec!["x".into()],
            candidate_state_blocks: Vec::new(),
            regimes: ["00", "10", "01", "11"]
                .iter()
                .map(|label| RegimeSpec {
                    id: (*label).into(),
                    design: DesignPoint::parse(label).unwrap(),
                    sampling_proportion: 0.25,
                    perturbations: Vec::new(),
                })
                .collect(),
            data: DataSource {
                format: "csv".into(),
                path: csv,
            },
            seed: 1,
        };
        let report = run_tabular_audit(
            &manifest,
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            None,
        )
        .unwrap();
        assert!(report.four_law.is_empty());
        assert!(
            report
                .ledger
                .findings()
                .iter()
                .any(|finding| finding.code == "cluster_spans_regimes")
        );
    }

    #[test]
    fn incomplete_common_support_is_ledgers_and_not_renormalized_away() {
        let dir = std::env::temp_dir().join("mic-engine-incomplete-support");
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("incomplete.csv");
        std::fs::write(
            &csv,
            "cluster_id,regime,x,included\n\
             c00a,00,0,1\nc00b,00,0,1\nc00c,00,1,1\n\
             c10a,10,0,1\nc10b,10,0,1\nc10c,10,1,1\n\
             c01a,01,0,1\nc01b,01,0,1\n\
             c11a,11,0,1\nc11b,11,0,1\n",
        )
        .unwrap();
        let manifest = ExperimentManifest {
            schema_version: "1.0.0".into(),
            experiment_id: "incomplete-support".into(),
            strict: true,
            inference_track: InferenceTrack::FourLaw,
            selection: SelectionContract::StateIndependentWithinRegime,
            cluster_column: "cluster_id".into(),
            regime_column: "regime".into(),
            state_columns: vec!["x".into()],
            candidate_state_blocks: Vec::new(),
            regimes: ["00", "10", "01", "11"]
                .iter()
                .map(|label| RegimeSpec {
                    id: (*label).into(),
                    design: DesignPoint::parse(label).unwrap(),
                    sampling_proportion: 0.25,
                    perturbations: Vec::new(),
                })
                .collect(),
            data: DataSource {
                format: "csv".into(),
                path: csv,
            },
            seed: 3,
        };
        let report = run_tabular_audit(
            &manifest,
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            None,
        )
        .unwrap();
        let face = &report.four_law[0];
        assert!(face.incomplete_cells > 0);
        assert!(face.omitted_baseline_mass > 0.0);
        assert!(report.ledger().findings().iter().any(|finding| {
            finding.code == "incomplete_common_support"
                && finding.severity == Severity::Error
                && finding.context.contains_key("omitted_baseline_mass")
        }));
        assert_eq!(report.status(), CertificateStatus::Abstained);
    }

    #[test]
    fn overlap_audits_all_three_primitive_ratios() {
        let report = run_tabular_audit(
            &load("examples/configs/four_law_discrete.json"),
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            Some(&workspace_root()),
        )
        .unwrap();
        for stage in ["overlap_r_A@", "overlap_r_B@", "overlap_r_AB@"] {
            assert!(
                report
                    .ledger
                    .findings()
                    .iter()
                    .any(|finding| finding.stage.starts_with(stage)),
                "missing overlap audit for {stage}"
            );
        }
    }

    #[test]
    fn later_face_overlap_failure_is_not_hidden_by_the_first_face() {
        let dir = std::env::temp_dir().join("mic-engine-later-face-overlap");
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("cube.csv");
        let mut rows = String::from("cluster_id,regime,x,included\n");
        let balanced = ["000", "100", "010", "110", "011", "111"];
        for regime in balanced {
            for value in [0, 1] {
                for index in 0..20 {
                    let _ = writeln!(rows, "c{regime}{value}{index},{regime},{value},1");
                }
            }
        }
        for index in 0..20 {
            let _ = writeln!(rows, "c0010{index},001,0,1");
            let _ = writeln!(rows, "c1011{index},101,1,1");
        }
        rows.push_str("c0011only,001,1,1\n");
        rows.push_str("c1010only,101,0,1\n");
        std::fs::write(&csv, rows).unwrap();

        let labels = ["000", "001", "010", "011", "100", "101", "110", "111"];
        let manifest = ExperimentManifest {
            schema_version: "1.0.0".into(),
            experiment_id: "later-face-overlap".into(),
            strict: true,
            inference_track: InferenceTrack::FourLaw,
            selection: SelectionContract::StateIndependentWithinRegime,
            cluster_column: "cluster_id".into(),
            regime_column: "regime".into(),
            state_columns: vec!["x".into()],
            candidate_state_blocks: Vec::new(),
            regimes: labels
                .iter()
                .map(|label| RegimeSpec {
                    id: (*label).into(),
                    design: DesignPoint::parse(label).unwrap(),
                    sampling_proportion: 0.125,
                    perturbations: Vec::new(),
                })
                .collect(),
            data: DataSource {
                format: "csv".into(),
                path: csv,
            },
            seed: 11,
        };
        let report = run_tabular_audit(
            &manifest,
            FourLawPolicy::default(),
            PreflightPolicy::default(),
            None,
        )
        .unwrap();
        assert!(report.preflight.design.square_faces.len() > 1);
        assert!(
            report.four_law.len() > 1,
            "the 3-cube fixture must project more than one square"
        );
        let first_face = &report.preflight.design.square_faces[0];
        let first_prefix = format!(
            "overlap_r_A@{}:{}:{}",
            first_face.base.bit_string(),
            first_face.first,
            first_face.second
        );
        assert!(
            report.ledger().findings().iter().any(|finding| {
                finding.stage == first_prefix && finding.code == "overlap_adequate"
            }),
            "first face must remain an adequate-overlap control"
        );
        assert!(
            report.ledger().findings().iter().any(|finding| {
                finding.code == code::OVERLAP_FAILURE && !finding.stage.starts_with(&first_prefix)
            }),
            "a later face must be able to fail overlap after the first face passed"
        );
        assert_eq!(report.status(), CertificateStatus::Abstained);
    }
}
