#!/usr/bin/env python3
"""Receipt S1–S3 in-repo atlas fixtures. No download."""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import REPO, REPORTS, TABLES, run_survey, sha256_file, summarize_survey, write_receipt

FIXTURES = {
    "s2a_season_trap": REPO / "examples/data/s2a_season_trap.csv",
    "s2b_coordinated": REPO / "examples/data/s2b_coordinated.csv",
    "s3_vir_mirror": REPO / "examples/data/s3_vir_mirror.csv",
}


def main() -> None:
    for name, src in FIXTURES.items():
        dest = TABLES / f"{name}.csv"
        dest.write_bytes(src.read_bytes())
        report = REPORTS / f"{name}.json"
        result = run_survey(dest, "cluster_id", report)
        summary = summarize_survey(report) if result.get("ok") else result
        write_receipt(
            name,
            {
                "status": "surveyed" if result.get("ok") else "blocked",
                "source_url": f"in-repo:{src.relative_to(REPO)}",
                "license": "repository LICENSE (do not treat as public-domain data)",
                "byte_sha256": sha256_file(src),
                "cluster_unit": "cluster_id",
                "ground_truth_authority": "structural construction of the fixture",
                "allowed_answers": ["complete_square", "proposal_only", "no_direction_scout"],
                "first_falsifier": "emitting unique_target, direction_scout, or certificate_status",
                "note": "Atlas-only world. Not a causal win.",
                "survey": summary,
            },
        )
        print(name, "ok" if result.get("ok") else result)


if __name__ == "__main__":
    main()
