//! Standard-library CSV ingest with cluster-level fingerprints.
//!
//! This is the default tabular reader. FrankenPandas remains a feature-gated
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
    #[error("std tabular reader supports csv only, got {0}; parquet/arrow require the franken adapter")]
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
    let mut lines = text.lines();
    let header_line = lines.next().ok_or(TableError::EmptyTable)?;
    let headers = parse_csv_line(header_line);
    let header_index = column_index(&headers);
    let required = required_columns(manifest);
    for column in &required {
        if !header_index.contains_key(column.as_str()) {
            return Err(TableError::MissingColumn(column.clone()));
        }
    }
    let regime_lookup = regime_lookup(manifest);
    let mut rows = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row_index = offset + 1;
        let fields = parse_csv_line(line);
        if fields.len() != headers.len() {
            return Err(TableError::Parse {
                row: row_index,
                message: format!(
                    "expected {} columns, found {}",
                    headers.len(),
                    fields.len()
                ),
            });
        }
        rows.push(parse_observation(
            manifest,
            &headers,
            &header_index,
            &regime_lookup,
            &path,
            row_index,
            &fields,
        )?);
    }
    if rows.is_empty() {
        return Err(TableError::EmptyTable);
    }
    Ok(summarize(manifest, &path, content_sha256, rows, n_folds))
}

/// Resolves a declared data path against an optional analysis root.
pub fn resolve_data_path(
    declared: &Path,
    base_dir: Option<&Path>,
) -> Result<PathBuf, TableError> {
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
    let regime_id = regime_lookup.get(raw_regime).cloned().ok_or_else(|| {
        TableError::Parse {
            row: row_index,
            message: format!("unknown regime label {raw_regime:?}"),
        }
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
    let row_id = header_index
        .get("row_id")
        .map(|&index| fields[index].clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{}:{row_index}", path.display()));
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
            fold: fold_for_cluster(manifest.seed, cluster_id, n_folds),
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
#[must_use]
pub fn fold_for_cluster(seed: u64, cluster_id: &str, n_folds: usize) -> usize {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_be_bytes());
    hasher.update(cluster_id.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    (u64::from_be_bytes(bytes) % n_folds as u64) as usize
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

fn column_index(headers: &[String]) -> BTreeMap<&str, usize> {
    headers
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect()
}

fn field<'a>(
    fields: &'a [String],
    header_index: &BTreeMap<&str, usize>,
    column: &str,
) -> &'a str {
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

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    fields.push(current.trim().to_string());
    fields
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
}
