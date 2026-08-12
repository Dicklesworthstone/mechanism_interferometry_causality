//! Tabular four-law surface that does not touch the reserved `mic` CLI.

#![forbid(unsafe_code)]

use mic_audit::CertificateStatus;
use mic_data::ExperimentManifest;
use mic_engine::{
    FourLawPolicy, PreflightPolicy, SurveyPolicy, run_tabular_audit, run_unsupervised_survey,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
        "ingest" | "four-law" | "report" => {
            let path = args.get(1).ok_or_else(|| {
                format!(
                    "usage: mic-tabular {command} MANIFEST.json [--output PATH] [--base-dir DIR]"
                )
            })?;
            let manifest =
                ExperimentManifest::from_json_path(path).map_err(|error| error.to_string())?;
            let base = option_value(args, "--base-dir")
                .map(PathBuf::from)
                .or_else(|| Path::new(path).parent().map(Path::to_path_buf));
            let report = run_tabular_audit(
                &manifest,
                FourLawPolicy::default(),
                PreflightPolicy::default(),
                base.as_deref(),
            )
            .map_err(|error| error.to_string())?;
            let value = match command {
                "ingest" => {
                    serde_json::to_value(report.ingest()).map_err(|error| error.to_string())?
                }
                "four-law" => serde_json::to_value(&report).map_err(|error| error.to_string())?,
                "report" => {
                    let narrative = report.narrative();
                    serde_json::json!({
                        "status": report.status(),
                        "preflight_status": report.preflight().status(),
                        "narrative": narrative,
                        "audit": report,
                    })
                }
                _ => unreachable!(),
            };
            if command == "report" {
                println!("{}", report.narrative().markdown());
            }
            if let Some(output) = option_value(args, "--output") {
                let pretty =
                    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
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
        "survey" => {
            let path = args
                .get(1)
                .ok_or("usage: mic-tabular survey TABLE.csv [--cluster COL] [--output PATH] [--base-dir DIR]")?;
            let base = option_value(args, "--base-dir").map(PathBuf::from);
            let cluster = option_value(args, "--cluster");
            let report =
                run_unsupervised_survey(path, base.as_deref(), cluster, SurveyPolicy::default())
                    .map_err(|error| error.to_string())?;
            let value = serde_json::to_value(&report).map_err(|error| error.to_string())?;
            if let Some(output) = option_value(args, "--output") {
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
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn option_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2).find_map(|window| {
        if window[0] == name {
            Some(window[1].as_str())
        } else {
            None
        }
    })
}

fn print_help() {
    println!(
        "Mechanism Interferometry tabular four-law surface\n\n\
         This binary exists because `mic` CLI wiring is reserved by another agent.\n\
         It is the std-CSV reader, not the FrankenPandas Packet 1 adapter.\n\n\
         Usage:\n\
           mic-tabular ingest MANIFEST.json [--output PATH] [--base-dir DIR]\n\
           mic-tabular four-law MANIFEST.json [--output PATH] [--base-dir DIR]\n\
           mic-tabular report MANIFEST.json [--output PATH] [--base-dir DIR]\n\
           mic-tabular survey TABLE.csv [--cluster COL] [--output PATH] [--base-dir DIR]\n\
           mic-tabular help"
    );
}
