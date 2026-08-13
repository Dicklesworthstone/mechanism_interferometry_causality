#![forbid(unsafe_code)]
//! Command-line entry point for simulation, design, manifest, and preflight audits.

use mic_audit::{EvidenceLedger, ExecutionMode};
use mic_data::ExperimentManifest;
use mic_design::{DesignPoint, audit_design, audit_sampling_odds};
use mic_engine::{
    PreflightPolicy, audit_orientation, resolve_selection_evidence_from_files, run_preflight,
    run_preflight_with_selection_evidence,
};
use mic_sim::{
    causal_tomography_chain, exact_suite, flat_noncausal_cube, hidden_sensor_tomography,
    identification_twins, implementation_inconsistency, latent_conservation, parity_example,
    running_example,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};

const MAX_JSON_REQUEST_BYTES: u64 = 64 * 1024 * 1024;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if let Err(error) = run(&args) {
        eprintln!("mic: {error}");
        std::process::exit(2);
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    match command {
        "version" | "--version" | "-V" => {
            if args.len() != 1 {
                return Err("version accepts no arguments".into());
            }
            println!("mic {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "simulate" => simulate(&args[1..]),
        "design" => design(&args[1..]),
        "validate-manifest" => validate_manifest(&args[1..]),
        "preflight" => preflight(&args[1..]),
        "closure-crossfit" => closure_crossfit(&args[1..]),
        "predict-combination" => predict_combination(&args[1..]),
        "predict-combination-refits" => predict_combination_refits(&args[1..]),
        "finite-completion" => finite_completion(&args[1..]),
        "kernel-completion" => kernel_completion(&args[1..]),
        "orient" => orient(&args[1..]),
        "propose-tilt" => propose_tilt(&args[1..]),
        "freeze-scout" => freeze_scout(&args[1..]),
        "freeze-dictionary" => freeze_dictionary(&args[1..]),
        "help" | "--help" | "-h" => {
            if args.len() != 1 {
                return Err("help accepts no arguments".into());
            }
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn simulate(args: &[String]) -> Result<(), String> {
    let (scenario, output) = match args {
        [] => ("all", None),
        [scenario] => (scenario.as_str(), None),
        [flag, output] if flag == "--output" && !output.trim().is_empty() => {
            ("all", Some(PathBuf::from(output)))
        }
        [scenario, flag, output] if flag == "--output" && !output.trim().is_empty() => {
            (scenario.as_str(), Some(PathBuf::from(output)))
        }
        _ => return Err("usage: mic simulate [SCENARIO] [--output PATH]".into()),
    };
    let value = match scenario {
        "all" => serde_json::to_value(exact_suite()),
        "running" => serde_json::to_value(running_example(0.6, 0.5, 0.8)),
        "parity" => serde_json::to_value(parity_example(0.1)),
        "latent" => serde_json::to_value(latent_conservation(0.3)),
        "implementation" => serde_json::to_value(implementation_inconsistency(0.45, 0.35, 0.4)),
        "tomography" => serde_json::to_value(causal_tomography_chain()),
        "flat-noncausal" => serde_json::to_value(flat_noncausal_cube()),
        "hidden-sensor" => serde_json::to_value(hidden_sensor_tomography()),
        "identification-twins" => serde_json::to_value(identification_twins()),
        other => return Err(format!("unknown simulation {other:?}")),
    }
    .map_err(|error| error.to_string())?;
    write_json_value(&value, output.as_deref())
}

fn design(args: &[String]) -> Result<(), String> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err("design requires odds or audit".into());
    };
    match subcommand {
        "odds" => {
            if args.len() != 5 {
                return Err("usage: mic design odds p00 p10 p01 p11".into());
            }
            let mut probabilities = [0.0; 4];
            for (slot, raw) in probabilities.iter_mut().zip(&args[1..]) {
                *slot = raw
                    .parse::<f64>()
                    .map_err(|_| format!("invalid probability {raw:?}"))?;
            }
            let audit =
                audit_sampling_odds(probabilities, 1e-10).map_err(|error| error.to_string())?;
            print_json(&audit)
        }
        "audit" => {
            let labels = &args[1..];
            if labels.is_empty() {
                return Err("usage: mic design audit CORNER... | MANIFEST.json".into());
            }
            let points = if labels.len() == 1 && looks_like_json_path(&labels[0]) {
                read_manifest_bounded(&labels[0])?
                    .regimes
                    .into_iter()
                    .map(|regime| regime.design)
                    .collect()
            } else {
                labels
                    .iter()
                    .map(|label| DesignPoint::parse(label))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?
            };
            let audit = audit_design(&points, 1e-10).map_err(|error| error.to_string())?;
            print_json(&audit)
        }
        other => Err(format!("unknown design subcommand {other:?}")),
    }
}

fn validate_manifest(args: &[String]) -> Result<(), String> {
    let [path] = args else {
        return Err("usage: mic validate-manifest PATH".into());
    };
    let manifest = read_manifest_bounded(path)?;
    println!(
        "validated experiment {} with {} regimes and {} state columns",
        manifest.experiment_id,
        manifest.regimes.len(),
        manifest.state_columns.len()
    );
    Ok(())
}

fn preflight(args: &[String]) -> Result<(), String> {
    let parsed = parse_preflight_args(args)?;
    let path = parsed.path;
    let manifest = read_manifest_bounded(path)?;
    let policy = PreflightPolicy {
        accept_unvalidated_selection_model: parsed.allow_unvalidated_selection_model,
        ..PreflightPolicy::default()
    };
    let receipt = parsed.selection_receipt;
    let authority = parsed.selection_authority_source;
    let report = match (receipt, authority) {
        (None, None) => run_preflight(&manifest, policy),
        (Some(receipt), Some(authority)) => {
            let explicit_base = parsed.base_dir.map(PathBuf::from);
            let base_dir = explicit_base
                .as_deref()
                .or_else(|| Path::new(path).parent());
            let evidence =
                resolve_selection_evidence_from_files(&manifest, receipt, authority, base_dir)
                    .map_err(|error| error.to_string())?;
            run_preflight_with_selection_evidence(&manifest, policy, &evidence)
        }
        _ => {
            return Err(
                "--selection-receipt and --selection-authority-source must be supplied together"
                    .into(),
            );
        }
    }
    .map_err(|error| error.to_string())?;
    let value = serde_json::to_value(report).map_err(|error| error.to_string())?;
    let output = parsed.output.map(PathBuf::from);
    write_json_value(&value, output.as_deref())
}

struct ParsedPreflightArgs<'a> {
    path: &'a str,
    output: Option<&'a str>,
    base_dir: Option<&'a str>,
    allow_unvalidated_selection_model: bool,
    selection_receipt: Option<&'a str>,
    selection_authority_source: Option<&'a str>,
}

fn parse_preflight_args(args: &[String]) -> Result<ParsedPreflightArgs<'_>, String> {
    let Some(path) = args.first().map(String::as_str) else {
        return Err("usage: mic preflight MANIFEST.json [OPTIONS]".into());
    };
    let mut parsed = ParsedPreflightArgs {
        path,
        output: None,
        base_dir: None,
        allow_unvalidated_selection_model: false,
        selection_receipt: None,
        selection_authority_source: None,
    };
    let mut index = 1;
    while index < args.len() {
        let flag = args[index].as_str();
        if flag == "--allow-unvalidated-selection-model" {
            if parsed.allow_unvalidated_selection_model {
                return Err(format!("duplicate option {flag}"));
            }
            parsed.allow_unvalidated_selection_model = true;
            index += 1;
            continue;
        }
        let slot = match flag {
            "--output" => &mut parsed.output,
            "--base-dir" => &mut parsed.base_dir,
            "--selection-receipt" => &mut parsed.selection_receipt,
            "--selection-authority-source" => &mut parsed.selection_authority_source,
            _ => return Err(format!("unknown preflight option {flag:?}")),
        };
        if slot.is_some() {
            return Err(format!("duplicate option {flag}"));
        }
        let value = args
            .get(index + 1)
            .filter(|value| !value.trim().is_empty() && !value.starts_with("--"))
            .ok_or_else(|| format!("{flag} requires a nonempty value"))?;
        *slot = Some(value);
        index += 2;
    }
    if parsed.allow_unvalidated_selection_model
        && (parsed.selection_receipt.is_some() || parsed.selection_authority_source.is_some())
    {
        return Err("--allow-unvalidated-selection-model cannot be combined with selection provenance files".into());
    }
    Ok(parsed)
}

/// Input contract for the diagnostic reference closure model.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClosureCrossFitInput {
    schema_version: String,
    declared_dependence_unit: String,
    declared_assignment_episode: String,
    sampling_proportions: [f64; 4],
    config: mic_model::ClosureCrossFitConfig,
    samples: Vec<mic_model::ClusteredMultinomialSample>,
}

