#![forbid(unsafe_code)]
//! Standard-library CSV ingest with cluster-level fingerprints.
//!
//! This is the default tabular reader. `FrankenPandas` remains a feature-gated
//! Packet 1 adapter and is not required for four-law diagnostics.

use crate::{ExperimentManifest, ManifestError};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// One validated observation after manifest column mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    /// Zero-based file row after the header.
    pub row_index: usize,
    /// Stable row identifier: explicit `row_id` column, else `{path}:{line}`.
    pub row_id: String,
    /// Assignment-unit identifier from `cluster_column`.
    pub cluster_id: String,
    /// Manifest regime identifier, not necessarily the raw CSV token.
    pub regime_id: String,
    /// Inclusion indicator. Missing column defaults to included.
    pub included: bool,
    /// Current-state features in `manifest.state_columns` order.
    pub state: Vec<f64>,
}

/// Loaded table plus content-addressed fingerprints.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TableFingerprint {
    /// SHA-256 of the raw file bytes.
    pub content_sha256: String,
    /// SHA-256 of the sorted unique cluster identifiers.
    pub cluster_fingerprint: String,
    /// SHA-256 of the sorted included-cluster identifiers.
    pub included_cluster_fingerprint: String,
    /// Absolute resolved path.
    pub resolved_path: String,
    /// Number of data rows.
    pub n_rows: usize,
    /// Number of included rows.
    pub n_included_rows: usize,
    /// Number of distinct clusters.
    pub n_clusters: usize,
    /// Number of distinct included clusters.
    pub n_included_clusters: usize,
}

/// Realized counts for one declared regime.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RegimeCount {
    /// Manifest regime identifier.
    pub regime_id: String,
    /// Design bit string.
    pub design: String,
    /// Declared state-independent sampling proportion.
    pub declared_quota: f64,
    /// Included-row share.
    pub empirical_row_quota: f64,
    /// Included-cluster share. This is the randomization-unit share.
    pub empirical_cluster_quota: f64,
    /// Included rows.
    pub n_included_rows: usize,
    /// Included clusters.
    pub n_included_clusters: usize,
}

/// Deterministic cluster-level fold assignment.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClusterFold {
    /// Cluster identifier.
    pub cluster_id: String,
    /// Fold in `0..n_folds`.
    pub fold: usize,
}

/// Validated ingest of a CSV against a manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct IngestReport {
    /// Included and excluded observations.
    pub rows: Vec<Observation>,
    /// Content and unit fingerprints.
    pub fingerprint: TableFingerprint,
    /// Per-regime realized quotas.
    pub regime_counts: Vec<RegimeCount>,
    /// Clusters that appear under more than one regime.
    pub clusters_spanning_regimes: Vec<String>,
    /// Declared regimes with zero included clusters.
    pub missing_regimes: Vec<String>,
    /// Cluster-level fold assignment from the manifest seed.
    pub cluster_folds: Vec<ClusterFold>,
    /// Maximum absolute declared-versus-row-quota gap.
    pub max_row_quota_gap: f64,
    /// Maximum absolute declared-versus-cluster-quota gap.
    pub max_cluster_quota_gap: f64,
}

/// Tabular ingest failures.
#[derive(Debug, thiserror::Error)]
pub enum TableError {
    /// Manifest contract failed.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// Filesystem failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The declared format is not the std CSV reader.
    #[error(
        "std tabular reader supports csv only, got {0}; this format requires an explicit adapter"
    )]
    UnsupportedFormat(String),
    /// The data file could not be resolved.
    #[error("data file not found: {0}")]
    MissingFile(String),
    /// A required column is absent.
    #[error("csv is missing required column {0}")]
    MissingColumn(String),
    /// A cell could not be parsed.
    #[error("row {row}: {message}")]
    Parse {
        /// Offending data row (1-based file line minus header).
        row: usize,
        /// Parse detail.
        message: String,
    },
    /// The file has no data rows.
    #[error("csv contains no data rows")]
    EmptyTable,
    /// A numeric state value was not finite.
    #[error("row {row} column {column} is not a finite number: {value}")]
    NonFiniteState {
        /// Offending data row.
        row: usize,
        /// Column name.
        column: String,
        /// Rejected token.
        value: String,
    },
}

