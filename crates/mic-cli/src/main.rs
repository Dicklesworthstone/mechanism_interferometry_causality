#![forbid(unsafe_code)]
//! Command-line entry point for simulation, design, manifest, and preflight audits.

use mic_audit::{EvidenceLedger, ExecutionMode};
use mic_data::ExperimentManifest;
use mic_design::{DesignPoint, audit_design, audit_sampling_odds};
use mic_engine::{PreflightPolicy, audit_orientation, run_preflight};
use mic_sim::{
    causal_tomography_chain, exact_suite, flat_noncausal_cube, hidden_sensor_tomography,
    identification_twins, implementation_inconsistency, latent_conservation, parity_example,
    running_example,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

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
            println!("mic {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "simulate" => simulate(&args[1..]),
        "design" => design(&args[1..]),
        "validate-manifest" => validate_manifest(&args[1..]),
        "preflight" => preflight(&args[1..]),
        "closure-crossfit" => closure_crossfit(&args[1..]),
        "orient" => orient(&args[1..]),
        "propose-tilt" => propose_tilt(&args[1..]),
        "freeze-scout" => freeze_scout(&args[1..]),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn simulate(args: &[String]) -> Result<(), String> {
    let scenario = args.first().map_or("all", String::as_str);
    let output = option_value(args, "--output").map(PathBuf::from);
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
                ExperimentManifest::from_json_path(&labels[0])
                    .map_err(|error| error.to_string())?
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
    let path = args.first().ok_or("usage: mic validate-manifest PATH")?;
    let manifest = ExperimentManifest::from_json_path(path).map_err(|error| error.to_string())?;
    println!(
        "validated experiment {} with {} regimes and {} state columns",
        manifest.experiment_id,
        manifest.regimes.len(),
        manifest.state_columns.len()
    );
    Ok(())
}

fn preflight(args: &[String]) -> Result<(), String> {
    let path = args
        .first()
        .ok_or("usage: mic preflight MANIFEST.json [--output PATH]")?;
    let manifest = ExperimentManifest::from_json_path(path).map_err(|error| error.to_string())?;
    let policy = PreflightPolicy {
        accept_unvalidated_selection_model: has_flag(args, "--allow-unvalidated-selection-model"),
        ..PreflightPolicy::default()
    };
    let report = run_preflight(&manifest, policy).map_err(|error| error.to_string())?;
    let value = serde_json::to_value(report).map_err(|error| error.to_string())?;
    let output = option_value(args, "--output").map(PathBuf::from);
    write_json_value(&value, output.as_deref())
}

/// Input contract for the diagnostic reference closure model.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClosureCrossFitInput {
    schema_version: String,
    declared_independent_unit: String,
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
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let input_sha256 = sha256_bytes(&bytes);
    let input: ClosureCrossFitInput =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if input.schema_version != "1.0.0" {
        return Err("closure-crossfit schema_version must be 1.0.0".into());
    }
    if input.declared_independent_unit.trim().is_empty() {
        return Err("declared_independent_unit must not be empty".into());
    }
    let diagnostic = mic_model::cross_fit_closure_models(
        &input.samples,
        input.sampling_proportions,
        input.config,
    )
    .map_err(|error| error.to_string())?;
    let mut ledger = EvidenceLedger::new(ExecutionMode::Exploratory);
    ledger.provenance("closure_crossfit_input_sha256", &input_sha256);
    ledger.provenance("closure_crossfit_seed", diagnostic.seed.to_string());
    ledger.provenance(
        "closure_crossfit_fold_plan_sha256",
        &diagnostic.fold_plan_sha256,
    );
    ledger.provenance(
        "declared_independent_unit",
        &input.declared_independent_unit,
    );
    let value = serde_json::json!({
        "schema_version": "1.0.0",
        "authority": "diagnostic_only",
        "certificate_eligible": false,
        "input_sha256": input_sha256,
        "declared_independent_unit": input.declared_independent_unit,
        "diagnostic": diagnostic,
        "ledger": ledger,
    });
    write_json_value(&value, output.as_deref())
}

/// One raw deletion bound row supplied by the orientation input file.
#[derive(Debug, Deserialize)]
struct OrientDeletionInput {
    variable: String,
    relative_discrepancy: f64,
    lower: f64,
    upper: f64,
}

/// Provenance required for externally supplied simultaneous orientation bounds.
#[derive(Debug, Deserialize)]
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
    let path = args
        .first()
        .ok_or("usage: mic orient INPUT.json [--output PATH]")?;
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
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
    let value = serde_json::json!({ "audit": audit, "ledger": ledger });
    let output = option_value(args, "--output").map(PathBuf::from);
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
struct ProposeTiltInput {
    request: mic_proposal::ActiveTiltRequest,
    candidates: Vec<mic_proposal::ActiveTiltCandidate>,
}

fn propose_tilt(args: &[String]) -> Result<(), String> {
    let path = args
        .first()
        .ok_or("usage: mic propose-tilt INPUT.json [--output PATH]")?;
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let input: ProposeTiltInput =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let proposal = mic_proposal::rank_active_tilts(&input.request, &input.candidates)
        .map_err(|error| error.to_string())?;
    let value = serde_json::to_value(proposal).map_err(|error| error.to_string())?;
    let output = option_value(args, "--output").map(PathBuf::from);
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
    let request_bytes = fs::read(request_path).map_err(|error| error.to_string())?;
    let draft_bytes = fs::read(draft_path).map_err(|error| error.to_string())?;
    let request: mic_proposal::SelfDrivingRequest =
        serde_json::from_slice(&request_bytes).map_err(|error| error.to_string())?;
    let draft: mic_proposal::ShiftFactorizationDraft =
        serde_json::from_slice(&draft_bytes).map_err(|error| error.to_string())?;
    let proposal = mic_proposal::freeze_shift_factorization_proposal(&request, &draft)
        .map_err(|error| error.to_string())?;
    let value = serde_json::to_value(proposal).map_err(|error| error.to_string())?;
    write_json_value(&value, output.as_deref())
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

fn option_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == name)
        .map(|window| window[1].as_str())
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|argument| argument == name)
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
           mic orient INPUT.json [--output PATH]\n\
           mic propose-tilt INPUT.json [--output PATH]\n\
           mic freeze-scout REQUEST.json DRAFT.json [--output PATH]\n\
           mic version"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn closure_crossfit_example_is_closed_and_unknown_options_fail_before_io() {
        let input: ClosureCrossFitInput = serde_json::from_str(include_str!(
            "../../../examples/closure_crossfit_request.json"
        ))
        .unwrap();
        assert_eq!(input.schema_version, "1.0.0");
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
}