fn closure_crossfit(args: &[String]) -> Result<(), String> {
    let (path, output) = match args {
        [path] => (path, None),
        [path, flag, output] if flag == "--output" && !output.trim().is_empty() => {
            (path, Some(PathBuf::from(output)))
        }
        _ => {
            return Err("usage: mic closure-crossfit INPUT.json [--output PATH]".into());
        }
    };
    let bytes = read_bounded_request(path, MAX_JSON_REQUEST_BYTES, "closure cross-fit request")?;
    let input_sha256 = sha256_bytes(&bytes);
    let input: ClosureCrossFitInput =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if input.schema_version != "1.1.0" {
        return Err("closure-crossfit schema_version must be 1.1.0".into());
    }
    if input.declared_dependence_unit.trim().is_empty()
        || input.declared_assignment_episode.trim().is_empty()
    {
        return Err(
            "declared_dependence_unit and declared_assignment_episode must not be empty".into(),
        );
    }
    let diagnostic = mic_model::cross_fit_closure_models(
        &input.samples,
        input.sampling_proportions,
        input.config,
    )
    .map_err(|error| error.to_string())?;
    let mut ledger = EvidenceLedger::new(ExecutionMode::Exploratory);
    ledger.provenance("closure_crossfit_input_sha256", &input_sha256);
    ledger.provenance("closure_crossfit_seed", diagnostic.seed().to_string());
    ledger.provenance(
        "closure_crossfit_fold_plan_sha256",
        diagnostic.fold_plan_sha256(),
    );
    ledger.provenance("declared_dependence_unit", &input.declared_dependence_unit);
    ledger.provenance(
        "declared_assignment_episode",
        &input.declared_assignment_episode,
    );
    let value = serde_json::json!({
        "schema_version": "1.1.0",
        "authority": "diagnostic_only",
        "certificate_eligible": false,
        "input_sha256": input_sha256,
        "declared_dependence_unit": input.declared_dependence_unit,
        "declared_assignment_episode": input.declared_assignment_episode,
        "diagnostic": diagnostic,
        "ledger": ledger,
    });
    write_json_value(&value, output.as_deref())
}