/// Strips a leading UTF-8 byte-order mark.
///
/// Spreadsheet exports routinely prepend one, and without this the first header name
/// silently carries an invisible `U+FEFF`. The column lookup then misses, and the run
/// fails with "missing required column `cluster_id`" naming a column that is plainly
/// present in the file — a confusing failure for a very common input. Stripping it here
/// keeps the header names the reader sees identical to the ones a human sees.
///
/// The content fingerprint is taken over the raw bytes before this runs, so the recorded
/// hash still identifies the file exactly as delivered.
fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// Loads a CSV declared by the manifest and fingerprints rows and clusters.
pub fn load_csv_table(
    manifest: &ExperimentManifest,
    base_dir: Option<&Path>,
    n_folds: usize,
) -> Result<IngestReport, TableError> {
    manifest.validate()?;
    if manifest.data.format != "csv" {
        return Err(TableError::UnsupportedFormat(manifest.data.format.clone()));
    }
    if n_folds == 0 {
        return Err(TableError::Parse {
            row: 0,
            message: "n_folds must be positive".into(),
        });
    }
    let path = resolve_data_path(&manifest.data.path, base_dir)?;
    let bytes = fs::read(&path)?;
    let content_sha256 = hex_sha256(&bytes);
    let text = String::from_utf8(bytes).map_err(|_| TableError::Parse {
        row: 0,
        message: "csv is not valid UTF-8".into(),
    })?;
    let mut lines = strip_bom(&text).lines();
    let header_line = lines.next().ok_or(TableError::EmptyTable)?;
    let headers =
        parse_csv_line(header_line).map_err(|message| TableError::Parse { row: 0, message })?;
    let mut records = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row_index = offset + 1;
        records.push(parse_csv_line(line).map_err(|message| TableError::Parse {
            row: row_index,
            message,
        })?);
    }
    build_ingest_report(manifest, &path, content_sha256, &headers, &records, n_folds)
}

/// Builds an [`IngestReport`] from already-tokenized cells.
///
/// Every semantic decision lives here: column requirements, regime mapping, row
/// identity, state parsing, and the cluster-level aggregation that produces
/// `clusters_spanning_regimes`. A tabular backend supplies header and cell strings
/// and nothing else.
///
/// That division is deliberate and load-bearing. The refusal chain for a
/// regime-spanning cluster — the `cluster_spans_regimes` finding, the skipped
/// projection, the abstained certificate — is conditioned entirely on the
/// `clusters_spanning_regimes` field, and nothing downstream re-derives it. A backend
/// that computed that field for itself could silently return an empty vector (a
/// group-by keyed on cluster id with last-write-wins on regime does exactly that), and
/// the whole chain would evaporate with no error anywhere, leaving cells treated as
/// independent in violation of the randomization-unit rule. Sharing this function makes
/// that divergence structurally impossible rather than merely tested for.
pub fn build_ingest_report(
    manifest: &ExperimentManifest,
    path: &Path,
    content_sha256: String,
    headers: &[String],
    records: &[Vec<String>],
    n_folds: usize,
) -> Result<IngestReport, TableError> {
    // Validated here rather than only in the callers: this is a public entry point that
    // does not go through `load_csv_table`, and the fold assignment below is infallible
    // only for a positive count. Returning the same error keeps every route to a zero
    // fold count identical, and keeps a bad argument a refusal rather than a panic.
    if n_folds == 0 {
        return Err(TableError::Parse {
            row: 0,
            message: "n_folds must be positive".into(),
        });
    }
    let header_index = column_index(headers)?;
    let required = required_columns(manifest);
    for column in &required {
        if !header_index.contains_key(column.as_str()) {
            return Err(TableError::MissingColumn(column.clone()));
        }
    }
    let regime_lookup = regime_lookup(manifest);
    let mut rows = Vec::new();
    let mut row_ids = BTreeSet::new();
    for (offset, fields) in records.iter().enumerate() {
        let row_index = offset + 1;
        if fields.len() != headers.len() {
            return Err(TableError::Parse {
                row: row_index,
                message: format!("expected {} columns, found {}", headers.len(), fields.len()),
            });
        }
        let observation = parse_observation(
            manifest,
            headers,
            &header_index,
            &regime_lookup,
            path,
            row_index,
            fields,
        )?;
        if !row_ids.insert(observation.row_id.clone()) {
            return Err(TableError::Parse {
                row: row_index,
                message: format!("duplicate row identifier {:?}", observation.row_id),
            });
        }
        rows.push(observation);
    }
    if rows.is_empty() {
        return Err(TableError::EmptyTable);
    }
    Ok(summarize(manifest, path, content_sha256, rows, n_folds))
}

