#![forbid(unsafe_code)]
//! Browser boundary for the audit system.
//!
//! The website used to reimplement four audits in JavaScript so that its figures
//! could be interactive: the design audit, the interaction-aliasing split, the
//! preflight gates and the estimator lens battery. Four parallel implementations
//! are four things that can drift from the Rust, and one of them did: the
//! preflight widget reported `Ready` for a modeled selection contract accepted by
//! policy for about an hour after the engine started reporting `DiagnosticOnly`.
//! This crate exists so the page runs the audit system instead of imitating it.
//!
//! Boundary shape, following the coarse-call discipline: every entry point takes
//! and returns one JSON string, so a call is a single crossing rather than a
//! conversation, and every fallible edge returns a structured error naming the
//! stage that failed. Nothing here computes anything itself; the whole file is
//! parameter marshalling around the same functions the CLI calls.

use serde::{Deserialize, Serialize};

use mic_data::ExperimentManifest;
use mic_design::{
    audit_design, audit_interaction_aliasing, audit_sampling_odds, DesignPoint, SquareFace,
};
use mic_engine::{audit_lens_battery, run_preflight, LensEstimate, PreflightPolicy};
use mic_sim::exact_suite;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Serialized failure carrying the stage that refused, so the page can say what
/// broke rather than reporting a bare exception.
#[derive(Debug, Serialize)]
pub struct BoundaryError {
    stage: &'static str,
    message: String,
}

impl BoundaryError {
    pub fn new(stage: &'static str, message: impl Into<String>) -> Self {
        Self { stage, message: message.into() }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            String::from(r#"{"stage":"boundary","message":"error serialization failed"}"#)
        })
    }
}

pub type BoundaryResult = Result<String, String>;

fn encode<T: Serialize>(stage: &'static str, value: &T) -> BoundaryResult {
    serde_json::to_string(value)
        .map_err(|error| BoundaryError::new(stage, error.to_string()).to_json())
}

fn decode<'a, T: Deserialize<'a>>(stage: &'static str, json: &'a str) -> Result<T, String> {
    serde_json::from_str(json)
        .map_err(|error| BoundaryError::new(stage, error.to_string()).to_json())
}

fn parse_corners(stage: &'static str, corners: &[String]) -> Result<Vec<DesignPoint>, String> {
    corners
        .iter()
        .map(|corner| {
            DesignPoint::parse(corner)
                .map_err(|error| BoundaryError::new(stage, error.to_string()).to_json())
        })
        .collect()
}

/// Workspace version, so the page can state which build answered it.
pub fn version_impl() -> String {
    String::from(env!("CARGO_PKG_VERSION"))
}

/// The exact-population fixtures, identical to `mic simulate all`.
pub fn simulate_all_impl() -> BoundaryResult {
    encode("simulate", &exact_suite())
}

/// Factorial geometry over the observed corners: main-effects rank, lack-of-fit
/// dimension and basis, fully observed square faces, and whether those squares
/// span the whole testable space.
pub fn audit_design_impl(corners: &[String], tolerance: f64) -> BoundaryResult {
    let points = parse_corners("design", corners)?;
    let audit = audit_design(&points, tolerance)
        .map_err(|error| BoundaryError::new("design", error.to_string()).to_json())?;
    encode("design", &audit)
}

/// Per-pair estimability: `fully_aliased`, `testable_via_squares`, or
/// `requires_general_contrast`, plus the lack-of-fit directions no observed
/// square reaches.
pub fn audit_aliasing_impl(corners: &[String], tolerance: f64) -> BoundaryResult {
    let points = parse_corners("aliasing", corners)?;
    let audit = audit_interaction_aliasing(&points, tolerance)
        .map_err(|error| BoundaryError::new("aliasing", error.to_string()).to_json())?;
    encode("aliasing", &audit)
}

/// Pooled corner odds for one square face.
pub fn audit_sampling_odds_impl(probabilities: &[f64], tolerance: f64) -> BoundaryResult {
    if probabilities.len() != 4 {
        return Err(BoundaryError::new(
            "sampling",
            format!("expected four corner probabilities, got {}", probabilities.len()),
        )
        .to_json());
    }
    let quad = [probabilities[0], probabilities[1], probabilities[2], probabilities[3]];
    let audit = audit_sampling_odds(quad, tolerance)
        .map_err(|error| BoundaryError::new("sampling", error.to_string()).to_json())?;
    encode("sampling", &audit)
}

/// The full preflight report for a manifest supplied as JSON text, including the
/// evidence ledger and every finding the engine raises.
pub fn preflight_impl(manifest_json: &str, policy_json: &str) -> BoundaryResult {
    let manifest: ExperimentManifest = decode("manifest", manifest_json)?;
    let policy: PreflightPolicy = if policy_json.trim().is_empty() {
        PreflightPolicy::default()
    } else {
        decode("policy", policy_json)?
    };
    let report = run_preflight(&manifest, policy)
        .map_err(|error| BoundaryError::new("preflight", error.to_string()).to_json())?;
    encode("preflight", &report)
}

/// Manifest validation on its own, for the page's schema-check affordance.
pub fn validate_manifest_impl(manifest_json: &str) -> BoundaryResult {
    let manifest: ExperimentManifest = decode("manifest", manifest_json)?;
    manifest
        .validate()
        .map_err(|error| BoundaryError::new("manifest", error.to_string()).to_json())?;
    #[derive(Serialize)]
    struct Summary<'a> {
        experiment_id: &'a str,
        regimes: usize,
        state_columns: usize,
        inference_track: String,
        selection: String,
        strict: bool,
    }
    encode(
        "manifest",
        &Summary {
            experiment_id: &manifest.experiment_id,
            regimes: manifest.regimes.len(),
            state_columns: manifest.state_columns.len(),
            inference_track: format!("{:?}", manifest.inference_track),
            selection: format!("{:?}", manifest.selection),
            strict: manifest.strict,
        },
    )
}

