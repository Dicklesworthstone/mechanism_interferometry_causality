#![forbid(unsafe_code)]
//! Machine-readable experiment contracts and input validation.

use mic_design::DesignPoint;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

mod table;
pub use table::{
    ClusterFold, IngestReport, Observation, RawTable, RegimeCount, TableError, TableFingerprint,
    fold_for_cluster, load_csv_table, load_raw_csv, resolve_data_path,
};

/// Requested inferential track.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InferenceTrack {
    /// Functionals of four normalized regime laws.
    FourLaw,
    /// Residual-product tests under product factorial sampling.
    ProductFactorial,
    /// Run both when contracts permit.
    Both,
}

/// Whether inclusion can depend on state within regime.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionContract {
    /// Inclusion is known to be independent of state conditional on regime.
    StateIndependentWithinRegime,
    /// A validated selection model is supplied separately.
    Modeled,
    /// The contract is unknown and strict causal inference must abstain.
    Unknown,
    /// Inclusion is known to depend on state and is not modeled.
    StateDependentUnmodeled,
}

/// One regime/corner declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegimeSpec {
    /// Stable regime identifier.
    pub id: String,
    /// Boolean design corner.
    pub design: DesignPoint,
    /// Known or empirical state-independent pooled sampling proportion.
    pub sampling_proportion: f64,
    /// Optional human-readable perturbation names.
    #[serde(default)]
    pub perturbations: Vec<String>,
}

/// Data source declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataSource {
    /// CSV, Parquet, Arrow, JSONL, or synthetic.
    pub format: String,
    /// Repository-relative or absolute source path.
    pub path: PathBuf,
}

/// Complete experiment manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExperimentManifest {
    /// Schema version.
    pub schema_version: String,
    /// Stable experiment identifier.
    pub experiment_id: String,
    /// Requested strictness.
    pub strict: bool,
    /// Requested inference track.
    pub inference_track: InferenceTrack,
    /// Within-regime selection contract.
    pub selection: SelectionContract,
    /// Randomization/cluster identifier column.
    pub cluster_column: String,
    /// Regime-label column.
    pub regime_column: String,
    /// Feature columns used as current state.
    pub state_columns: Vec<String>,
    /// Optional candidate state-expansion blocks.
    #[serde(default)]
    pub candidate_state_blocks: Vec<Vec<String>>,
    /// Declared regimes.
    pub regimes: Vec<RegimeSpec>,
    /// Input data.
    pub data: DataSource,
    /// Deterministic root seed.
    pub seed: u64,
}