/// Untyped CSV used by unsupervised column triage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTable {
    /// Header names in file order.
    pub headers: Vec<String>,
    /// Data rows aligned to `headers`.
    pub rows: Vec<Vec<String>>,
    /// Resolved path.
    pub path: PathBuf,
    /// SHA-256 of the raw file bytes.
    pub content_sha256: String,
}

/// Loads a CSV without a manifest. Used only to *propose* designs.
pub fn load_raw_csv(
    path: impl AsRef<Path>,
    base_dir: Option<&Path>,
) -> Result<RawTable, TableError> {
    let path = resolve_data_path(path.as_ref(), base_dir)?;
    let bytes = fs::read(&path)?;
    let content_sha256 = hex_sha256(&bytes);
    let text = String::from_utf8(bytes).map_err(|_| TableError::Parse {
        row: 0,
        message: "csv is not valid UTF-8".into(),
    })?;
    let mut lines = strip_bom(&text).lines();
    let header_line = lines.next().ok_or(TableError::EmptyTable)?;
    let headers =
        parse_csv_line(header_line).map_err(|message| TableError::Parse { row: 0, message })?;
    if headers.is_empty() || headers.iter().all(String::is_empty) {
        return Err(TableError::Parse {
            row: 0,
            message: "csv header is empty".into(),
        });
    }
    let _ = column_index(&headers)?;
    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line).map_err(|message| TableError::Parse {
            row: offset + 1,
            message,
        })?;
        if fields.len() != headers.len() {
            return Err(TableError::Parse {
                row: offset + 1,
                message: format!("expected {} columns, found {}", headers.len(), fields.len()),
            });
        }
        rows.push(fields);
    }
    if rows.is_empty() {
        return Err(TableError::EmptyTable);
    }
    Ok(RawTable {
        headers,
        rows,
        path,
        content_sha256,
    })
}

/// Resolves a declared data path against an optional analysis root.
pub fn resolve_data_path(declared: &Path, base_dir: Option<&Path>) -> Result<PathBuf, TableError> {
    let mut candidates = Vec::new();
    candidates.push(declared.to_path_buf());
    if let Some(base) = base_dir {
        candidates.push(base.join(declared));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(declared));
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.join(declared));
            if let Some(grand) = parent.parent() {
                candidates.push(grand.join(declared));
            }
        }
    }
    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(TableError::MissingFile(declared.display().to_string()))
}

fn required_columns(manifest: &ExperimentManifest) -> Vec<String> {
    let mut columns = vec![
        manifest.cluster_column.clone(),
        manifest.regime_column.clone(),
    ];
    columns.extend(manifest.state_columns.iter().cloned());
    columns
}

fn regime_lookup(manifest: &ExperimentManifest) -> BTreeMap<String, String> {
    let mut lookup = BTreeMap::new();
    for regime in &manifest.regimes {
        lookup.insert(regime.id.clone(), regime.id.clone());
        lookup.insert(regime.design.bit_string(), regime.id.clone());
    }
    lookup
}