/// Stage-A request. It cannot represent the combination arm.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrimitiveTransportRequest {
    schema_version: String,
    declared_independent_unit: String,
    primitive_sampling_proportions: [f64; 3],
    feature_contract: mic_model::FrozenFeatureContract,
    config: mic_model::PrimitiveTransportConfig,
    samples: Vec<mic_model::PrimitiveTransportSample>,
}

/// Stage-B request, opened only after the primitive transport is frozen.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CombinationConfirmationRequest {
    schema_version: String,
    declared_independent_unit: String,
    feature_contract: mic_model::FrozenFeatureContract,
    samples: Vec<mic_model::CombinationConfirmationSample>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransportRefitRequest {
    schema_version: String,
    seed: u64,
    n_refits: usize,
    retain_fraction: f64,
}

fn predict_combination(args: &[String]) -> Result<(), String> {
    let (primitive_path, confirmation_path, output) = match args {
        [primitive, confirmation] => (primitive, confirmation, None),
        [primitive, confirmation, flag, output]
            if flag == "--output" && !output.trim().is_empty() =>
        {
            (primitive, confirmation, Some(PathBuf::from(output)))
        }
        _ => {
            return Err(
                "usage: mic predict-combination PRIMITIVES.json CONFIRMATION.json [--output PATH]"
                    .into(),
            );
        }
    };

    let primitive_bytes = read_bounded_request(
        primitive_path,
        MAX_JSON_REQUEST_BYTES,
        "primitive transport",
    )?;
    let primitive_request_sha256 = sha256_bytes(&primitive_bytes);
    let primitive: PrimitiveTransportRequest =
        serde_json::from_slice(&primitive_bytes).map_err(|error| error.to_string())?;
    validate_transport_schema_version(&primitive.schema_version)?;
    if primitive.declared_independent_unit.trim().is_empty() {
        return Err("declared_independent_unit must not be empty".into());
    }
    let frozen = mic_model::freeze_primitive_transport(
        &primitive.samples,
        primitive.primitive_sampling_proportions,
        &primitive.declared_independent_unit,
        primitive.feature_contract.clone(),
        primitive.config,
    )
    .map_err(|error| error.to_string())?;

    // Deliberately open Stage B only after Stage A has been fully frozen.
    let confirmation_bytes = read_bounded_request(
        confirmation_path,
        MAX_JSON_REQUEST_BYTES,
        "combination confirmation",
    )?;
    let confirmation_request_sha256 = sha256_bytes(&confirmation_bytes);
    let confirmation: CombinationConfirmationRequest =
        serde_json::from_slice(&confirmation_bytes).map_err(|error| error.to_string())?;
    validate_transport_schema_version(&confirmation.schema_version)?;
    if primitive.declared_independent_unit != confirmation.declared_independent_unit {
        return Err("primitive and confirmation independent-unit declarations differ".into());
    }
    if primitive.feature_contract != confirmation.feature_contract {
        return Err("primitive and confirmation feature contracts differ".into());
    }
    let report = mic_model::score_combination_confirmation(
        &frozen,
        &confirmation.declared_independent_unit,
        &confirmation.feature_contract,
        &confirmation.samples,
    )
    .map_err(|error| error.to_string())?;

    let mut ledger = EvidenceLedger::new(ExecutionMode::Exploratory);
    ledger.provenance(
        "primitive_transport_request_sha256",
        &primitive_request_sha256,
    );
    ledger.provenance(
        "combination_confirmation_request_sha256",
        &confirmation_request_sha256,
    );
    ledger.provenance(
        "primitive_transport_input_sha256",
        report.primitive_receipt().primitive_input_sha256(),
    );
    ledger.provenance(
        "primitive_transport_fold_plan_sha256",
        report.primitive_receipt().fold_plan_sha256(),
    );
    ledger.provenance(
        "primitive_transport_seed",
        report.primitive_receipt().seed().to_string(),
    );
    ledger.provenance(
        "declared_independent_unit",
        &primitive.declared_independent_unit,
    );
    let value = serde_json::json!({
        "schema_version": "1.0.0",
        "authority": "diagnostic_only",
        "certificate_eligible": false,
        "product_design_required": false,
        "stage_order": "primitive_frozen_before_confirmation_opened",
        "primitive_request_sha256": primitive_request_sha256,
        "confirmation_request_sha256": confirmation_request_sha256,
        "declared_independent_unit": primitive.declared_independent_unit,
        "diagnostic": report,
        "ledger": ledger,
    });
    write_json_value(&value, output.as_deref())
}

fn validate_transport_schema_version(version: &str) -> Result<(), String> {
    if version == "1.0.0" {
        Ok(())
    } else {
        Err("fitted transport schema_version must be 1.0.0".into())
    }
}

fn read_bounded_request(
    path: impl AsRef<Path>,
    limit: u64,
    label: &str,
) -> Result<Vec<u8>, String> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > limit {
        return Err(format!(
            "{label} request exceeds the fixed {limit}-byte input limit"
        ));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| format!("{label} request length is not representable on this platform"))?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut reader = File::open(path)
        .map_err(|error| error.to_string())?
        .take(limit.saturating_add(1));
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > limit) {
        return Err(format!(
            "{label} request exceeds the fixed {limit}-byte input limit"
        ));
    }
    Ok(bytes)
}

