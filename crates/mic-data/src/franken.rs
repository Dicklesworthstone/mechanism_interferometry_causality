#![forbid(unsafe_code)]
//! `FrankenPandas` tabular adapter.
//!
//! The sibling library reads the file. It decides nothing about the experiment: the
//! adapter hands cell strings to [`build_ingest_report`], which is the same function the
//! standard-library reader uses, so both backends share every semantic rule.

use crate::{ExperimentManifest, IngestReport, TableError, build_ingest_report, resolve_data_path};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use frankenpandas::{CsvReadOptions, DType, Scalar, read_csv_with_options};

/// The reviewed sibling revision for audit provenance.
pub const REVISION: &str = "9599d6f4a12306897a9bc19be3d2ba2ac228a97c";

/// Returns the selected tabular backend name.
#[must_use]
pub const fn backend_name() -> &'static str {
    "FrankenPandas"
}

/// Loads the manifest's CSV through `FrankenPandas`.
///
/// Contractually identical to [`crate::load_csv_table`] — same report on success, same
/// error text on failure. Three properties make that hold rather than merely hope for it.
///
/// Structure and every malformed-input error come from
/// [`crate::tokenize_csv_with_std_rules`], which runs before the sibling is invoked. Both
/// readers therefore reject a duplicate header, a short row, an extra field, or an
/// unterminated quote with one message from one implementation. Letting each backend
/// report its own diagnostic was the earlier behavior and it is a subtler bug than it
/// looks: both sides still refuse, so nothing fails, and the disagreement only surfaces
/// when somebody is debugging real data against the wrong explanation.
///
/// Every column is forced to [`DType::Utf8`]. Left to infer, `FrankenPandas` types a
/// cluster column of `007, 008` as `Int64` and hands back `7, 8`, silently merging
/// `007` with `7` and — worse for this system — changing which clusters look distinct.
/// Cluster identity is the randomization unit, so a backend that normalizes identifiers
/// is not a faster reader, it is a different experiment. Numeric state columns are
/// parsed downstream by the shared code path using its own finiteness rules, so reading
/// everything as text loses nothing and removes an entire class of dtype-inference
/// divergence.
///
/// Aggregation is not reimplemented here. Cells go straight to
/// [`build_ingest_report`], so `clusters_spanning_regimes`, the quotas, and the fold
/// plan are computed by exactly one implementation.
pub fn load_csv_table_franken(
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

    // The standard tokenizer is the oracle for structure and for every malformed-input
    // error, so both backends refuse the same files with the same message. It also gives
    // the cell values the sibling's output is checked against.
    let (headers, expected_records) = crate::tokenize_csv_with_std_rules(&text)?;
    let mut dtype = HashMap::new();
    for name in &headers {
        dtype.insert(name.clone(), DType::Utf8);
    }
    let options = CsvReadOptions {
        dtype: Some(dtype),
        // A missing marker and the literal text "NA" must stay distinguishable: the
        // shared parser rejects an unparseable state cell, and silently turning it into
        // a null here would convert a hard parse failure into a quiet absence.
        na_filter: false,
        keep_default_na: false,
        ..CsvReadOptions::default()
    };
    let frame = read_csv_with_options(&text, &options).map_err(|error| TableError::Parse {
        row: 0,
        message: format!("frankenpandas could not read the csv: {error}"),
    })?;

    let mut records: Vec<Vec<String>> = Vec::new();
    let mut columns: Vec<Vec<String>> = Vec::with_capacity(headers.len());
    for name in &headers {
        columns.push(column_strings(&frame, name)?);
    }
    let row_count = columns.first().map_or(0, Vec::len);
    for column in &columns {
        if column.len() != row_count {
            return Err(TableError::Parse {
                row: 0,
                message: "frankenpandas returned ragged columns".into(),
            });
        }
    }
    for index in 0..row_count {
        records.push(columns.iter().map(|column| column[index].clone()).collect());
    }

    verify_cell_fidelity(&expected_records, &headers, &records)?;
    build_ingest_report(manifest, &path, content_sha256, &headers, &records, n_folds)
}