fn parse_observation(
    manifest: &ExperimentManifest,
    headers: &[String],
    header_index: &BTreeMap<&str, usize>,
    regime_lookup: &BTreeMap<String, String>,
    path: &Path,
    row_index: usize,
    fields: &[String],
) -> Result<Observation, TableError> {
    let cluster_id = field(fields, header_index, &manifest.cluster_column).to_string();
    if cluster_id.trim().is_empty() {
        return Err(TableError::Parse {
            row: row_index,
            message: "cluster identifier is empty".into(),
        });
    }
    let raw_regime = field(fields, header_index, &manifest.regime_column);
    let regime_id = regime_lookup
        .get(raw_regime)
        .cloned()
        .ok_or_else(|| TableError::Parse {
            row: row_index,
            message: format!("unknown regime label {raw_regime:?}"),
        })?;
    let included = match header_index.get("included") {
        Some(&index) => parse_flag(&fields[index], row_index)?,
        None => true,
    };
    let mut state = Vec::with_capacity(manifest.state_columns.len());
    for column in &manifest.state_columns {
        let raw = field(fields, header_index, column);
        let value = raw.parse::<f64>().map_err(|_| TableError::NonFiniteState {
            row: row_index,
            column: column.clone(),
            value: raw.to_string(),
        })?;
        if !value.is_finite() {
            return Err(TableError::NonFiniteState {
                row: row_index,
                column: column.clone(),
                value: raw.to_string(),
            });
        }
        state.push(value);
    }
    let row_id = if let Some(&index) = header_index.get("row_id") {
        let value = fields[index].trim();
        if value.is_empty() {
            return Err(TableError::Parse {
                row: row_index,
                message: "row identifier is empty".into(),
            });
        }
        value.to_string()
    } else {
        format!("{}:{row_index}", path.display())
    };
    let _ = headers;
    Ok(Observation {
        row_index,
        row_id,
        cluster_id,
        regime_id,
        included,
        state,
    })
}

fn summarize(
    manifest: &ExperimentManifest,
    path: &Path,
    content_sha256: String,
    rows: Vec<Observation>,
    n_folds: usize,
) -> IngestReport {
    let mut clusters = BTreeSet::new();
    let mut included_clusters = BTreeSet::new();
    let mut cluster_regimes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut included_rows = 0usize;
    for row in &rows {
        clusters.insert(row.cluster_id.clone());
        if row.included {
            included_rows += 1;
            included_clusters.insert(row.cluster_id.clone());
            cluster_regimes
                .entry(row.cluster_id.clone())
                .or_default()
                .insert(row.regime_id.clone());
        }
    }
    let clusters_spanning_regimes = cluster_regimes
        .iter()
        .filter(|(_, regimes)| regimes.len() > 1)
        .map(|(cluster, _)| cluster.clone())
        .collect::<Vec<_>>();
    let mut counts: BTreeMap<String, (usize, BTreeSet<String>)> = BTreeMap::new();
    for row in &rows {
        if !row.included {
            continue;
        }
        let entry = counts
            .entry(row.regime_id.clone())
            .or_insert_with(|| (0, BTreeSet::new()));
        entry.0 += 1;
        entry.1.insert(row.cluster_id.clone());
    }
    let total_included_clusters = included_clusters.len().max(1);
    let mut regime_counts = Vec::new();
    let mut missing_regimes = Vec::new();
    for regime in &manifest.regimes {
        let (n_rows, clusters_in_regime) = counts
            .get(&regime.id)
            .cloned()
            .unwrap_or_else(|| (0, BTreeSet::new()));
        if clusters_in_regime.is_empty() {
            missing_regimes.push(regime.id.clone());
        }
        regime_counts.push(RegimeCount {
            regime_id: regime.id.clone(),
            design: regime.design.bit_string(),
            declared_quota: regime.sampling_proportion,
            empirical_row_quota: if included_rows == 0 {
                0.0
            } else {
                n_rows as f64 / included_rows as f64
            },
            empirical_cluster_quota: clusters_in_regime.len() as f64
                / total_included_clusters as f64,
            n_included_rows: n_rows,
            n_included_clusters: clusters_in_regime.len(),
        });
    }
    let max_row_quota_gap = regime_counts
        .iter()
        .map(|count| (count.empirical_row_quota - count.declared_quota).abs())
        .fold(0.0, f64::max);
    let max_cluster_quota_gap = regime_counts
        .iter()
        .map(|count| (count.empirical_cluster_quota - count.declared_quota).abs())
        .fold(0.0, f64::max);
    let mut cluster_folds = included_clusters
        .iter()
        .map(|cluster_id| ClusterFold {
            cluster_id: cluster_id.clone(),
            fold: fold_for_cluster(manifest.seed, cluster_id, n_folds)
                .expect("build_ingest_report rejects a zero fold count before reaching here"),
        })
        .collect::<Vec<_>>();
    cluster_folds.sort_by(|left, right| left.cluster_id.cmp(&right.cluster_id));
    IngestReport {
        fingerprint: TableFingerprint {
            content_sha256,
            cluster_fingerprint: fingerprint_ids(&clusters),
            included_cluster_fingerprint: fingerprint_ids(&included_clusters),
            resolved_path: path.display().to_string(),
            n_rows: rows.len(),
            n_included_rows: included_rows,
            n_clusters: clusters.len(),
            n_included_clusters: included_clusters.len(),
        },
        rows,
        regime_counts,
        clusters_spanning_regimes,
        missing_regimes,
        cluster_folds,
        max_row_quota_gap,
        max_cluster_quota_gap,
    }
}