#[allow(clippy::too_many_lines)]
fn predict_combination_refits(args: &[String]) -> Result<(), String> {
    let (primitive_path, confirmation_path, refit_path, output) = match args {
        [primitive, confirmation, refit] => (primitive, confirmation, refit, None),
        [primitive, confirmation, refit, flag, output]
            if flag == "--output" && !output.trim().is_empty() =>
        {
            (primitive, confirmation, refit, Some(PathBuf::from(output)))
        }
        _ => {
            return Err("usage: mic predict-combination-refits PRIMITIVES.json CONFIRMATION.json REFITS.json [--output PATH]".into());
        }
    };
    let refit_bytes = read_bounded_request(refit_path, MAX_JSON_REQUEST_BYTES, "transport refit")?;
    let refit_request_sha256 = sha256_bytes(&refit_bytes);
    let refit: TransportRefitRequest =
        serde_json::from_slice(&refit_bytes).map_err(|error| error.to_string())?;
    validate_transport_schema_version(&refit.schema_version)?;

    let primitive_bytes = read_bounded_request(
        primitive_path,
        MAX_JSON_REQUEST_BYTES,
        "primitive transport",
    )?;
    let primitive_request_sha256 = sha256_bytes(&primitive_bytes);
    let primitive: PrimitiveTransportRequest =
        serde_json::from_slice(&primitive_bytes).map_err(|error| error.to_string())?;
    validate_transport_schema_version(&primitive.schema_version)?;
    if primitive.declared_independent_unit.trim().is_empty() {
        return Err("declared_independent_unit must not be empty".into());
    }
    let refit_config = mic_model::TransportRefitConfig {
        seed: refit.seed,
        n_refits: refit.n_refits,
        retain_fraction: refit.retain_fraction,
    };
    mic_model::validate_primitive_refit_request(&primitive.samples, primitive.config, refit_config)
        .map_err(|error| error.to_string())?;
    // Establish the API-observable Stage-A freeze before opening `11`.
    let _stage_a_guard = mic_model::freeze_primitive_transport(
        &primitive.samples,
        primitive.primitive_sampling_proportions,
        &primitive.declared_independent_unit,
        primitive.feature_contract.clone(),
        primitive.config,
    )
    .map_err(|error| error.to_string())?;

    let confirmation_bytes = read_bounded_request(
        confirmation_path,
        MAX_JSON_REQUEST_BYTES,
        "combination confirmation",
    )?;
    let confirmation_request_sha256 = sha256_bytes(&confirmation_bytes);
    let confirmation: CombinationConfirmationRequest =
        serde_json::from_slice(&confirmation_bytes).map_err(|error| error.to_string())?;
    validate_transport_schema_version(&confirmation.schema_version)?;
    if primitive.declared_independent_unit != confirmation.declared_independent_unit {
        return Err("primitive and confirmation independent-unit declarations differ".into());
    }
    if primitive.feature_contract != confirmation.feature_contract {
        return Err("primitive and confirmation feature contracts differ".into());
    }
    let report = mic_model::refit_transport_uncertainty(
        &primitive.samples,
        &confirmation.samples,
        primitive.primitive_sampling_proportions,
        &primitive.declared_independent_unit,
        &primitive.feature_contract,
        primitive.config,
        refit_config,
    )
    .map_err(|error| error.to_string())?;
    let mut ledger = EvidenceLedger::new(ExecutionMode::Exploratory);
    for (key, value) in [
        (
            "primitive_transport_request_sha256",
            primitive_request_sha256.as_str(),
        ),
        (
            "combination_confirmation_request_sha256",
            confirmation_request_sha256.as_str(),
        ),
        (
            "transport_refit_request_sha256",
            refit_request_sha256.as_str(),
        ),
        ("transport_refit_plan_sha256", report.resample_plan_sha256()),
    ] {
        ledger.provenance(key, value);
    }
    ledger.provenance(
        "primitive_transport_seed",
        primitive.config.seed.to_string(),
    );
    ledger.provenance("transport_refit_seed", refit.seed.to_string());
    ledger.provenance(
        "declared_independent_unit",
        &primitive.declared_independent_unit,
    );
    let value = serde_json::json!({
        "schema_version": "1.0.0",
        "authority": "diagnostic_only",
        "certificate_eligible": false,
        "calibrated_test": false,
        "stage_order": "primitive_validated_before_confirmation_opened",
        "diagnostic": report,
        "ledger": ledger,
    });
    write_json_value(&value, output.as_deref())
}

/// Input contract for the finite-state fixed-model completion diagnostic.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FiniteCompletionRequest {
    schema_version: String,
    input: mic_model::FiniteCompletionInput,
}

fn finite_completion(args: &[String]) -> Result<(), String> {
    completion_command(args, false)
}

fn kernel_completion(args: &[String]) -> Result<(), String> {
    completion_command(args, true)
}