/// Refuses to proceed unless the backend returned the file's cells verbatim.
///
/// This is not defensive paranoia. The previously pinned revision (`a9f8d86c`) failed
/// it: asked for [`DType::Utf8`], `fp-io` parsed each cell numerically and re-rendered
/// it, so `007` came back `"7"`, `00` came back `"0"`, and `1e3` came back `"1000.0"`.
/// The dtype override selected the storage type, not value fidelity. Fixed upstream in
/// `9599d6f4a`, where a `dtype=str` column is taken from the verbatim source text.
///
/// The check stays because of what that class of bug costs here. `007` and `7` are
/// distinct assignment units, and collapsing them pools two randomization units into
/// one — the independence violation the randomization-unit rule forbids, occurring
/// silently, with no error and a plausible-looking report. It equally destroys design
/// bit strings, so `00` and `0` stop being distinguishable regimes. Worse, identifier
/// merging can both manufacture and mask a regime-spanning cluster, and that single
/// field gates the entire refusal chain.
///
/// So the adapter audits its own backend rather than trusting a version pin: cells are
/// compared against the standard tokenizer and any disagreement is a hard error naming
/// the row, column, and both values. A faithful backend passes for the cost of one
/// comparison. An unfaithful one — a future regression, or a different pin — is refused
/// with a diagnosable message instead of producing a wrong `IngestReport`.
fn verify_cell_fidelity(
    expected_records: &[Vec<String>],
    headers: &[String],
    records: &[Vec<String>],
) -> Result<(), TableError> {
    if expected_records.len() != records.len() {
        return Err(TableError::Parse {
            row: 0,
            message: format!(
                "frankenpandas returned {} data rows where the file has {}",
                records.len(),
                expected_records.len()
            ),
        });
    }
    for (index, (expected, actual)) in expected_records.iter().zip(records).enumerate() {
        // Compared before the cells: zipping two rows of different length silently stops
        // at the shorter one, and the sibling pads a short row with empty strings rather
        // than rejecting it, so a padded row would otherwise pass the cell comparison.
        if expected.len() != actual.len() {
            return Err(TableError::Parse {
                row: index + 1,
                message: format!(
                    "frankenpandas returned {} fields where the file row has {}; a padded or \
                     truncated row changes which value belongs to which column",
                    actual.len(),
                    expected.len()
                ),
            });
        }
        for (column, (want, got)) in expected.iter().zip(actual).enumerate() {
            if want != got {
                let name = headers.get(column).map_or("<unknown>", String::as_str);
                return Err(TableError::Parse {
                    row: index + 1,
                    message: format!(
                        "frankenpandas altered column {name:?}: file has {want:?} but the \
                         backend returned {got:?}; identifier and design-label text must \
                         survive ingest unchanged because cluster and regime identity define \
                         the randomization unit"
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Extracts one column as owned strings, refusing anything that is not present text.
fn column_strings(frame: &frankenpandas::DataFrame, name: &str) -> Result<Vec<String>, TableError> {
    let column = frame
        .columns()
        .get(name)
        .ok_or_else(|| TableError::MissingColumn(name.to_string()))?;
    // A CSV load materializes Utf8 in a lazy representation that the contiguous getters
    // do not expose, so read the scalars directly. Every cell must be a present
    // `Scalar::Utf8`: the dtype override asked for text, and anything else coming back
    // means the value was coerced or nulled, which would change cell identity relative
    // to the standard reader. Refuse rather than paper over it.
    let mut values = Vec::with_capacity(column.values().len());
    for (index, scalar) in column.values().iter().enumerate() {
        match scalar {
            Scalar::Utf8(value) => values.push(value.clone()),
            other => {
                return Err(TableError::Parse {
                    row: index + 1,
                    message: format!(
                        "column {name:?} yielded {other:?} where present utf8 text was requested; \
                         a coerced dtype or a null would change cell identity relative to the \
                         standard reader"
                    ),
                });
            }
        }
    }
    Ok(values)
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in &digest {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataSource, InferenceTrack, RegimeSpec, SelectionContract, load_csv_table};
    use mic_design::DesignPoint;
    use std::path::PathBuf;

    /// Regime *ids* are free-form labels; only the design is a bit string. Alphabetic ids
    /// are used here so the fixture is not destroyed by the backend's numeric
    /// re-rendering, which is exercised deliberately in its own test below.
    fn manifest_for(path: &Path) -> ExperimentManifest {
        ExperimentManifest {
            schema_version: "1.0.0".into(),
            experiment_id: "franken-differential".into(),
            strict: true,
            inference_track: InferenceTrack::FourLaw,
            selection: SelectionContract::StateIndependentWithinRegime,
            cluster_column: "cluster_id".into(),
            regime_column: "regime".into(),
            state_columns: vec!["x".into()],
            candidate_state_blocks: Vec::new(),
            regimes: [("base", "00"), ("a", "10"), ("b", "01"), ("ab", "11")]
                .iter()
                .map(|(id, design)| RegimeSpec {
                    id: (*id).into(),
                    design: DesignPoint::parse(design).unwrap(),
                    sampling_proportion: 0.25,
                    perturbations: Vec::new(),
                })
                .collect(),
            data: DataSource {
                format: "csv".into(),
                path: path.to_path_buf(),
            },
            seed: 20_260_812,
        }
    }

    fn write_fixture(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("mic-data-franken-differential");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, body).unwrap();
        path
    }

    /// The falsifier for this adapter, on input the backend reproduces faithfully.
    ///
    /// A regime-spanning cluster is recorded, not rejected, by either reader, and every
    /// downstream refusal is conditioned on `clusters_spanning_regimes` being non-empty.
    /// So the property that matters is not "each backend notices something" but "the two
    /// backends produce the same field". Whole-report equality asserts that and more.
    #[test]
    fn spanning_cluster_is_identical_across_backends() {
        let path = write_fixture(
            "spanning.csv",
            "cluster_id,regime,x,included\n\
             shared,base,0.5,1\n\
             shared,a,0.25,1\n\
             c1,b,0.75,1\n\
             c2,ab,0.125,1\n",
        );
        let manifest = manifest_for(&path);
        let std_report = load_csv_table(&manifest, None, 2).unwrap();
        let franken_report = load_csv_table_franken(&manifest, None, 2).unwrap();

        assert_eq!(
            std_report.clusters_spanning_regimes,
            vec!["shared".to_string()],
            "the standard reader must still record the spanning cluster"
        );
        assert_eq!(
            std_report.clusters_spanning_regimes, franken_report.clusters_spanning_regimes,
            "a backend that drops a spanning cluster silently disables the refusal chain"
        );
        assert_eq!(
            std_report, franken_report,
            "backends must agree on the whole report, folds and quotas included"
        );
    }

    /// Bit-string design labels must survive ingest byte-for-byte.
    ///
    /// This is the case that killed the previous pin: `00` came back as `0` even with
    /// `DType::Utf8` forced and the field quoted, which stopped two regimes being
    /// distinguishable. Fixed upstream in `frankenpandas` 9599d6f4a, where `dtype=str`
    /// now returns the source text instead of re-rendering an inferred scalar. Asserted
    /// here as whole-report equality so a regression is caught on the mic side too.
    #[test]
    fn bit_string_regime_labels_survive_ingest() {
        let path = write_fixture(
            "bitstring_regimes.csv",
            "cluster_id,regime,x,included\n\
             c0,00,0.5,1\n\
             c1,10,0.25,1\n",
        );
        let mut manifest = manifest_for(&path);
        for (spec, label) in manifest.regimes.iter_mut().zip(["00", "10", "01", "11"]) {
            spec.id = label.into();
        }
        let std_report = load_csv_table(&manifest, None, 2).unwrap();
        let franken_report = load_csv_table_franken(&manifest, None, 2).unwrap();

        let regimes: Vec<&str> = std_report
            .rows
            .iter()
            .map(|row| row.regime_id.as_str())
            .collect();
        assert_eq!(
            regimes,
            vec!["00", "10"],
            "leading-zero design labels must not be renumbered"
        );
        assert_eq!(
            std_report, franken_report,
            "both backends must resolve the same regimes from bit-string labels"
        );
    }

    /// Leading-zero cluster ids must never be merged into one randomization unit.
    ///
    /// `007` and `7` are distinct assignment units. The previous pin rendered both as
    /// `7`, which pooled two clusters and treated their rows as one — the independence
    /// violation the randomization-unit rule exists to prevent, occurring silently.
    /// Both backends must now see four clusters and agree on the fingerprint that binds
    /// the unit.
    #[test]
    fn leading_zero_cluster_ids_are_not_merged() {
        let path = write_fixture(
            "leading_zero.csv",
            "cluster_id,regime,x,included\n\
             007,base,0.5,1\n\
             7,a,0.25,1\n\
             c1,b,0.75,1\n\
             c2,ab,0.125,1\n",
        );
        let manifest = manifest_for(&path);
        let std_report = load_csv_table(&manifest, None, 2).unwrap();
        let franken_report = load_csv_table_franken(&manifest, None, 2).unwrap();

        assert_eq!(
            std_report.fingerprint.n_clusters, 4,
            "007 and 7 are distinct assignment units"
        );
        assert_eq!(
            franken_report.fingerprint.n_clusters, 4,
            "dtype inference must not merge distinct cluster identifiers"
        );
        assert_eq!(
            std_report.fingerprint.cluster_fingerprint,
            franken_report.fingerprint.cluster_fingerprint,
            "cluster fingerprints bind the randomization unit and must match exactly"
        );
        assert_eq!(std_report, franken_report);
    }

    /// Malformed input must be refused with the *same message* by both backends.
    ///
    /// Every case here diverged before the standard tokenizer was made the oracle: the
    /// two readers refused the same files for different stated reasons. Both refusing is
    /// not sufficient — a differing diagnostic means the backends disagree about what is
    /// wrong, and that is the kind of divergence that hides until someone debugs a real
    /// dataset at 2am.
    ///
    /// The short-row case is the important one. `FrankenPandas` pads a short row with
    /// empty strings rather than rejecting it, so without an explicit row-length check
    /// the padded row silently reaches the cell comparison, where zipping two rows of
    /// unequal length stops at the shorter and reports agreement.
    #[test]
    fn malformed_input_is_refused_identically_by_both_backends() {
        for (name, body) in [
            (
                "dup_headers.csv",
                "cluster_id,regime,x,x,included\nc0,base,0.5,0.6,1\n",
            ),
            (
                "short_row.csv",
                "cluster_id,regime,x,included\nc0,base,0.5\n",
            ),
            (
                "long_row.csv",
                "cluster_id,regime,x,included\nc0,base,0.5,1,9\n",
            ),
            (
                "unterminated_quote.csv",
                "cluster_id,regime,x,included\nc0,base,\"0.5,1\n",
            ),
            (
                "unparseable_state.csv",
                "cluster_id,regime,x,included\nc0,base,notanumber,1\n",
            ),
            (
                "empty_state.csv",
                "cluster_id,regime,x,included\nc0,base,,1\n",
            ),
        ] {
            let path = write_fixture(name, body);
            let manifest = manifest_for(&path);
            let std_error = load_csv_table(&manifest, None, 2)
                .expect_err(&format!("{name} must be refused by the standard reader"));
            let franken_error = load_csv_table_franken(&manifest, None, 2)
                .expect_err(&format!("{name} must be refused by the franken reader"));
            assert_eq!(
                std_error.to_string(),
                franken_error.to_string(),
                "{name}: both backends refuse, but for different stated reasons"
            );
        }
    }

    /// A BOM-prefixed file must load, and load identically, under both backends.
    ///
    /// Before the standard reader stripped the mark, the two disagreed on the failure:
    /// the standard path reported a missing `cluster_id` while the adapter reported a
    /// missing `\u{feff}cluster_id`, because the sibling drops the mark from its header
    /// and the standard tokenizer did not.
    #[test]
    fn byte_order_mark_agrees_across_backends() {
        let path = write_fixture(
            "bom.csv",
            "\u{feff}cluster_id,regime,x,included\nc0,base,0.5,1\nc1,a,0.25,1\n",
        );
        let manifest = manifest_for(&path);
        let std_report = load_csv_table(&manifest, None, 2).unwrap();
        let franken_report = load_csv_table_franken(&manifest, None, 2).unwrap();
        assert_eq!(std_report.fingerprint.n_rows, 2);
        assert_eq!(std_report, franken_report);
    }

    /// A CRLF file must load, and load identically, under both backends.
    #[test]
    fn crlf_line_endings_agree_across_backends() {
        let path = write_fixture(
            "crlf.csv",
            "cluster_id,regime,x,included\r\nc0,base,0.5,1\r\nc1,a,0.25,1\r\n",
        );
        let manifest = manifest_for(&path);
        let std_report = load_csv_table(&manifest, None, 2).unwrap();
        let franken_report = load_csv_table_franken(&manifest, None, 2).unwrap();
        assert_eq!(std_report.fingerprint.n_rows, 2);
        assert_eq!(std_report, franken_report);
    }

    /// Both readers must refuse the same malformed input, not merely succeed alike.
    #[test]
    fn duplicate_row_identifiers_fail_in_both_backends() {
        let path = write_fixture(
            "duplicate_rows.csv",
            "row_id,cluster_id,regime,x,included\n\
             r1,c0,base,0.5,1\n\
             r1,c1,a,0.25,1\n\
             r2,c2,b,0.75,1\n\
             r3,c3,ab,0.125,1\n",
        );
        let manifest = manifest_for(&path);
        let std_error = load_csv_table(&manifest, None, 2).unwrap_err();
        let franken_error = load_csv_table_franken(&manifest, None, 2).unwrap_err();
        assert!(std_error.to_string().contains("duplicate row identifier"));
        assert_eq!(
            std_error.to_string(),
            franken_error.to_string(),
            "the two backends must reject identically, message included"
        );
    }
}