impl ExperimentManifest {
    /// Parses and validates a JSON manifest.
    pub fn from_json_path(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let bytes = std::fs::read(path)?;
        let manifest: Self = serde_json::from_slice(&bytes)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validates structural contracts independent of loading the dataset.
    pub fn validate(&self) -> Result<(), ManifestError> {
        const FORMATS: [&str; 6] = ["csv", "jsonl", "parquet", "arrow", "feather", "synthetic"];
        if self.schema_version != "1.0.0" {
            return Err(ManifestError::UnsupportedSchemaVersion(
                self.schema_version.clone(),
            ));
        }
        if self.experiment_id.trim().is_empty() {
            return Err(ManifestError::MissingExperimentId);
        }
        self.validate_columns()?;
        self.validate_regimes()?;
        if !FORMATS.contains(&self.data.format.as_str()) {
            return Err(ManifestError::UnsupportedDataFormat(
                self.data.format.clone(),
            ));
        }
        if self.data.path.as_os_str().is_empty() {
            return Err(ManifestError::MissingDataPath);
        }
        Ok(())
    }

    fn validate_columns(&self) -> Result<(), ManifestError> {
        if self.state_columns.is_empty() {
            return Err(ManifestError::NoStateColumns);
        }
        if self.cluster_column.trim().is_empty() || self.regime_column.trim().is_empty() {
            return Err(ManifestError::MissingColumnRole);
        }
        if self.cluster_column == self.regime_column {
            return Err(ManifestError::ColumnRoleCollision(
                self.cluster_column.clone(),
            ));
        }
        let semantic_columns =
            BTreeSet::from([self.cluster_column.as_str(), self.regime_column.as_str()]);
        let mut state_columns = BTreeSet::new();
        for column in &self.state_columns {
            if column.trim().is_empty() {
                return Err(ManifestError::EmptyStateColumn);
            }
            if semantic_columns.contains(column.as_str()) {
                return Err(ManifestError::ColumnRoleCollision(column.clone()));
            }
            if !state_columns.insert(column.as_str()) {
                return Err(ManifestError::DuplicateStateColumn(column.clone()));
            }
        }
        for block in &self.candidate_state_blocks {
            if block.is_empty() {
                return Err(ManifestError::EmptyCandidateStateBlock);
            }
            let mut block_columns = BTreeSet::new();
            for column in block {
                if column.trim().is_empty() {
                    return Err(ManifestError::EmptyStateColumn);
                }
                if !block_columns.insert(column.as_str()) {
                    return Err(ManifestError::DuplicateCandidateColumn(column.clone()));
                }
                if semantic_columns.contains(column.as_str()) {
                    return Err(ManifestError::ColumnRoleCollision(column.clone()));
                }
                if state_columns.contains(column.as_str()) {
                    return Err(ManifestError::CandidateAlreadyInState(column.clone()));
                }
            }
        }
        Ok(())
    }

    fn validate_regimes(&self) -> Result<(), ManifestError> {
        if self.regimes.is_empty() {
            return Err(ManifestError::NoRegimes);
        }
        let dimension = self.regimes[0].design.dimension();
        if dimension == 0 {
            return Err(ManifestError::EmptyDesignPoint);
        }
        let mut ids = BTreeSet::new();
        let mut corners = BTreeSet::new();
        let mut label_owners = BTreeMap::new();
        let mut probability_sum = 0.0;
        for regime in &self.regimes {
            if regime.id.trim().is_empty() {
                return Err(ManifestError::EmptyRegimeId);
            }
            if !ids.insert(regime.id.clone()) {
                return Err(ManifestError::DuplicateRegimeId(regime.id.clone()));
            }
            if !corners.insert(regime.design.clone()) {
                return Err(ManifestError::DuplicateCorner(regime.design.bit_string()));
            }
            for label in [regime.id.clone(), regime.design.bit_string()] {
                if let Some(owner) = label_owners.insert(label.clone(), regime.id.clone())
                    && owner != regime.id
                {
                    return Err(ManifestError::AmbiguousRegimeLabel(label));
                }
            }
            if regime.design.dimension() != dimension {
                return Err(ManifestError::DimensionMismatch);
            }
            if !regime.sampling_proportion.is_finite() || regime.sampling_proportion <= 0.0 {
                return Err(ManifestError::InvalidSamplingProportion {
                    regime: regime.id.clone(),
                    value: regime.sampling_proportion,
                });
            }
            let unique_perturbations: BTreeSet<&str> =
                regime.perturbations.iter().map(String::as_str).collect();
            if unique_perturbations.len() != regime.perturbations.len() {
                return Err(ManifestError::DuplicatePerturbation(regime.id.clone()));
            }
            probability_sum += regime.sampling_proportion;
        }
        if (probability_sum - 1.0).abs() > 1e-8 {
            return Err(ManifestError::SamplingProportionsDoNotSumToOne(
                probability_sum,
            ));
        }
        Ok(())
    }
}

/// Manifest parse and contract errors.
#[derive(Debug, Error)]
pub enum ManifestError {
    /// The schema version is not supported.
    #[error("unsupported manifest schema version {0}")]
    UnsupportedSchemaVersion(String),
    /// Experiment identifier is empty.
    #[error("experiment_id must be nonempty")]
    MissingExperimentId,
    /// Input could not be read.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON could not be parsed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// No regimes were declared.
    #[error("manifest declares no regimes")]
    NoRegimes,
    /// No state features were declared.
    #[error("manifest declares no state columns")]
    NoStateColumns,
    /// A state column name is empty.
    #[error("state column names must be nonempty")]
    EmptyStateColumn,
    /// A state column was repeated.
    #[error("duplicate state column {0}")]
    DuplicateStateColumn(String),
    /// A candidate state block is empty.
    #[error("candidate state blocks must not be empty")]
    EmptyCandidateStateBlock,
    /// A candidate block repeated a column.
    #[error("duplicate candidate state column {0}")]
    DuplicateCandidateColumn(String),
    /// A candidate state column is already present in the base state.
    #[error("candidate state column {0} is already in state_columns")]
    CandidateAlreadyInState(String),
    /// A required semantic column name is empty.
    #[error("cluster_column and regime_column must be nonempty")]
    MissingColumnRole,
    /// One physical column was assigned incompatible semantic roles.
    #[error("column {0} is assigned more than one semantic role")]
    ColumnRoleCollision(String),
    /// A regime identifier is empty.
    #[error("regime id must be nonempty")]
    EmptyRegimeId,
    /// Duplicate regime identifier.
    #[error("duplicate regime id {0}")]
    DuplicateRegimeId(String),
    /// Duplicate design corner.
    #[error("duplicate design corner {0}")]
    DuplicateCorner(String),
    /// A regime identifier aliases another regime's design bit string.
    #[error("regime label {0} is ambiguous between an id and a design bit string")]
    AmbiguousRegimeLabel(String),
    /// A design point contains no coordinates.
    #[error("regime design points must contain at least one bit")]
    EmptyDesignPoint,
    /// Inconsistent design dimensions.
    #[error("regime design dimensions do not match")]
    DimensionMismatch,
    /// Invalid sampling proportion.
    #[error("regime {regime} has invalid sampling proportion {value}")]
    InvalidSamplingProportion {
        /// Identifier of the offending regime.
        regime: String,
        /// Rejected proportion value.
        value: f64,
    },
    /// Sampling proportions did not normalize.
    #[error("sampling proportions sum to {0}, expected 1")]
    SamplingProportionsDoNotSumToOne(f64),
    /// A regime repeats a perturbation label.
    #[error("regime {0} contains duplicate perturbation labels")]
    DuplicatePerturbation(String),
    /// Unsupported tabular format.
    #[error("unsupported data format {0}")]
    UnsupportedDataFormat(String),
    /// Input path is empty.
    #[error("data.path must be nonempty")]
    MissingDataPath,
}

/// FrankenPandas integration marker and future typed reader boundary.
#[cfg(feature = "franken")]
pub mod franken {
    /// The reviewed sibling revision for audit provenance.
    pub const REVISION: &str = "a9f8d86c9e52923b9b2082d00a65841862d5ca9a";