fn completion_command(args: &[String], use_kernel_solver: bool) -> Result<(), String> {
    let (path, output) = match args {
        [path] => (path, None),
        [path, flag, output] if flag == "--output" && !output.trim().is_empty() => {
            (path, Some(PathBuf::from(output)))
        }
        _ => {
            let command = if use_kernel_solver {
                "kernel-completion"
            } else {
                "finite-completion"
            };
            return Err(format!("usage: mic {command} INPUT.json [--output PATH]"));
        }
    };
    let bytes = read_bounded_request(path, MAX_JSON_REQUEST_BYTES, "completion")?;
    let input_sha256 = sha256_bytes(&bytes);
    let request: FiniteCompletionRequest =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if request.schema_version != "1.0.0" {
        return Err("finite-completion schema_version must be 1.0.0".into());
    }
    let report = if use_kernel_solver {
        serde_json::to_value(
            mic_model::solve_finite_kernel_completion(&request.input)
                .map_err(|error| error.to_string())?,
        )
    } else {
        serde_json::to_value(
            mic_model::solve_finite_modular_completion(&request.input)
                .map_err(|error| error.to_string())?,
        )
    }
    .map_err(|error| error.to_string())?;
    let value = serde_json::json!({
        "schema_version": "1.0.0",
        "authority": "diagnostic_only",
        "certificate_eligible": false,
        "scope": "fixed_finite_state_dag_and_distinct_declared_targets",
        "solver": if use_kernel_solver { "conditional_kernel" } else { "treatment_coded_log_potential" },
        "input_sha256": input_sha256,
        "report": report,
    });
    write_json_value(&value, output.as_deref())
}

/// One raw deletion bound row supplied by the orientation input file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrientDeletionInput {
    variable: String,
    relative_discrepancy: f64,
    lower: f64,
    upper: f64,
}

/// Provenance required for externally supplied simultaneous orientation bounds.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrientCalibrationInput {
    /// Stable name of the joint interval construction.
    interval_method: String,
    /// Must be true: pointwise intervals cannot certify a pass count.
    simultaneous: bool,
    /// Declared simultaneous coverage level.
    confidence: f64,
    /// Unit at which randomization and uncertainty were computed.
    randomization_unit: String,
    /// Content fingerprint of the data or exact source behind the supplied bounds.
    source_fingerprint: String,
    /// Deterministic seed used by the interval procedure or its upstream fit.
    seed: u64,
}

/// Input contract for `mic orient`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrientInput {
    epsilon: f64,
    full_discrepancy: f64,
    min_full_discrepancy: f64,
    #[serde(default = "default_strict")]
    strict: bool,
    calibration: OrientCalibrationInput,
    deletions: Vec<OrientDeletionInput>,
}

fn default_strict() -> bool {
    true
}

fn orient(args: &[String]) -> Result<(), String> {
    let (path, output) = match args {
        [path] => (path, None),
        [path, flag, output] if flag == "--output" && !output.trim().is_empty() => {
            (path, Some(PathBuf::from(output)))
        }
        _ => return Err("usage: mic orient INPUT.json [--output PATH]".into()),
    };
    let bytes = read_bounded_request(path, MAX_JSON_REQUEST_BYTES, "orientation request")?;
    let input_fingerprint = sha256_bytes(&bytes);
    let input: OrientInput = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    validate_orientation_calibration(&input.calibration)?;
    let deletions = input
        .deletions
        .iter()
        .map(|deletion| {
            mic_stats::classify_deletion(
                deletion.variable.clone(),
                deletion.relative_discrepancy,
                deletion.lower,
                deletion.upper,
                input.epsilon,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mode = if input.strict {
        ExecutionMode::Strict
    } else {
        ExecutionMode::Exploratory
    };
    let mut ledger = EvidenceLedger::new(mode);
    ledger.provenance("input_path", path.as_str());
    ledger.provenance("epsilon", format!("{:.6}", input.epsilon));
    ledger.provenance("interval_method", &input.calibration.interval_method);
    ledger.provenance(
        "simultaneous_confidence",
        format!("{:.6}", input.calibration.confidence),
    );
    ledger.provenance("randomization_unit", &input.calibration.randomization_unit);
    ledger.provenance(
        "orientation_source_fingerprint",
        &input.calibration.source_fingerprint,
    );
    ledger.provenance("orientation_input_sha256", input_fingerprint);
    ledger.provenance("orientation_seed", input.calibration.seed.to_string());
    let audit = audit_orientation(
        &deletions,
        input.full_discrepancy,
        input.min_full_discrepancy,
        "orientation",
        &mut ledger,
    )
    .map_err(|error| error.to_string())?;
    let value = serde_json::json!({
        "schema_version": "1.1.0",
        "authority": "diagnostic_only",
        "certificate_eligible": false,
        "causal_orientation": "unresolved",
        "required_premises": ["single_target_semantics", "deletion_faithfulness"],
        "audit": audit,
        "ledger": ledger
    });
    write_json_value(&value, output.as_deref())
}

fn validate_orientation_calibration(calibration: &OrientCalibrationInput) -> Result<(), String> {
    if !calibration.simultaneous {
        return Err(
            "orientation bounds must be simultaneous over the declared deletion family".into(),
        );
    }
    if !calibration.confidence.is_finite()
        || calibration.confidence <= 0.0
        || calibration.confidence >= 1.0
    {
        return Err("orientation calibration confidence must lie in (0, 1)".into());
    }
    for (name, value) in [
        ("interval_method", calibration.interval_method.as_str()),
        (
            "randomization_unit",
            calibration.randomization_unit.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            return Err(format!("orientation calibration {name} must not be empty"));
        }
    }
    let Some(hex) = calibration.source_fingerprint.strip_prefix("sha256:") else {
        return Err(
            "orientation source_fingerprint must use the sha256:<lowercase-hex> form".into(),
        );
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "orientation source_fingerprint must use the sha256:<lowercase-hex> form".into(),
        );
    }
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing hex into a String cannot fail");
    }
    encoded
}

/// Input contract for `mic propose-tilt`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposeTiltInput {
    request: mic_proposal::ActiveTiltRequest,
    candidates: Vec<mic_proposal::ActiveTiltCandidate>,
}