/// Assigns a cluster to a fold from the experiment seed. Rows never receive folds.
///
/// Returns `None` when `n_folds` is zero.
#[must_use]
pub fn fold_for_cluster(seed: u64, cluster_id: &str, n_folds: usize) -> Option<usize> {
    if n_folds == 0 {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(seed.to_be_bytes());
    hasher.update(cluster_id.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    // Both conversions are total: the remainder is strictly below `n_folds`, which
    // arrived as a `usize`, so neither direction can truncate on any target width.
    let modulus = u64::try_from(n_folds).expect("fold count originates from a usize");
    let fold = u64::from_be_bytes(bytes) % modulus;
    Some(usize::try_from(fold).expect("fold index is below the fold count, which is a usize"))
}

fn fingerprint_ids(ids: &BTreeSet<String>) -> String {
    let mut hasher = Sha256::new();
    hasher.update((ids.len() as u64).to_be_bytes());
    for id in ids {
        hasher.update((id.len() as u64).to_be_bytes());
        hasher.update(id.as_bytes());
    }
    hex_sha256_digest(hasher.finalize())
}

fn hex_sha256(bytes: &[u8]) -> String {
    hex_sha256_digest(Sha256::digest(bytes))
}

fn hex_sha256_digest(digest: impl AsRef<[u8]>) -> String {
    let digest = digest.as_ref();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn column_index(headers: &[String]) -> Result<BTreeMap<&str, usize>, TableError> {
    let mut index = BTreeMap::new();
    for (position, name) in headers.iter().enumerate() {
        if name.is_empty() {
            return Err(TableError::Parse {
                row: 0,
                message: format!("csv header column {} is empty", position + 1),
            });
        }
        if index.insert(name.as_str(), position).is_some() {
            return Err(TableError::Parse {
                row: 0,
                message: format!("duplicate csv header {name:?}"),
            });
        }
    }
    Ok(index)
}

fn field<'a>(fields: &'a [String], header_index: &BTreeMap<&str, usize>, column: &str) -> &'a str {
    &fields[header_index[column]]
}

fn parse_flag(raw: &str, row: usize) -> Result<bool, TableError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => Ok(true),
        "0" | "false" | "no" | "n" => Ok(false),
        other => Err(TableError::Parse {
            row,
            message: format!("included must be boolean, got {other:?}"),
        }),
    }
}

