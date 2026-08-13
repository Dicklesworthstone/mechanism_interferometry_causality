#![forbid(unsafe_code)]
//! Tabular four-law surface that does not touch the reserved `mic` CLI.

use mic_audit::CertificateStatus;
use mic_data::ExperimentManifest;
use mic_engine::{
    FourLawPolicy, PreflightPolicy, SurveyPolicy, resolve_selection_evidence_from_files,
    run_tabular_audit, run_tabular_audit_with_selection_evidence, run_unsupervised_survey,
};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_JSON_REQUEST_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Default)]
struct AuditArgs<'a> {
    output: Option<&'a str>,
    base_dir: Option<&'a str>,
    allow_unvalidated_selection_model: bool,
    selection_receipt: Option<&'a str>,
    selection_authority_source: Option<&'a str>,
}

#[derive(Default)]
struct SurveyArgs<'a> {
    output: Option<&'a str>,
    base_dir: Option<&'a str>,
    cluster: Option<&'a str>,
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if let Err(error) = run(&args) {
        eprintln!("mic-tabular: {error}");
        std::process::exit(2);
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    match command {
        "ingest" | "four-law" | "report" => run_audit_command(command, args),
        "survey" => {
            let path = args
                .get(1)
                .ok_or("usage: mic-tabular survey TABLE.csv [--cluster COL] [--output PATH] [--base-dir DIR]")?;
            let parsed = parse_survey_args(&args[2..])?;
            let base = parsed.base_dir.map(PathBuf::from);
            let report = run_unsupervised_survey(
                path,
                base.as_deref(),
                parsed.cluster,
                SurveyPolicy::default(),
            )
            .map_err(|error| error.to_string())?;
            let value = serde_json::to_value(&report).map_err(|error| error.to_string())?;
            if let Some(output) = parsed.output {
                fs::write(
                    output,
                    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
                );
            }
            Ok(())
        }
        "help" | "--help" | "-h" => {
            if args.len() != 1 {
                return Err("help accepts no additional arguments".into());
            }
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn run_audit_command(command: &str, args: &[String]) -> Result<(), String> {
    let path = args.get(1).ok_or_else(|| {
        format!(
            "usage: mic-tabular {command} MANIFEST.json [--output PATH] [--base-dir DIR] [--allow-unvalidated-selection-model]"
        )
    })?;
    let parsed = parse_audit_args(&args[2..])?;
    let manifest = read_manifest_bounded(path)?;
    let base = parsed
        .base_dir
        .map(PathBuf::from)
        .or_else(|| Path::new(path).parent().map(Path::to_path_buf));
    let policy = PreflightPolicy {
        accept_unvalidated_selection_model: parsed.allow_unvalidated_selection_model,
        ..PreflightPolicy::default()
    };
    let report = match (parsed.selection_receipt, parsed.selection_authority_source) {
        (None, None) => {
            run_tabular_audit(&manifest, FourLawPolicy::default(), policy, base.as_deref())
        }
        (Some(receipt), Some(authority)) => {
            let evidence = resolve_selection_evidence_from_files(
                &manifest,
                receipt,
                authority,
                base.as_deref(),
            )
            .map_err(|error| error.to_string())?;
            run_tabular_audit_with_selection_evidence(
                &manifest,
                FourLawPolicy::default(),
                policy,
                base.as_deref(),
                &evidence,
            )
        }
        _ => {
            return Err(
                "--selection-receipt and --selection-authority-source must be supplied together"
                    .into(),
            );
        }
    }
    .map_err(|error| error.to_string())?;
    let value = match command {
        "ingest" => serde_json::to_value(report.ingest()).map_err(|error| error.to_string())?,
        "four-law" => serde_json::to_value(&report).map_err(|error| error.to_string())?,
        "report" => serde_json::json!({
            "status": report.status(),
            "preflight_status": report.preflight().status(),
            "narrative": report.narrative(),
            "audit": report,
        }),
        _ => unreachable!(),
    };
    if command == "report" {
        println!("{}", report.narrative().markdown());
    }
    if let Some(output) = parsed.output {
        let pretty = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
        fs::write(output, pretty).map_err(|error| error.to_string())?;
    } else if command != "report" {
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
        );
    }
    if matches!(
        report.status(),
        CertificateStatus::Abstained | CertificateStatus::DiagnosticOnly
    ) && command != "ingest"
    {
        // Abstention is the honest default, not a process failure.
    }
    Ok(())
}

fn parse_audit_args(args: &[String]) -> Result<AuditArgs<'_>, String> {
    let mut parsed = AuditArgs::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => set_option(&mut parsed.output, args, &mut index, "--output")?,
            "--base-dir" => set_option(&mut parsed.base_dir, args, &mut index, "--base-dir")?,
            "--selection-receipt" => set_option(
                &mut parsed.selection_receipt,
                args,
                &mut index,
                "--selection-receipt",
            )?,
            "--selection-authority-source" => set_option(
                &mut parsed.selection_authority_source,
                args,
                &mut index,
                "--selection-authority-source",
            )?,
            "--allow-unvalidated-selection-model" => {
                if parsed.allow_unvalidated_selection_model {
                    return Err("duplicate --allow-unvalidated-selection-model".into());
                }
                parsed.allow_unvalidated_selection_model = true;
            }
            other => return Err(format!("unknown option {other:?}")),
        }
        index += 1;
    }
    if parsed.allow_unvalidated_selection_model
        && (parsed.selection_receipt.is_some() || parsed.selection_authority_source.is_some())
    {
        return Err(
            "--allow-unvalidated-selection-model cannot be combined with selection evidence".into(),
        );
    }
    Ok(parsed)
}