fn propose_tilt(args: &[String]) -> Result<(), String> {
    let (path, output) = match args {
        [path] => (path, None),
        [path, flag, output] if flag == "--output" && !output.trim().is_empty() => {
            (path, Some(PathBuf::from(output)))
        }
        _ => return Err("usage: mic propose-tilt INPUT.json [--output PATH]".into()),
    };
    let bytes = read_bounded_request(path, MAX_JSON_REQUEST_BYTES, "tilt proposal request")?;
    let input: ProposeTiltInput =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let proposal = mic_proposal::rank_active_tilts(&input.request, &input.candidates)
        .map_err(|error| error.to_string())?;
    let value = serde_json::to_value(proposal).map_err(|error| error.to_string())?;
    write_json_value(&value, output.as_deref())
}

fn freeze_scout(args: &[String]) -> Result<(), String> {
    let output = match args {
        [_, _] => None,
        [_, _, flag, path] if flag == "--output" && !path.trim().is_empty() => {
            Some(PathBuf::from(path))
        }
        _ => {
            return Err("usage: mic freeze-scout REQUEST.json DRAFT.json [--output PATH]".into());
        }
    };
    let request_path = args
        .first()
        .ok_or("usage: mic freeze-scout REQUEST.json DRAFT.json [--output PATH]")?;
    let draft_path = args
        .get(1)
        .ok_or("usage: mic freeze-scout REQUEST.json DRAFT.json [--output PATH]")?;
    let request_bytes =
        read_bounded_request(request_path, MAX_JSON_REQUEST_BYTES, "self-driving request")?;
    let draft_bytes = read_bounded_request(
        draft_path,
        MAX_JSON_REQUEST_BYTES,
        "shift-factorization draft",
    )?;
    let request: mic_proposal::SelfDrivingRequest =
        serde_json::from_slice(&request_bytes).map_err(|error| error.to_string())?;
    let draft: mic_proposal::ShiftFactorizationDraft =
        serde_json::from_slice(&draft_bytes).map_err(|error| error.to_string())?;
    let proposal = mic_proposal::freeze_shift_factorization_proposal(&request, &draft)
        .map_err(|error| error.to_string())?;
    let value = serde_json::to_value(proposal).map_err(|error| error.to_string())?;
    write_json_value(&value, output.as_deref())
}

fn freeze_dictionary(args: &[String]) -> Result<(), String> {
    let (request_path, shift_path, plan_path, dictionary_path, output) = match args {
        [request, shift, plan, dictionary] => (request, shift, plan, dictionary, None),
        [request, shift, plan, dictionary, flag, output]
            if flag == "--output" && !output.trim().is_empty() =>
        {
            (
                request,
                shift,
                plan,
                dictionary,
                Some(PathBuf::from(output)),
            )
        }
        _ => {
            return Err("usage: mic freeze-dictionary REQUEST.json SHIFT_DRAFT.json PLAN.json DICTIONARY_DRAFT.json [--output PATH]".into());
        }
    };
    let request_bytes =
        read_bounded_request(request_path, MAX_JSON_REQUEST_BYTES, "self-driving request")?;
    let shift_bytes = read_bounded_request(
        shift_path,
        MAX_JSON_REQUEST_BYTES,
        "shift-factorization draft",
    )?;
    let plan_bytes =
        read_bounded_request(plan_path, MAX_JSON_REQUEST_BYTES, "dictionary search plan")?;
    let dictionary_bytes = read_bounded_request(
        dictionary_path,
        MAX_JSON_REQUEST_BYTES,
        "transport-dictionary draft",
    )?;
    let request: mic_proposal::SelfDrivingRequest =
        serde_json::from_slice(&request_bytes).map_err(|error| error.to_string())?;
    let shift: mic_proposal::ShiftFactorizationDraft =
        serde_json::from_slice(&shift_bytes).map_err(|error| error.to_string())?;
    let plan: mic_proposal::DictionarySearchPlan =
        serde_json::from_slice(&plan_bytes).map_err(|error| error.to_string())?;
    let dictionary: mic_proposal::TransportDictionaryDraft =
        serde_json::from_slice(&dictionary_bytes).map_err(|error| error.to_string())?;
    let proposal =
        mic_proposal::freeze_transport_dictionary_proposal(&request, &shift, &plan, &dictionary)
            .map_err(|error| error.to_string())?;
    let value = serde_json::to_value(proposal).map_err(|error| error.to_string())?;
    write_json_value(&value, output.as_deref())
}

fn read_manifest_bounded(path: impl AsRef<Path>) -> Result<ExperimentManifest, String> {
    let bytes = read_bounded_request(path, MAX_JSON_REQUEST_BYTES, "experiment manifest")?;
    let manifest: ExperimentManifest =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    manifest.validate().map_err(|error| error.to_string())?;
    Ok(manifest)
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn write_json_value(value: &serde_json::Value, output: Option<&Path>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())? + "\n";
    if let Some(path) = output {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, text).map_err(|error| error.to_string())?;
    } else {
        print!("{text}");
    }
    Ok(())
}