/// Tokenizes a whole CSV with the standard reader's rules, raising its exact errors.
///
/// The `FrankenPandas` adapter runs this before handing the text to the sibling, so that
/// malformed input is rejected by one implementation with one set of messages. Without
/// it the two backends refuse the same files for different stated reasons — duplicate
/// headers, short rows, extra fields, and unterminated quotes each produced a different
/// diagnostic — which makes "the adapter behaves identically" false in the way that is
/// hardest to notice, because both sides still refuse and only the wording differs.
#[cfg(feature = "franken")]
pub(crate) fn tokenize_csv_with_std_rules(
    text: &str,
) -> Result<(Vec<String>, Vec<Vec<String>>), TableError> {
    let mut lines = strip_bom(text).lines();
    let header_line = lines.next().ok_or(TableError::EmptyTable)?;
    let headers =
        parse_csv_line(header_line).map_err(|message| TableError::Parse { row: 0, message })?;
    // Raises the empty-header and duplicate-header errors before the sibling sees anything.
    column_index(&headers)?;
    let mut records = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row_index = offset + 1;
        let fields = parse_csv_line(line).map_err(|message| TableError::Parse {
            row: row_index,
            message,
        })?;
        if fields.len() != headers.len() {
            return Err(TableError::Parse {
                row: row_index,
                message: format!("expected {} columns, found {}", headers.len(), fields.len()),
            });
        }
        records.push(fields);
    }
    Ok((headers, records))
}

