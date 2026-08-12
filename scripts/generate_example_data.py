#!/usr/bin/env python3
from __future__ import annotations

import csv
import math
import random
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "examples" / "data"
DATA.mkdir(parents=True, exist_ok=True)


def write_csv(name: str, fieldnames: list[str], rows: list[dict[str, object]]) -> None:
    with (DATA / name).open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)


def running_example() -> None:
    rng = random.Random(20260812)
    a, b, sigma = 0.6, 0.5, 0.8
    rows: list[dict[str, object]] = []
    for regime, active_a, active_b in [("00", 0, 0), ("10", 1, 0), ("01", 0, 1), ("11", 1, 1)]:
        p1 = (1 + a * active_a) / 2
        p2 = (1 + b * active_b) / 2
        for index in range(128):
            x1 = 1 if rng.random() < p1 else -1
            x2 = 1 if rng.random() < p2 else -1
            y = x1 * x2 + rng.gauss(0, sigma)
            rows.append({
                "cluster_id": f"{regime}-{index // 8:02d}",
                "regime": regime,
                "x1": x1,
                "x2": x2,
                "y": f"{y:.12f}",
                "included": 1,
            })
    write_csv("running_example.csv", ["cluster_id", "regime", "x1", "x2", "y", "included"], rows)


def feature_flags() -> None:
    rng = random.Random(19051988)
    rows: list[dict[str, object]] = []
    for regime, flag_a, flag_b in [("00", 0, 0), ("10", 1, 0), ("01", 0, 1), ("11", 1, 1)]:
        for unit in range(48):
            demand = max(0.0, rng.gauss(70.0, 14.0))
            shared_pressure = max(0.0, 0.45 + 0.006 * demand + rng.gauss(0.0, 0.05))
            module_a_output = demand * (1.10 if flag_a else 1.0) + rng.gauss(0.0, 2.0)
            module_b_output = math.sqrt(demand + 1.0) * (0.92 if flag_b else 1.0) + rng.gauss(0.0, 0.2)
            queue_depth = max(0.0, demand - 74.0 - 8.0 * flag_a + rng.gauss(0.0, 3.0))
            latency = 28.0 + 0.35 * queue_depth + 11.0 * shared_pressure + rng.gauss(0.0, 1.2)
            rows.append({
                "deployment_id": f"dep-{regime}-{unit:03d}",
                "regime": regime,
                "flag_a": flag_a,
                "flag_b": flag_b,
                "demand": f"{demand:.9f}",
                "module_a_output": f"{module_a_output:.9f}",
                "module_b_output": f"{module_b_output:.9f}",
                "queue_depth": f"{queue_depth:.9f}",
                "shared_resource_pressure": f"{shared_pressure:.9f}",
                "p95_latency_ms": f"{latency:.9f}",
                "included": 1,
            })
    write_csv(
        "feature_flag_pilot.csv",
        [
            "deployment_id", "regime", "flag_a", "flag_b", "demand",
            "module_a_output", "module_b_output", "queue_depth",
            "shared_resource_pressure", "p95_latency_ms", "included",
        ],
        rows,
    )


def perturbseq() -> None:
    rng = random.Random(31415926)
    rows: list[dict[str, object]] = []
    for regime, gene_a, gene_b in [("00", 0, 0), ("10", 1, 0), ("01", 0, 1), ("11", 1, 1)]:
        for replicate in range(4):
            guide_efficiency = min(0.99, max(0.55, rng.gauss(0.84, 0.05)))
            for cell in range(40):
                latent_state = rng.gauss(0.0, 1.0)
                g1 = 2.0 + 1.2 * gene_a * guide_efficiency + 0.4 * latent_state + rng.gauss(0.0, 0.35)
                g2 = 1.5 - 0.9 * gene_b * guide_efficiency + 0.3 * latent_state + rng.gauss(0.0, 0.30)
                g3 = 1.0 + 0.7 * gene_a * gene_b * guide_efficiency + 0.5 * latent_state + rng.gauss(0.0, 0.40)
                g4 = 2.4 + 0.2 * gene_a - 0.3 * gene_b + rng.gauss(0.0, 0.25)
                rows.append({
                    "replicate_id": f"rep-{replicate + 1}",
                    "cell_id": f"{regime}-{replicate + 1}-{cell:03d}",
                    "regime": regime,
                    "gene_a_perturbed": gene_a,
                    "gene_b_perturbed": gene_b,
                    "guide_efficiency": f"{guide_efficiency:.9f}",
                    "latent_cell_state_proxy": f"{latent_state:.9f}",
                    "expr_g1": f"{g1:.9f}",
                    "expr_g2": f"{g2:.9f}",
                    "expr_g3": f"{g3:.9f}",
                    "expr_g4": f"{g4:.9f}",
                    "included": 1,
                })
    write_csv(
        "perturbseq_pair.csv",
        [
            "replicate_id", "cell_id", "regime", "gene_a_perturbed",
            "gene_b_perturbed", "guide_efficiency", "latent_cell_state_proxy",
            "expr_g1", "expr_g2", "expr_g3", "expr_g4", "included",
        ],
        rows,
    )


def main() -> None:
    running_example()
    feature_flags()
    perturbseq()
    print(f"wrote example datasets to {DATA}")


if __name__ == "__main__":
    main()