    /// Returns the selected tabular backend name.
    #[must_use]
    pub const fn backend_name() -> &'static str {
        "FrankenPandas"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_manifest() -> ExperimentManifest {
        ExperimentManifest {
            schema_version: "1.0.0".into(),
            experiment_id: "test".into(),
            strict: true,
            inference_track: InferenceTrack::Both,
            selection: SelectionContract::StateIndependentWithinRegime,
            cluster_column: "cluster".into(),
            regime_column: "regime".into(),
            state_columns: vec!["x".into()],
            candidate_state_blocks: Vec::new(),
            regimes: vec![
                RegimeSpec {
                    id: "0".into(),
                    design: DesignPoint::parse("0").unwrap(),
                    sampling_proportion: 0.5,
                    perturbations: Vec::new(),
                },
                RegimeSpec {
                    id: "1".into(),
                    design: DesignPoint::parse("1").unwrap(),
                    sampling_proportion: 0.5,
                    perturbations: vec!["A".into()],
                },
            ],
            data: DataSource {
                format: "synthetic".into(),
                path: "none".into(),
            },
            seed: 7,
        }
    }

    #[test]
    fn validates_minimal_manifest() {
        let manifest = minimal_manifest();
        manifest.validate().unwrap();
    }

    #[test]
    fn rejects_colliding_semantic_column_roles() {
        for mutate in [
            ("cluster/regime", 0_u8),
            ("cluster/state", 1_u8),
            ("regime/candidate", 2_u8),
        ] {
            let mut manifest = minimal_manifest();
            match mutate.1 {
                0 => manifest.cluster_column = manifest.regime_column.clone(),
                1 => manifest.state_columns = vec![manifest.cluster_column.clone()],
                2 => manifest.candidate_state_blocks = vec![vec![manifest.regime_column.clone()]],
                _ => unreachable!(),
            }
            let error = manifest.validate().unwrap_err();
            assert!(
                matches!(error, ManifestError::ColumnRoleCollision(_)),
                "{} unexpectedly produced {error}",
                mutate.0
            );
        }
    }

    #[test]
    fn rejects_cross_namespace_regime_label_aliases() {
        let mut manifest = minimal_manifest();
        manifest.regimes[0].id = "1".into();
        manifest.regimes[1].id = "0".into();
        let error = manifest.validate().unwrap_err();
        assert!(matches!(error, ManifestError::AmbiguousRegimeLabel(_)));
    }
}
