#![forbid(unsafe_code)]
//! Command-line entry point for simulation, design, manifest, and preflight audits.

use mic_audit::{EvidenceLedger, ExecutionMode};
use mic_data::ExperimentManifest;
use mic_design::{DesignPoint, audit_design, audit_sampling_odds};
use mic_engine::{PreflightPolicy, audit_orientation, run_preflight};
use mic_sim::{
    exact_suite, implementation_inconsistency, latent_conservation, parity_example, running_example,
};
use serde::Deserialize;
use std::env;
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
        "orient" => orient(&args[1..]),
        "propose-tilt" => propose_tilt(&args[1..]),
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

/// One raw deletion bound row supplied by the orientation input file.
#[derive(Debug, Deserialize)]
struct OrientDeletionInput {
    variable: String,
    relative_discrepancy: f64,
    lower: f64,
    upper: f64,
}

/// Input contract for `mic orient`.
#[derive(Debug, Deserialize)]
struct OrientInput {
    epsilon: f64,
    full_discrepancy: f64,
    min_full_discrepancy: f64,
    #[serde(default = "default_strict")]
    strict: bool,
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
    let input: OrientInput = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
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
           mic simulate [all|running|parity|latent|implementation] [--output PATH]\n\
           mic design odds P00 P10 P01 P11\n\
           mic design audit CORNER...\n\
           mic design audit MANIFEST.json\n\
           mic validate-manifest MANIFEST.json\n\
           mic preflight MANIFEST.json [--output PATH]\n\
           mic orient INPUT.json [--output PATH]\n\
           mic propose-tilt INPUT.json [--output PATH]\n\
           mic version"
    );
}
