#!/usr/bin/env python3
"""Shared helpers for the public-extract campaign. Stdlib only."""

from __future__ import annotations

import hashlib
import json
import subprocess
import time
import urllib.request
from pathlib import Path

ROOT = Path("/Users/jemanuel/brennerbot_sessions/mic-public-extracts")
RAW = ROOT / "raw"
TABLES = ROOT / "tables"
REPORTS = ROOT / "reports"
RECEIPTS = ROOT / "receipts"
LOGS = ROOT / "logs"
REPO = Path("/Users/jemanuel/projects/mechanism_interferometry_causality")
TABULAR = Path("/tmp/mic-beige-target/debug/mic-tabular")
SEED = 20260813
RETRIEVED = "2026-08-13T00:00:00Z"

for path in (RAW, TABLES, REPORTS, RECEIPTS, LOGS):
    path.mkdir(parents=True, exist_ok=True)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def md5_file(path: Path) -> str:
    digest = hashlib.md5()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def download(url: str, dest: Path, timeout: int = 180) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "MIC-public-extract/0.1 (research; BeigeGorge)"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as response:
        dest.write_bytes(response.read())


def write_receipt(name: str, payload: dict) -> Path:
    payload = {
        "schema_version": "extract_receipt.v1",
        "dataset": name,
        "retrieved_at": RETRIEVED,
        "seed": SEED,
        "authority": payload.get("authority", "proposal_only"),
        **payload,
    }
    path = RECEIPTS / f"{name}.json"
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    return path


def run_survey(table: Path, cluster: str, report: Path) -> dict:
    report.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        str(TABULAR),
        "survey",
        str(table),
        "--cluster",
        cluster,
        "--output",
        str(report),
    ]
    completed = subprocess.run(cmd, check=False, capture_output=True, text=True)
    if completed.returncode != 0:
        return {
            "ok": False,
            "returncode": completed.returncode,
            "stderr": completed.stderr[-4000:],
            "stdout": completed.stdout[-2000:],
        }
    return {"ok": True, "report": str(report)}


def summarize_survey(report_path: Path) -> dict:
    data = json.loads(report_path.read_text())
    squares = [
        {
            "id": item.get("interferometer_id"),
            "complete": item.get("complete_square"),
            "missing": item.get("missing_corners"),
            "dropped": item.get("dropped_corners"),
            "min_corner": item.get("min_corner_count"),
            "product": item.get("empirically_product"),
        }
        for item in data.get("interferometers", [])
    ]
    info = data.get("information_content") or {}
    return {
        "n_rows": data.get("n_rows"),
        "authority": data.get("authority"),
        "cluster_unit_basis": data.get("cluster_unit_basis"),
        "n_independent_units": info.get("n_independent_units"),
        "n_complete_testable_squares": info.get("n_complete_testable_squares"),
        "n_distinct_supported_regimes": info.get("n_distinct_supported_regimes"),
        "units_per_corner_min": info.get("confirmatory_units_per_corner_min"),
        "units_per_corner_max": info.get("confirmatory_units_per_corner_max"),
        "recommended_next_corner": info.get("recommended_next_corner"),
        "identified_set_dimension": info.get("identified_set_dimension"),
        "recommended_next_corner_cost": info.get("recommended_next_corner_cost"),
        "recommended_next_corner_kind": info.get("recommended_next_corner_kind"),
        "ranked_next_corners": info.get("ranked_next_corners") or [],
        "units_are_rows": info.get("units_are_rows"),
        "confirmatory": info.get("confirmatory"),
        "information_content_note": info.get("note"),
        "n_interferometers": len(squares),
        "complete_squares": [item["id"] for item in squares if item["complete"]],
        "has_direction_scout": "direction_scout" in data and data["direction_scout"] is not None,
        "next_step": (data.get("next_step") or "")[:240],
        "columns": [
            {"column": col["column"], "role": col["role"]} for col in data.get("columns", [])
        ],
        "squares": squares[:8],
    }


def token_hi_lo(value: float, cut: float) -> str:
    return "high" if value >= cut else "low"