/// The estimator lens battery, with its asymmetric verdict and its fail-closed
/// rejection of degenerate standard errors.
pub fn lens_battery_impl(estimates_json: &str, tolerance: f64) -> BoundaryResult {
    let estimates: Vec<LensEstimate> = decode("lens", estimates_json)?;
    let policy = PreflightPolicy { lens_gap_tolerance: tolerance, ..PreflightPolicy::default() };
    let mut ledger = mic_audit::EvidenceLedger::new(mic_audit::ExecutionMode::Strict);
    let audit = audit_lens_battery(&estimates, &policy, "curvature", &mut ledger)
        .map_err(|error| BoundaryError::new("lens", error.to_string()).to_json())?;

    #[derive(Serialize)]
    struct Combined<'a> {
        audit: &'a mic_engine::LensBatteryAudit,
        findings: &'a [mic_audit::Finding],
        blocking: bool,
    }
    encode(
        "lens",
        &Combined {
            audit: &audit,
            findings: ledger.findings(),
            blocking: ledger.has_blocking_error(),
        },
    )
}

/// Every fully observed square face of the design, as corner bit-strings.
pub fn square_faces_impl(corners: &[String]) -> BoundaryResult {
    let points = parse_corners("faces", corners)?;
    let faces = mic_design::enumerate_square_faces(&points)
        .map_err(|error| BoundaryError::new("faces", error.to_string()).to_json())?;
    let described: Vec<Vec<String>> = faces
        .iter()
        .map(|face: &SquareFace| face.corners().iter().map(DesignPoint::bit_string).collect())
        .collect();
    encode("faces", &described)
}

// ---------------------------------------------------------------------------
// wasm boundary
//
// Every export is a thin wrapper: the panic hook is installed once at module
// start so that a Rust panic arrives as a named message rather than as an opaque
// `unreachable` trap, which is the single highest-value debugging move available
// in a tab.
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    version_impl()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn simulate_all() -> Result<String, String> {
    simulate_all_impl()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn design_audit(corners: Vec<String>, tolerance: f64) -> Result<String, String> {
    audit_design_impl(&corners, tolerance)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn interaction_aliasing(corners: Vec<String>, tolerance: f64) -> Result<String, String> {
    audit_aliasing_impl(&corners, tolerance)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sampling_odds(probabilities: Vec<f64>, tolerance: f64) -> Result<String, String> {
    audit_sampling_odds_impl(&probabilities, tolerance)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn preflight(manifest_json: &str, policy_json: &str) -> Result<String, String> {
    preflight_impl(manifest_json, policy_json)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn validate_manifest(manifest_json: &str) -> Result<String, String> {
    validate_manifest_impl(manifest_json)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn lens_battery(estimates_json: &str, tolerance: f64) -> Result<String, String> {
    lens_battery_impl(estimates_json, tolerance)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn square_faces(corners: Vec<String>) -> Result<String, String> {
    square_faces_impl(&corners)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corners(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn simulate_all_carries_the_paper_fixtures() {
        let json = simulate_all_impl().expect("suite serializes");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let synergy = value["running_example"]["outcome_synergy"].as_f64().expect("synergy");
        assert!((synergy - 0.3).abs() < 1e-14);
    }

    #[test]
    fn full_cube_squares_span_the_lack_of_fit_space() {
        let json = audit_design_impl(
            &corners(&["000", "001", "010", "011", "100", "101", "110", "111"]),
            1e-10,
        )
        .expect("audit");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["main_effects_rank"], 4);
        assert_eq!(value["lack_of_fit_dimension"], 4);
        assert_eq!(value["squares_span_lack_of_fit"], true);
    }

    #[test]
    fn six_corner_cube_has_no_square_but_keeps_two_restrictions() {
        let observed = corners(&["001", "010", "011", "100", "101", "110"]);
        let design: serde_json::Value =
            serde_json::from_str(&audit_design_impl(&observed, 1e-10).expect("audit"))
                .expect("valid json");
        assert_eq!(design["lack_of_fit_dimension"], 2);
        assert_eq!(design["square_faces"].as_array().expect("faces").len(), 0);
        assert_eq!(design["squares_span_lack_of_fit"], false);

        let alias: serde_json::Value =
            serde_json::from_str(&audit_aliasing_impl(&observed, 1e-10).expect("aliasing"))
                .expect("valid json");
        assert_eq!(alias["untested_lack_of_fit_dimension"], 2);
    }

    #[test]
    fn non_product_quotas_are_reported_as_such() {
        let json = audit_sampling_odds_impl(&[0.10, 0.20, 0.30, 0.40], 1e-10).expect("odds");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["is_product"], false);
        let log_odds = value["log_odds_ratio"].as_f64().expect("log odds");
        assert!((log_odds - (2.0f64 / 3.0).ln()).abs() < 1e-12);
    }

    #[test]
    fn a_degenerate_standard_error_is_refused() {
        let estimates = r#"[
            {"family":"four_law","estimate":0.04,"standard_error":0.01},
            {"family":"gcm","estimate":0.04,"standard_error":0.0}
        ]"#;
        let error = lens_battery_impl(estimates, 3.0).expect_err("must fail closed");
        assert!(error.contains("\"stage\":\"lens\""), "error names its stage: {error}");
    }

    #[test]
    fn malformed_json_names_the_stage_that_refused() {
        let error = preflight_impl("{ not json", "").expect_err("must fail");
        assert!(error.contains("\"stage\":\"manifest\""), "got {error}");
    }
}