fn parse_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_closed = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if quote_closed {
            match ch {
                ',' => {
                    fields.push(current.trim().to_string());
                    current.clear();
                    quote_closed = false;
                }
                ch if ch.is_whitespace() => {}
                _ => return Err("characters after a closing csv quote".into()),
            }
            continue;
        }
        match ch {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                    quote_closed = true;
                }
            }
            '"' if current.is_empty() => in_quotes = true,
            '"' => {
                return Err("quote inside an unquoted csv field".into());
            }
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if in_quotes {
        return Err(
            "unterminated quoted csv field; the standard-library reader does not support quoted newlines"
                .into(),
        );
    }
    fields.push(current.trim().to_string());
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataSource, InferenceTrack, RegimeSpec, SelectionContract};
    use mic_design::DesignPoint;

    fn manifest_for(path: &Path) -> ExperimentManifest {
        ExperimentManifest {
            schema_version: "1.0.0".into(),
            experiment_id: "table-test".into(),
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
                path: path.to_path_buf(),
            },
            seed: 7,
        }
    }

    #[test]
    fn loads_bitstring_regimes_and_fingerprints_clusters() {
        let dir = std::env::temp_dir().join("mic-data-table-test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tiny.csv");
        fs::write(
            &path,
            "cluster_id,regime,x,included\n\
             c0,00,0,1\n\
             c1,10,1,1\n\
             c2,01,0,1\n\
             c3,11,1,1\n",
        )
        .unwrap();
        let report = load_csv_table(&manifest_for(&path), None, 5).unwrap();
        assert_eq!(report.fingerprint.n_rows, 4);
        assert_eq!(report.fingerprint.n_included_clusters, 4);
        assert!(report.clusters_spanning_regimes.is_empty());
        assert_eq!(report.cluster_folds.len(), 4);
        assert!(report.cluster_folds.iter().all(|fold| fold.fold < 5));
        assert_eq!(
            fold_for_cluster(7, "c0", 5),
            fold_for_cluster(7, "c0", 5),
            "folds are deterministic"
        );
        assert_eq!(fold_for_cluster(7, "c0", 0), None);
    }

    #[test]
    fn rejects_a_cluster_that_changes_regime() {
        let dir = std::env::temp_dir().join("mic-data-table-span");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("span.csv");
        fs::write(
            &path,
            "cluster_id,regime,x,included\n\
             shared,00,0,1\n\
             shared,10,1,1\n\
             c1,01,0,1\n\
             c2,11,1,1\n",
        )
        .unwrap();
        let report = load_csv_table(&manifest_for(&path), None, 2).unwrap();
        assert_eq!(report.clusters_spanning_regimes, vec!["shared".to_string()]);
    }

    #[test]
    fn build_ingest_report_refuses_zero_folds_instead_of_panicking() {
        // `build_ingest_report` is a public entry point that bypasses `load_csv_table`,
        // so it cannot rely on a caller having checked the fold count. Before this guard
        // it reached `fold_for_cluster(..).expect(..)` and panicked on a plain argument.
        let dir = std::env::temp_dir().join("mic-data-zero-folds");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zero_folds.csv");
        fs::write(&path, "cluster_id,regime,x,included\nc0,00,0.5,1\n").unwrap();
        let manifest = manifest_for(&path);
        let headers: Vec<String> = ["cluster_id", "regime", "x", "included"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let records = vec![
            ["c0", "00", "0.5", "1"]
                .iter()
                .map(|s| (*s).to_string())
                .collect::<Vec<String>>(),
        ];
        let error = build_ingest_report(
            &manifest,
            &path,
            "fingerprint".into(),
            &headers,
            &records,
            0,
        )
        .unwrap_err();
        assert!(error.to_string().contains("n_folds must be positive"));
        // Identical to the error the file-reading entry point produces.
        let via_file = load_csv_table(&manifest, None, 0).unwrap_err();
        assert_eq!(error.to_string(), via_file.to_string());
    }

    #[test]
    fn reads_a_csv_with_a_utf8_byte_order_mark() {
        // Spreadsheet exports prepend a BOM. Without stripping it the first header name
        // carries an invisible U+FEFF and the run fails with "missing required column
        // cluster_id" while pointing at a column the file plainly contains.
        let dir = std::env::temp_dir().join("mic-data-bom");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bom.csv");
        fs::write(
            &path,
            "\u{feff}cluster_id,regime,x,included\nc0,00,0.5,1\nc1,10,0.25,1\n",
        )
        .unwrap();
        let report = load_csv_table(&manifest_for(&path), None, 2).unwrap();
        assert_eq!(report.fingerprint.n_rows, 2);
        assert_eq!(report.rows[0].cluster_id, "c0");
    }

    #[test]
    fn rejects_duplicate_headers_before_semantic_lookup() {
        let dir = std::env::temp_dir().join("mic-data-table-duplicate-header");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("duplicate-header.csv");
        fs::write(
            &path,
            "cluster_id,regime,x,x\n\
             c0,00,0,0\n",
        )
        .unwrap();
        let error = load_csv_table(&manifest_for(&path), None, 2).unwrap_err();
        assert!(error.to_string().contains("duplicate csv header \"x\""));
    }

    #[test]
    fn rejects_duplicate_explicit_row_identifiers() {
        let dir = std::env::temp_dir().join("mic-data-table-duplicate-row-id");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("duplicate-row-id.csv");
        fs::write(
            &path,
            "row_id,cluster_id,regime,x\n\
             same,c0,00,0\n\
             same,c1,10,1\n",
        )
        .unwrap();
        let error = load_csv_table(&manifest_for(&path), None, 2).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("duplicate row identifier \"same\"")
        );
    }

    #[test]
    fn rejects_empty_explicit_row_identifiers() {
        let dir = std::env::temp_dir().join("mic-data-table-empty-row-id");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty-row-id.csv");
        fs::write(
            &path,
            "row_id,cluster_id,regime,x\n\
             ,c0,00,0\n",
        )
        .unwrap();
        let error = load_csv_table(&manifest_for(&path), None, 2).unwrap_err();
        assert!(error.to_string().contains("row identifier is empty"));
    }

    #[test]
    fn rejects_unterminated_or_trailing_quote_content() {
        let dir = std::env::temp_dir().join("mic-data-table-malformed-quotes");
        fs::create_dir_all(&dir).unwrap();

        let unterminated = dir.join("unterminated.csv");
        fs::write(
            &unterminated,
            "cluster_id,regime,x\n\
             c0,00,\"unterminated\n",
        )
        .unwrap();
        let error = load_raw_csv(&unterminated, None).unwrap_err();
        assert!(error.to_string().contains("unterminated quoted csv field"));

        let trailing = dir.join("trailing.csv");
        fs::write(
            &trailing,
            "cluster_id,regime,x\n\
             c0,00,\"quoted\"tail\n",
        )
        .unwrap();
        let error = load_raw_csv(&trailing, None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("characters after a closing csv quote")
        );
    }
}
