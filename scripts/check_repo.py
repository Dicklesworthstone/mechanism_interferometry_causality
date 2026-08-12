#!/usr/bin/env python3
from __future__ import annotations

import csv
import hashlib
import json
import math
import os
import re
import struct
import subprocess
import sys
import tomllib
from pathlib import Path

from jsonschema import Draft202012Validator
from lxml import html

ROOT = Path(__file__).resolve().parents[1]
ERRORS: list[str] = []
CHECKS = 0


def fail(message: str) -> None:
    ERRORS.append(message)


def check(condition: bool, message: str) -> None:
    global CHECKS
    CHECKS += 1
    if not condition:
        fail(message)


def load_json(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        fail(f"invalid JSON {path.relative_to(ROOT)}: {exc}")
        return {}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def active_tilt_candidate_fingerprint(candidates: list[object]) -> str:
    """Mirror mic-proposal's exact, order-sensitive candidate-library framing."""
    digest = hashlib.sha256()
    digest.update(b"mic-active-tilt-candidates-v1\0")

    def add_length(value: int) -> None:
        digest.update(value.to_bytes(8, "big"))

    def add_string(value: object) -> None:
        encoded = str(value).encode("utf-8")
        add_length(len(encoded))
        digest.update(encoded)

    add_length(len(candidates))
    for item in candidates:
        if not isinstance(item, dict):
            continue
        add_string(item.get("candidate_id", ""))
        add_string(item.get("primitive_id", ""))
        digest.update(bytes([bool(item.get("measurable_delivery"))]))
        digest.update(bytes([bool(item.get("common_support"))]))
        eligibility = item.get("design_eligibility", {})
        status = eligibility.get("status") if isinstance(eligibility, dict) else None
        if status == "not_required_for_four_law":
            digest.update(b"\x00")
        elif status == "product_odds_verified":
            digest.update(b"\x01")
            add_string(eligibility.get("audit_id", ""))
        elif status == "reweighted_to_product":
            digest.update(b"\x02")
            add_string(eligibility.get("plan_id", ""))
        else:
            digest.update(b"\x03")
        predictions = item.get("predicted_pairwise_separations", [])
        add_length(len(predictions))
        for prediction in predictions:
            add_string(prediction.get("first", ""))
            add_string(prediction.get("second", ""))
            digest.update(struct.pack(">d", float(prediction.get("separation", math.nan))))
        cost = item.get("cost")
        if cost is None:
            digest.update(b"\x00")
        else:
            digest.update(b"\x01")
            digest.update(struct.pack(">d", float(cost)))
    return f"sha256:{digest.hexdigest()}"


def required_files() -> None:
    paths = [
        "README.md",
        "REPOSITORY_MANIFEST.json",
        "Cargo.toml",
        "pyproject.toml",
        "paper/main.tex",
        "paper/main.pdf",
        "paper/references.bib",
        "site/index.html",
        "site/styles.css",
        "site/app.js",
        "site/mechanism_interferometry.pdf",
        "schemas/experiment_manifest.schema.json",
        "schemas/evidence_finding.schema.json",
        "schemas/audit_report.schema.json",
        "schemas/active_tilt_input.schema.json",
        "schemas/proposal_batch.schema.json",
        "examples/proposal_inputs/parity_active_tilt.json",
        "examples/proposals/parity_active_tilt.json",
        "crates/mic-proposal/Cargo.toml",
        "crates/mic-proposal/src/lib.rs",
        "scripts/generate_simulations.py",
        "scripts/generate_example_data.py",
        "scripts/generate_repository_manifest.py",
        "scripts/package_release.sh",
        "scripts/build_all.sh",
        "docs/FORMAL_SPEC.md",
        "docs/IMPLEMENTATION_BLUEPRINT.md",
        "docs/INFERENCE_PROTOCOL.md",
        "docs/PROPOSAL_ADAPTERS.md",
        "docs/FRANKEN_INTEGRATION.md",
    ]
    for relative in paths:
        check((ROOT / relative).is_file(), f"missing required file {relative}")


def validate_schemas_and_manifests() -> None:
    schema_dir = ROOT / "schemas"
    schemas: dict[str, dict] = {}
    for path in sorted(schema_dir.glob("*.schema.json")):
        document = load_json(path)
        if not isinstance(document, dict):
            continue
        try:
            Draft202012Validator.check_schema(document)
        except Exception as exc:
            fail(f"invalid JSON Schema {path.relative_to(ROOT)}: {exc}")
        schemas[path.name] = document

    manifest_schema = schemas.get("experiment_manifest.schema.json")
    proposal_input_schema = schemas.get("active_tilt_input.schema.json")
    proposal_schema = schemas.get("proposal_batch.schema.json")
    check(proposal_input_schema is not None, "active-tilt input schema was not loaded")
    if proposal_input_schema is not None:
        input_validator = Draft202012Validator(proposal_input_schema)
        for path in sorted((ROOT / "examples" / "proposal_inputs").glob("*.json")):
            proposal_input = load_json(path)
            errors = sorted(input_validator.iter_errors(proposal_input), key=lambda error: list(error.path))
            for error in errors:
                location = ".".join(str(part) for part in error.path) or "<root>"
                fail(f"{path.relative_to(ROOT)} proposal-input schema violation at {location}: {error.message}")
            if not isinstance(proposal_input, dict):
                continue
            request = proposal_input.get("request", {})
            candidates = proposal_input.get("candidates", [])
            if not isinstance(request, dict) or not isinstance(candidates, list):
                continue
            feature_flags = request.get("source", {}).get("feature_flags", [])
            check(feature_flags == sorted(set(feature_flags)), f"{path.name} input feature flags are not canonicalized")
            candidate_ids = [candidate.get("candidate_id") for candidate in candidates if isinstance(candidate, dict)]
            check(len(candidate_ids) == len(set(candidate_ids)), f"{path.name} input repeats a candidate identifier")

    check(proposal_schema is not None, "proposal batch schema was not loaded")
    if proposal_schema is not None:
        proposal_validator = Draft202012Validator(proposal_schema)
        for path in sorted((ROOT / "examples" / "proposals").glob("*.json")):
            proposal = load_json(path)
            errors = sorted(proposal_validator.iter_errors(proposal), key=lambda error: list(error.path))
            for error in errors:
                location = ".".join(str(part) for part in error.path) or "<root>"
                fail(f"{path.relative_to(ROOT)} proposal schema violation at {location}: {error.message}")
            if not isinstance(proposal, dict):
                continue
            check(proposal.get("authority") == "proposal_only", f"{path.name} grants proposal artifact certificate authority")
            fingerprint = str(proposal.get("candidate_library_fingerprint", ""))
            check(bool(re.fullmatch(r"sha256:[0-9a-f]{64}", fingerprint)), f"{path.name} has an invalid candidate-library fingerprint")
            hypotheses = proposal.get("surviving_hypotheses", [])
            check(hypotheses == sorted(hypotheses), f"{path.name} hypotheses are not canonicalized")
            expected_pairs = {
                (hypotheses[left], hypotheses[right])
                for left in range(len(hypotheses))
                for right in range(left + 1, len(hypotheses))
            }
            source = proposal.get("source", {})
            feature_flags = source.get("feature_flags", []) if isinstance(source, dict) else []
            check(feature_flags == sorted(set(feature_flags)), f"{path.name} feature flags are not canonicalized")
            rankings = proposal.get("rankings", [])
            if isinstance(rankings, list):
                ranks = [item.get("rank") for item in rankings if isinstance(item, dict)]
                check(ranks == list(range(1, len(rankings) + 1)), f"{path.name} ranks are not consecutive")
                scores = [item.get("worst_case_predicted_separation") for item in rankings if isinstance(item, dict)]
                check(scores == sorted(scores, reverse=True), f"{path.name} maximin scores are not descending")
                candidate_ids = [item.get("candidate_id") for item in rankings if isinstance(item, dict)]
                check(len(candidate_ids) == len(set(candidate_ids)), f"{path.name} repeats a ranked candidate")
                for item in rankings:
                    if not isinstance(item, dict):
                        continue
                    check(item.get("primitive_id") == proposal.get("primitive_id"), f"{path.name} ranked tilt changes primitive")
                    predictions = item.get("predicted_pairwise_separations", [])
                    pair_keys = [
                        (prediction.get("first"), prediction.get("second"))
                        for prediction in predictions
                        if isinstance(prediction, dict)
                    ]
                    check(pair_keys == sorted(pair_keys), f"{path.name} pairwise predictions are not canonicalized")
                    check(set(pair_keys) == expected_pairs and len(pair_keys) == len(expected_pairs), f"{path.name} lacks a complete unique hypothesis-pair table")
                    values = [
                        float(prediction["separation"])
                        for prediction in predictions
                        if isinstance(prediction, dict) and "separation" in prediction
                    ]
                    score = float(item.get("worst_case_predicted_separation", math.nan))
                    check(bool(values) and math.isclose(score, min(values), abs_tol=1e-14), f"{path.name} maximin score does not match raw predictions")
                    if proposal.get("planned_analysis") == "product_factorial":
                        eligibility = item.get("design_eligibility", {})
                        status = eligibility.get("status") if isinstance(eligibility, dict) else None
                        check(status in {"product_odds_verified", "reweighted_to_product"}, f"{path.name} ranks a product-factorial tilt without design evidence")
                selected = proposal.get("selected_candidate_id")
                expected = rankings[0].get("candidate_id") if rankings and isinstance(rankings[0], dict) else None
                check(selected == expected, f"{path.name} selected candidate does not match rank one")
                expected_status = "recommended" if rankings else "abstained_no_eligible_candidate"
                check(proposal.get("status") == expected_status, f"{path.name} proposal status does not match eligibility")
                rejected = proposal.get("rejected", [])
                rejected_ids = {item.get("candidate_id") for item in rejected if isinstance(item, dict)}
                check(not rejected_ids.intersection(candidate_ids), f"{path.name} both ranks and rejects a candidate")
            semantics = str(proposal.get("score_semantics", "")).lower()
            check("not probability or confidence" in semantics, f"{path.name} does not quarantine proposal score semantics")
            ranking_policy = str(proposal.get("ranking_policy", "")).lower()
            check("candidate_id" in ranking_policy and "cost" in ranking_policy, f"{path.name} does not freeze deterministic tie-breaking")

            input_path = ROOT / "examples" / "proposal_inputs" / path.name
            if input_path.is_file():
                proposal_input = load_json(input_path)
                if isinstance(proposal_input, dict):
                    request = proposal_input.get("request", {})
                    candidates = proposal_input.get("candidates", [])
                    if isinstance(request, dict) and isinstance(candidates, list):
                        for field in ["schema_version", "proposal_id", "primitive_id", "planned_analysis", "source", "seed"]:
                            check(proposal.get(field) == request.get(field), f"{path.name} output drifted from input field {field}")
                        check(proposal.get("surviving_hypotheses") == sorted(request.get("surviving_hypotheses", [])), f"{path.name} output hypotheses drifted from input")
                        input_ids = {item.get("candidate_id") for item in candidates if isinstance(item, dict)}
                        output_ids = {
                            item.get("candidate_id")
                            for collection in [proposal.get("rankings", []), proposal.get("rejected", [])]
                            for item in collection
                            if isinstance(item, dict)
                        }
                        check(input_ids == output_ids, f"{path.name} does not account for every input candidate")
                        expected_fingerprint = active_tilt_candidate_fingerprint(candidates)
                        check(proposal.get("candidate_library_fingerprint") == expected_fingerprint, f"{path.name} candidate-library fingerprint drifted from input")

    check(manifest_schema is not None, "experiment manifest schema was not loaded")
    if manifest_schema is None:
        return
    validator = Draft202012Validator(manifest_schema)
    for path in sorted((ROOT / "examples" / "configs").glob("*.json")):
        manifest = load_json(path)
        errors = sorted(validator.iter_errors(manifest), key=lambda error: list(error.path))
        for error in errors:
            location = ".".join(str(part) for part in error.path) or "<root>"
            fail(f"{path.relative_to(ROOT)} schema violation at {location}: {error.message}")
        if not isinstance(manifest, dict):
            continue
        regimes = manifest.get("regimes", [])
        if isinstance(regimes, list):
            proportions = [float(regime["sampling_proportion"]) for regime in regimes if isinstance(regime, dict)]
            check(math.isclose(sum(proportions), 1.0, abs_tol=1e-10), f"sampling proportions do not sum to one in {path.name}")
            dimensions = {
                len(regime.get("design", {}).get("bits", []))
                for regime in regimes
                if isinstance(regime, dict)
            }
            check(len(dimensions) == 1, f"regime dimensions differ in {path.name}")
            ids = [regime.get("id") for regime in regimes if isinstance(regime, dict)]
            corners = [tuple(regime.get("design", {}).get("bits", [])) for regime in regimes if isinstance(regime, dict)]
            check(len(ids) == len(set(ids)), f"duplicate regime id in {path.name}")
            check(len(corners) == len(set(corners)), f"duplicate design corner in {path.name}")
        data = manifest.get("data", {})
        if isinstance(data, dict) and data.get("format") != "synthetic":
            source = ROOT / str(data.get("path", ""))
            check(source.is_file(), f"manifest data path does not exist: {source.relative_to(ROOT) if source.is_absolute() and ROOT in source.parents else source}")
            if source.suffix.lower() == ".csv" and source.is_file():
                with source.open(newline="", encoding="utf-8") as handle:
                    reader = csv.DictReader(handle)
                    headers = set(reader.fieldnames or [])
                    rows = list(reader)
                required_columns = {
                    manifest.get("cluster_column"),
                    manifest.get("regime_column"),
                    *manifest.get("state_columns", []),
                    *(column for block in manifest.get("candidate_state_blocks", []) for column in block),
                }
                required_columns.discard(None)
                check(required_columns <= headers, f"{source.name} is missing declared columns {sorted(required_columns - headers)}")
                check(len(rows) > 0, f"{source.name} contains no data rows")



IGNORED_DIRECTORIES = {".git", "_renders", "target", "__pycache__", ".venv", "venv", "dist"}


def manifest_included(path: Path) -> bool:
    relative = path.relative_to(ROOT)
    ignored_directories = IGNORED_DIRECTORIES
    ignored_suffixes = {
        ".aux", ".bbl", ".bcf", ".blg", ".fdb_latexmk", ".fls",
        ".log", ".out", ".pyc", ".run.xml", ".toc",
    }
    if any(part in ignored_directories for part in relative.parts):
        return False
    if path.name == "REPOSITORY_MANIFEST.json":
        return False
    if path.name.endswith(tuple(ignored_suffixes)):
        return False
    return path.is_file()


def validate_repository_manifest() -> None:
    path = ROOT / "REPOSITORY_MANIFEST.json"
    if not path.is_file():
        return
    document = load_json(path)
    if not isinstance(document, dict):
        return
    check(document.get("schema_version") == "1.0.0", "unexpected repository manifest schema version")
    check(document.get("hash_algorithm") == "sha256", "repository manifest must use sha256")
    files = document.get("files")
    check(isinstance(files, list), "repository manifest files must be a list")
    if not isinstance(files, list):
        return
    entries: dict[str, dict] = {}
    for item in files:
        check(isinstance(item, dict), "repository manifest contains a non-object entry")
        if not isinstance(item, dict):
            continue
        relative = item.get("path")
        check(isinstance(relative, str) and bool(relative), "repository manifest entry has invalid path")
        if not isinstance(relative, str) or not relative:
            continue
        check(relative not in entries, f"repository manifest repeats {relative}")
        entries[relative] = item
        target = ROOT / relative
        check(target.is_file(), f"repository manifest path is missing: {relative}")
        if target.is_file():
            check(item.get("bytes") == target.stat().st_size, f"repository manifest byte count drift: {relative}")
            check(item.get("sha256") == sha256(target), f"repository manifest hash drift: {relative}")
    actual = {path.relative_to(ROOT).as_posix() for path in ROOT.rglob("*") if manifest_included(path)}
    declared = set(entries)
    check(actual == declared, f"repository manifest path set drift: missing={sorted(actual - declared)}, extra={sorted(declared - actual)}")
    check(document.get("file_count") == len(files), "repository manifest file_count drift")
    aggregate = hashlib.sha256()
    for relative in sorted(entries):
        aggregate.update(relative.encode())
        aggregate.update(b"\0")
        aggregate.update(str(entries[relative].get("sha256", "")).encode())
        aggregate.update(b"\n")
    check(document.get("aggregate_sha256") == aggregate.hexdigest(), "repository manifest aggregate hash drift")

def validate_site() -> None:
    index = ROOT / "site" / "index.html"
    if not index.is_file():
        return
    document = html.fromstring(index.read_text(encoding="utf-8"))
    ids = document.xpath("//@id")
    check(len(ids) == len(set(ids)), "site has duplicate HTML ids")
    id_set = set(ids)
    for href in document.xpath("//@href"):
        if href.startswith("#"):
            check(href[1:] in id_set, f"site fragment target is missing: {href}")
        elif re.match(r"^(?:https?:|mailto:|tel:|data:)", href):
            continue
        else:
            target = (ROOT / "site" / href.split("#", 1)[0]).resolve()
            check(target.is_file(), f"site link target is missing: {href}")
    for source in document.xpath("//@src"):
        if re.match(r"^(?:https?:|data:)", source):
            continue
        target = (ROOT / "site" / source).resolve()
        check(target.is_file(), f"site asset is missing: {source}")
    title = "".join(document.xpath("/html/head/title/text()")).strip()
    check(title == "Mechanism Interferometry", "unexpected site title")
    remote = re.findall(r"(?:src|href)=[\"']https?://", index.read_text(encoding="utf-8"))
    check(not remote, "static site contains a remote dependency")
    node = shutil_which("node")
    if node:
        result = subprocess.run([node, "--check", str(ROOT / "site" / "app.js")], capture_output=True, text=True, check=False)
        check(result.returncode == 0, f"site JavaScript syntax failed: {result.stderr.strip()}")


def validate_paper() -> None:
    paper = ROOT / "paper" / "main.pdf"
    site_copy = ROOT / "site" / "mechanism_interferometry.pdf"
    if paper.is_file() and site_copy.is_file():
        check(sha256(paper) == sha256(site_copy), "website paper copy differs from paper/main.pdf")
    pdfinfo = shutil_which("pdfinfo")
    if pdfinfo and paper.is_file():
        result = subprocess.run([pdfinfo, str(paper)], capture_output=True, text=True, check=False)
        check(result.returncode == 0, f"pdfinfo failed: {result.stderr.strip()}")
        match = re.search(r"^Pages:\s+(\d+)$", result.stdout, re.MULTILINE)
        check(match is not None and int(match.group(1)) >= 30, "paper PDF is unexpectedly short")
    source = (ROOT / "paper" / "main.tex").read_text(encoding="utf-8")
    for token in [
        "Finite modular soft-intervention certificate",
        "curvature-balance",
        "Nested state expansion",
        "Generalised Covariance Measure",
        "Partial factorial designs",
        "Representation learning",
    ]:
        check(token.lower() in source.lower(), f"paper source is missing core section/token: {token}")
    log = ROOT / "paper" / "main.log"
    if log.is_file():
        text = log.read_text(encoding="utf-8", errors="replace")
        check("undefined references" not in text.lower(), "paper build has undefined references")
        check("citation" not in "\n".join(line.lower() for line in text.splitlines() if "undefined" in line.lower()), "paper build has undefined citations")


def validate_proposal_boundary() -> None:
    path = ROOT / "docs" / "PROPOSAL_ADAPTERS.md"
    if not path.is_file():
        return
    source = path.read_text(encoding="utf-8")
    for token in [
        "It may decide what to test next. It may not decide what is true.",
        "diagnostic_only",
        "may not break a `MULTIPLE_PASSES`",
        "must preserve the asserted primitive target",
        "product-odds rule",
        "mic_proposal::rank_active_tilts",
        "proposal_only",
    ]:
        check(token in source, f"proposal-adapter contract is missing boundary token: {token}")


def validate_simulations() -> None:
    results = load_json(ROOT / "artifacts" / "simulations" / "exact_results.json")
    if not isinstance(results, dict):
        return
    running = results.get("running_example", {})
    latent = results.get("latent_conservation", {})
    implementation = results.get("implementation_inconsistency", {})
    parity = results.get("parity_orientation_failure", {})
    check(math.isclose(float(running.get("outcome_synergy", math.nan)), 0.3, abs_tol=1e-14), "running-example synergy drift")
    check(math.isclose(float(running.get("full_state_curvature", math.nan)), 0.0, abs_tol=1e-14), "running-example full curvature drift")
    check(math.isclose(float(latent.get("cov_observed_ratios", math.nan)), -0.09, abs_tol=1e-14), "latent observable covariance drift")
    check(math.isclose(float(latent.get("mean_conditional_latent_covariance", math.nan)), 0.09, abs_tol=1e-14), "latent hidden covariance drift")
    check(math.isclose(float(implementation.get("normalizer", math.nan)), 1.063, abs_tol=1e-14), "implementation normalizer drift")
    check(int(parity.get("pass_count", -1)) == 2, "parity pass-count drift")
    for figure in ["running_example", "latent_conservation", "implementation_inconsistency"]:
        check((ROOT / "paper" / "figures" / f"{figure}.pdf").is_file(), f"missing PDF figure {figure}")
        check((ROOT / "paper" / "figures" / f"{figure}.png").is_file(), f"missing PNG figure {figure}")


def validate_cargo_and_sources() -> None:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    members = cargo.get("workspace", {}).get("members", [])
    for member in members:
        member_path = ROOT / member
        check((member_path / "Cargo.toml").is_file(), f"workspace member lacks Cargo.toml: {member}")
        source_files = sorted((member_path / "src").glob("*.rs"))
        check(bool(source_files), f"workspace member has no Rust source: {member}")
        for source in source_files:
            text = source.read_text(encoding="utf-8")
            check(text.startswith("#![forbid(unsafe_code)]"), f"{source.relative_to(ROOT)} does not forbid unsafe code")
            check(re.search(r"\bunsafe\s*\{", text) is None, f"unsafe block in {source.relative_to(ROOT)}")
    dependency_text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    integration_text = (ROOT / "docs" / "FRANKEN_INTEGRATION.md").read_text(encoding="utf-8")
    revisions = {
        "frankenpandas": "a9f8d86c9e52923b9b2082d00a65841862d5ca9a",
        "franken_numpy": "6964e776528f1e492620ebd627d78d4f958220f4",
        "frankenscipy": "e259ed002eec05a2eca08d38a0763e0e58b0623c",
        "frankentorch": "5a3a0e70a2854c08e42ae02d816a78b8f88d912d",
    }
    for name, revision in revisions.items():
        check(revision in dependency_text, f"{name} revision missing from Cargo.toml")
        check(revision in integration_text, f"{name} revision missing from FRANKEN_INTEGRATION.md")


def validate_shell_scripts() -> None:
    for path in sorted((ROOT / "scripts").glob("*.sh")):
        check(os.access(path, os.X_OK), f"shell script is not executable: {path.relative_to(ROOT)}")
        bash = shutil_which("bash")
        if bash:
            result = subprocess.run([bash, "-n", str(path)], capture_output=True, text=True, check=False)
            check(result.returncode == 0, f"shell syntax failed for {path.name}: {result.stderr.strip()}")


def validate_no_placeholders() -> None:
    pattern = re.compile(r"\b(?:TODO|TBD|FIXME|XXX)\b|Rest of your code|Previous code")
    ignored_suffixes = {".pdf", ".png", ".csv"}
    ignored_names = {"main.log", "main.aux", "main.bbl", "main.bcf", "main.blg", "main.fls", "main.out", "main.run.xml", "main.toc", "main.fdb_latexmk"}
    for path in ROOT.rglob("*"):
        relative_parts = path.relative_to(ROOT).parts
        if any(part in IGNORED_DIRECTORIES for part in relative_parts):
            continue
        if not path.is_file() or path == Path(__file__).resolve() or path.suffix in ignored_suffixes or path.name in ignored_names:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        match = pattern.search(text)
        check(match is None, f"placeholder token {match.group(0)!r} in {path.relative_to(ROOT)}" if match else "")


def shutil_which(command: str) -> str | None:
    for directory in os.environ.get("PATH", "").split(os.pathsep):
        candidate = Path(directory) / command
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


def main() -> int:
    required_files()
    validate_schemas_and_manifests()
    validate_repository_manifest()
    validate_site()
    validate_paper()
    validate_proposal_boundary()
    validate_simulations()
    validate_cargo_and_sources()
    validate_shell_scripts()
    validate_no_placeholders()
    if ERRORS:
        print(f"repository validation failed with {len(ERRORS)} error(s):", file=sys.stderr)
        for error in ERRORS:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print(f"repository validation passed ({CHECKS} checks)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