fn parse_survey_args(args: &[String]) -> Result<SurveyArgs<'_>, String> {
    let mut parsed = SurveyArgs::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => set_option(&mut parsed.output, args, &mut index, "--output")?,
            "--base-dir" => set_option(&mut parsed.base_dir, args, &mut index, "--base-dir")?,
            "--cluster" => set_option(&mut parsed.cluster, args, &mut index, "--cluster")?,
            other => return Err(format!("unknown option {other:?}")),
        }
        index += 1;
    }
    Ok(parsed)
}

fn set_option<'a>(
    slot: &mut Option<&'a str>,
    args: &'a [String],
    index: &mut usize,
    name: &str,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("duplicate {name}"));
    }
    *index += 1;
    let value = args
        .get(*index)
        .ok_or_else(|| format!("{name} requires a value"))?;
    if value.starts_with("--") {
        return Err(format!("{name} requires a value"));
    }
    *slot = Some(value);
    Ok(())
}

fn read_manifest_bounded(path: impl AsRef<Path>) -> Result<ExperimentManifest, String> {
    let path = path.as_ref();
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if metadata.len() > MAX_JSON_REQUEST_BYTES {
        return Err(format!(
            "request exceeds the {MAX_JSON_REQUEST_BYTES}-byte JSON limit"
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.take(MAX_JSON_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_JSON_REQUEST_BYTES {
        return Err(format!(
            "request exceeds the {MAX_JSON_REQUEST_BYTES}-byte JSON limit"
        ));
    }
    let manifest: ExperimentManifest =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    manifest.validate().map_err(|error| error.to_string())?;
    Ok(manifest)
}

fn print_help() {
    println!(
        "Mechanism Interferometry tabular four-law surface\n\n\
         This binary exists because `mic` CLI wiring is reserved by another agent.\n\
         It is the std-CSV reader, not the FrankenPandas Packet 1 adapter.\n\n\
         Usage:\n\
           mic-tabular ingest MANIFEST.json [--output PATH] [--base-dir DIR] [--allow-unvalidated-selection-model | --selection-receipt PATH --selection-authority-source PATH]\n\
           mic-tabular four-law MANIFEST.json [--output PATH] [--base-dir DIR] [--allow-unvalidated-selection-model | --selection-receipt PATH --selection-authority-source PATH]\n\
           mic-tabular report MANIFEST.json [--output PATH] [--base-dir DIR] [--allow-unvalidated-selection-model | --selection-receipt PATH --selection-authority-source PATH]\n\
           mic-tabular survey TABLE.csv [--cluster COL] [--output PATH] [--base-dir DIR]\n\
           mic-tabular help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn audit_arguments_reject_unknown_duplicate_and_conflicting_authority_flags() {
        assert!(parse_audit_args(&args(&["--bogus"])).is_err());
        assert!(parse_audit_args(&args(&["--output", "a", "--output", "b"])).is_err());
        assert!(
            parse_audit_args(&args(&[
                "--allow-unvalidated-selection-model",
                "--selection-receipt",
                "receipt.json",
                "--selection-authority-source",
                "source.txt",
            ]))
            .is_err()
        );
    }

    #[test]
    fn survey_arguments_require_values_and_reject_unknown_options() {
        assert!(parse_survey_args(&args(&["--cluster"])).is_err());
        assert!(parse_survey_args(&args(&["--output", "--bogus"])).is_err());
        assert!(parse_survey_args(&args(&["--bogus", "value"])).is_err());
    }
}