fn looks_like_json_path(value: &str) -> bool {
    Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

fn print_help() {
    println!(
        "Mechanism Interferometry CLI\n\n\
         Usage:\n\
           mic simulate [all|running|parity|latent|implementation|tomography|flat-noncausal|hidden-sensor|identification-twins] [--output PATH]\n\
           mic design odds P00 P10 P01 P11\n\
           mic design audit CORNER...\n\
           mic design audit MANIFEST.json\n\
           mic validate-manifest MANIFEST.json\n\
           mic preflight MANIFEST.json [--output PATH]\n\
           mic closure-crossfit INPUT.json [--output PATH]\n\
           mic predict-combination PRIMITIVES.json CONFIRMATION.json [--output PATH]\n\
           mic predict-combination-refits PRIMITIVES.json CONFIRMATION.json REFITS.json [--output PATH]\n\
           mic finite-completion INPUT.json [--output PATH]\n\
           mic kernel-completion INPUT.json [--output PATH]\n\
           mic orient INPUT.json [--output PATH]\n\
           mic propose-tilt INPUT.json [--output PATH]\n\
           mic freeze-scout REQUEST.json DRAFT.json [--output PATH]\n\
           mic freeze-dictionary REQUEST.json SHIFT_DRAFT.json PLAN.json DICTIONARY_DRAFT.json [--output PATH]\n\
           mic version"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_test_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("mic-{label}-{}-{nonce}.json", std::process::id()))
    }

    fn calibration() -> OrientCalibrationInput {
        OrientCalibrationInput {
            interval_method: "cluster_multiplier".into(),
            simultaneous: true,
            confidence: 0.95,
            randomization_unit: "request".into(),
            source_fingerprint: format!("sha256:{}", "a".repeat(64)),
            seed: 17,
        }
    }

    #[test]
    fn orientation_calibration_requires_simultaneous_bounds() {
        let mut input = calibration();
        input.simultaneous = false;
        assert!(validate_orientation_calibration(&input).is_err());
    }

    #[test]
    fn orientation_calibration_requires_a_sha256_source() {
        let mut input = calibration();
        input.source_fingerprint = "not-a-hash".into();
        assert!(validate_orientation_calibration(&input).is_err());
        assert!(validate_orientation_calibration(&calibration()).is_ok());
    }

    #[test]
    fn freeze_scout_rejects_unknown_or_incomplete_output_options_before_io() {
        let unknown = vec!["request.json", "draft.json", "--bogus", "out.json"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert!(freeze_scout(&unknown).is_err());

        let missing = vec!["request.json", "draft.json", "--output"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert!(freeze_scout(&missing).is_err());
    }

    #[test]
    fn freeze_dictionary_rejects_unknown_or_incomplete_options_before_io() {
        let unknown = vec![
            "request.json",
            "shift.json",
            "plan.json",
            "dictionary.json",
            "--bogus",
            "out.json",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        assert!(freeze_dictionary(&unknown).is_err());

        let missing = vec![
            "request.json",
            "shift.json",
            "plan.json",
            "dictionary.json",
            "--output",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        assert!(freeze_dictionary(&missing).is_err());
    }

    #[test]
    fn dictionary_examples_are_closed_deserializable_inputs() {
        let plan: mic_proposal::DictionarySearchPlan = serde_json::from_str(include_str!(
            "../../../examples/dictionary_inputs/search_plan.json"
        ))
        .expect("dictionary search plan example");
        let draft: mic_proposal::TransportDictionaryDraft = serde_json::from_str(include_str!(
            "../../../examples/dictionary_inputs/transport_dictionary_draft.json"
        ))
        .expect("transport dictionary draft example");
        assert_eq!(plan.schema_version, "1.0.0");
        assert_eq!(draft.attempts.len(), 2);
    }

    #[test]
    fn closure_crossfit_example_is_closed_and_unknown_options_fail_before_io() {
        let input: ClosureCrossFitInput = serde_json::from_str(include_str!(
            "../../../examples/closure_crossfit_request.json"
        ))
        .unwrap();
        assert_eq!(input.schema_version, "1.1.0");
        assert_eq!(input.samples.len(), 8);

        let unknown = vec!["request.json".into(), "--bogus".into(), "out.json".into()];
        assert_eq!(
            closure_crossfit(&unknown).unwrap_err(),
            "usage: mic closure-crossfit INPUT.json [--output PATH]"
        );
        let missing_output = vec!["request.json".into(), "--output".into()];
        assert_eq!(
            closure_crossfit(&missing_output).unwrap_err(),
            "usage: mic closure-crossfit INPUT.json [--output PATH]"
        );
    }

    #[test]
    fn manifest_runtime_rejects_unknown_fields_at_every_closed_layer() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/configs/feature_flag_pilot.json"
        ))
        .unwrap();
        let mut root = source.clone();
        root.as_object_mut()
            .unwrap()
            .insert("future_authority".into(), serde_json::json!(true));
        assert!(serde_json::from_value::<ExperimentManifest>(root).is_err());

        let mut data = source.clone();
        data["data"]["future_path_semantics"] = serde_json::json!("trusted");
        assert!(serde_json::from_value::<ExperimentManifest>(data).is_err());

        let mut regime = source;
        regime["regimes"][0]["future_assignment_claim"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ExperimentManifest>(regime).is_err());
    }

    #[test]
    fn fitted_transport_examples_are_closed_and_options_fail_before_io() {
        let primitive: PrimitiveTransportRequest = serde_json::from_str(include_str!(
            "../../../examples/primitive_transport_request.json"
        ))
        .unwrap();
        let confirmation: CombinationConfirmationRequest = serde_json::from_str(include_str!(
            "../../../examples/combination_confirmation_request.json"
        ))
        .unwrap();
        assert_eq!(primitive.schema_version, "1.0.0");
        assert_eq!(primitive.samples.len(), 6);
        assert_eq!(confirmation.samples.len(), 2);
        let refit: TransportRefitRequest = serde_json::from_str(include_str!(
            "../../../examples/transport_refit_request.json"
        ))
        .unwrap();
        assert_eq!(refit.n_refits, 20);

        let unknown = vec![
            "primitive.json".into(),
            "confirmation.json".into(),
            "--bogus".into(),
        ];
        assert_eq!(
            predict_combination(&unknown).unwrap_err(),
            "usage: mic predict-combination PRIMITIVES.json CONFIRMATION.json [--output PATH]"
        );
        let missing_output = vec![
            "primitive.json".into(),
            "confirmation.json".into(),
            "--output".into(),
        ];
        assert_eq!(
            predict_combination(&missing_output).unwrap_err(),
            "usage: mic predict-combination PRIMITIVES.json CONFIRMATION.json [--output PATH]"
        );

        let unknown_refit = vec![
            "primitive.json".into(),
            "confirmation.json".into(),
            "refits.json".into(),
            "--bogus".into(),
        ];
        assert_eq!(
            predict_combination_refits(&unknown_refit).unwrap_err(),
            "usage: mic predict-combination-refits PRIMITIVES.json CONFIRMATION.json REFITS.json [--output PATH]"
        );
    }

    #[test]
    fn fitted_transport_reader_rejects_oversized_input_before_parsing() {
        let path = temporary_test_path("bounded-reader");
        fs::write(&path, b"123456789").unwrap();
        assert_eq!(
            read_bounded_request(path.to_str().unwrap(), 8, "test").unwrap_err(),
            "test request exceeds the fixed 8-byte input limit"
        );
        assert_eq!(
            read_bounded_request(path.to_str().unwrap(), 9, "test").unwrap(),
            b"123456789"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn fitted_transport_refit_cli_emits_noncertificate_provenance() {
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let primitive = repository.join("examples/primitive_transport_request.json");
        let confirmation = repository.join("examples/combination_confirmation_request.json");
        let refits = repository.join("examples/transport_refit_request.json");
        let output = temporary_test_path("refit-output");
        let args = vec![
            "predict-combination-refits".into(),
            primitive.to_string_lossy().into_owned(),
            confirmation.to_string_lossy().into_owned(),
            refits.to_string_lossy().into_owned(),
            "--output".into(),
            output.to_string_lossy().into_owned(),
        ];
        run(&args).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(value["authority"], "diagnostic_only");
        assert_eq!(value["certificate_eligible"], false);
        assert_eq!(value["calibrated_test"], false);
        assert_eq!(
            value["stage_order"],
            "primitive_validated_before_confirmation_opened"
        );
        assert_eq!(value["diagnostic"]["authority"], "diagnostic_only");
        assert_eq!(value["diagnostic"]["certificate_eligible"], false);
        assert_eq!(value["diagnostic"]["calibrated_test"], false);
        assert_eq!(
            value["diagnostic"]["feature_transform_treatment"],
            "frozen_not_refit"
        );
        assert_eq!(
            value["diagnostic"]["resample_plan_sha256"]
                .as_str()
                .map(str::len),
            Some(64)
        );
        let provenance = &value["ledger"]["provenance"];
        assert_eq!(value["ledger"]["mode"], "exploratory");
        assert_eq!(
            provenance["primitive_transport_request_sha256"],
            sha256_bytes(&fs::read(&primitive).unwrap())
        );
        assert_eq!(
            provenance["combination_confirmation_request_sha256"],
            sha256_bytes(&fs::read(&confirmation).unwrap())
        );
        assert_eq!(
            provenance["transport_refit_request_sha256"],
            sha256_bytes(&fs::read(&refits).unwrap())
        );
        assert_eq!(
            provenance["transport_refit_plan_sha256"],
            value["diagnostic"]["resample_plan_sha256"]
        );
        assert_eq!(provenance["primitive_transport_seed"], "41");
        assert_eq!(provenance["transport_refit_seed"], "73");
        assert_eq!(provenance["declared_independent_unit"], "experimental_run");
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn finite_completion_example_is_closed_and_unknown_options_fail_before_io() {
        let input: FiniteCompletionRequest = serde_json::from_str(include_str!(
            "../../../examples/finite_completion_request.json"
        ))
        .unwrap();
        assert_eq!(input.schema_version, "1.0.0");
        assert_eq!(input.input.regimes.len(), 2);

        let unknown = vec!["request.json".into(), "--bogus".into(), "out.json".into()];
        assert_eq!(
            finite_completion(&unknown).unwrap_err(),
            "usage: mic finite-completion INPUT.json [--output PATH]"
        );
        let missing_output = vec!["request.json".into(), "--output".into()];
        assert_eq!(
            finite_completion(&missing_output).unwrap_err(),
            "usage: mic finite-completion INPUT.json [--output PATH]"
        );
        assert_eq!(
            kernel_completion(&unknown).unwrap_err(),
            "usage: mic kernel-completion INPUT.json [--output PATH]"
        );
    }
}
