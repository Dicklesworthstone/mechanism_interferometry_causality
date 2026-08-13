#!/usr/bin/env python3
from __future__ import annotations

import csv
import json
import math
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
FIG = ROOT / "paper" / "figures"
ART = ROOT / "artifacts" / "simulations"
FIG.mkdir(parents=True, exist_ok=True)
ART.mkdir(parents=True, exist_ok=True)


def running_example(a: float = 0.6, b: float = 0.5, sigma: float = 0.8) -> dict:
    y = np.linspace(-4.0, 4.0, 801)
    kappa = np.log1p(a * b * np.tanh(y / sigma**2))
    ratio_ab = np.exp(kappa)
    rows = list(zip(y.tolist(), kappa.tolist(), ratio_ab.tolist(), strict=True))
    with (ART / "running_example_curve.csv").open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["y", "kappa_y", "r_ab_y"])
        w.writerows(rows)

    fig, ax = plt.subplots(figsize=(7.2, 4.3))
    ax.plot(y, kappa, label=r"$\kappa_{AB}(y)$")
    ax.axhline(0.0, linewidth=1)
    ax.set_xlabel(r"Observed outcome $y$")
    ax.set_ylabel(r"Marginal curvature $\kappa_{AB}(y)$")
    ax.set_title("Autonomous source mechanisms become curved after retaining only Y")
    ax.legend()
    ax.grid(alpha=0.25)
    fig.tight_layout()
    fig.savefig(FIG / "running_example.pdf", bbox_inches="tight", metadata={"CreationDate": None, "ModDate": None, "Creator": "Mechanism Interferometry"})
    fig.savefig(FIG / "running_example.png", dpi=180, bbox_inches="tight")
    plt.close(fig)

    return {
        "a": a,
        "b": b,
        "sigma": sigma,
        "outcome_synergy": a * b,
        "full_state_curvature": 0.0,
        "marginal_curvature_at_positive_infinity": math.log1p(a * b),
        "marginal_curvature_at_negative_infinity": math.log1p(-a * b),
        "primitive_y_ratios": {"r_A": 1.0, "r_B": 1.0},
        "scalar_moment_battery": 1.0,
    }


def latent_conservation(a: float = 0.3) -> dict:
    x = np.array([-1.0, 1.0])
    r_a = 1.0 + a * x
    r_b = 1.0 - a * x
    covariance_terms = (r_a - 1.0) * (r_b - 1.0)
    conditional_latent_covariance = np.full_like(x, a**2)
    kappa = -math.log1p(-a**2)

    fig, ax = plt.subplots(figsize=(7.2, 4.3))
    positions = np.arange(2)
    width = 0.34
    ax.bar(positions - width / 2, covariance_terms, width, label=r"$(r_A-1)(r_B-1)$")
    ax.bar(positions + width / 2, conditional_latent_covariance, width, label=r"$\mathrm{Cov}(L_A,L_B\mid X)$")
    ax.axhline(0.0, linewidth=1)
    ax.set_xticks(positions, [r"$X=-1$", r"$X=+1$"])
    ax.set_ylabel("Contribution")
    ax.set_title("Hidden conditional coupling is balanced by observed ratio anticorrelation")
    ax.legend()
    ax.grid(axis="y", alpha=0.25)
    fig.tight_layout()
    fig.savefig(FIG / "latent_conservation.pdf", bbox_inches="tight", metadata={"CreationDate": None, "ModDate": None, "Creator": "Mechanism Interferometry"})
    fig.savefig(FIG / "latent_conservation.png", dpi=180, bbox_inches="tight")
    plt.close(fig)

    return {
        "a": a,
        "r_A": r_a.tolist(),
        "r_B": r_b.tolist(),
        "r_AB": [1.0, 1.0],
        "kappa": kappa,
        "cov_observed_ratios": -(a**2),
        "mean_conditional_latent_covariance": a**2,
        "single_mechanism_locality": False,
        "interpretation": "coordinated multi-source tilts that compose flatly",
    }


def implementation_inconsistency(a: float = 0.45, b: float = 0.35, gamma: float = 0.4) -> dict:
    products = np.array([-1.0, 1.0])
    kappa = np.log1p(gamma * products) - math.log1p(a * b * gamma)

    fig, ax = plt.subplots(figsize=(7.2, 4.3))
    ax.bar([0, 1], kappa)
    ax.axhline(0.0, linewidth=1)
    ax.set_xticks([0, 1], [r"$X_1X_2=-1$", r"$X_1X_2=+1$"])
    ax.set_ylabel(r"$\kappa_{AB}$")
    ax.set_title("Combination-specific implementation effects create curvature")
    ax.grid(axis="y", alpha=0.25)
    fig.tight_layout()
    fig.savefig(FIG / "implementation_inconsistency.pdf", bbox_inches="tight", metadata={"CreationDate": None, "ModDate": None, "Creator": "Mechanism Interferometry"})
    fig.savefig(FIG / "implementation_inconsistency.png", dpi=180, bbox_inches="tight")
    plt.close(fig)

    return {
        "a": a,
        "b": b,
        "gamma": gamma,
        "normalizer": 1.0 + a * b * gamma,
        "kappa_product_minus_one": float(kappa[0]),
        "kappa_product_plus_one": float(kappa[1]),
        "negative_control_gamma": 0.0,
        "negative_control_curvature": 0.0,
    }


def parity_example(epsilon: float = 0.1) -> dict:
    return {
        "epsilon": epsilon,
        "baseline": "T=P xor N",
        "intervention": "T=(not P) xor N",
        "support": ["P", "T"],
        "invariant_deletions": ["P", "T"],
        "pass_count": 2,
        "conclusion": "normalization-faithfulness fails under balanced parity symmetry",
    }


def hidden_sensor_tomography(a: float = 0.4, b: float = 0.5) -> dict:
    states = [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)]
    regimes: dict[str, dict[str, list[float]]] = {}
    for tilt_a, tilt_b in [(False, False), (True, False), (False, True), (True, True)]:
        complete = np.array(
            [
                0.25
                * (1.0 + a * u if tilt_a else 1.0)
                * (1.0 + b * v if tilt_b else 1.0)
                for u, v in states
            ]
        )
        observed = np.array([complete[1] + complete[2], complete[0] + complete[3]])
        regimes[f"{int(tilt_a)}{int(tilt_b)}"] = {
            "complete": complete.tolist(),
            "observed_y": observed.tolist(),
        }

    curvature = np.log(np.array([1.0 - a * b, 1.0 + a * b]))
    rows = [
        ["observed_y", -1, float(curvature[0])],
        ["observed_y", 1, float(curvature[1])],
        ["reveal_hidden_u", -1, 0.0],
        ["reveal_hidden_u", 1, 0.0],
        ["add_independent_noise", -1, float(curvature[0])],
        ["add_independent_noise", 1, float(curvature[1])],
    ]
    with (ART / "hidden_sensor_tomography.csv").open("w", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["representation", "observed_y", "curvature"])
        writer.writerows(rows)

    positions = np.arange(2)
    width = 0.24
    fig, ax = plt.subplots(figsize=(7.2, 4.3))
    ax.bar(positions - width, curvature, width, label="Observe Y only")
    ax.bar(positions, [0.0, 0.0], width, label="Reveal hidden U")
    ax.bar(positions + width, curvature, width, label="Add irrelevant noise")
    ax.axhline(0.0, linewidth=1, color="black")
    ax.set_xticks(positions, [r"$Y=-1$", r"$Y=+1$"])
    ax.set_ylabel(r"Curvature $\kappa_{AB}$")
    ax.set_title("The resolving sensor removes curvature; an irrelevant sensor does not")
    ax.legend()
    ax.grid(axis="y", alpha=0.25)
    fig.tight_layout()
    fig.savefig(
        FIG / "hidden_sensor_tomography.pdf",
        bbox_inches="tight",
        metadata={
            "CreationDate": None,
            "ModDate": None,
            "Creator": "Mechanism Interferometry",
        },
    )
    fig.savefig(FIG / "hidden_sensor_tomography.png", dpi=180, bbox_inches="tight")
    plt.close(fig)

    return {
        "a": a,
        "b": b,
        "state_order": ["--", "-+", "+-", "++"],
        "regimes": regimes,
        "observed_curvature": curvature.tolist(),
        "complete_max_abs_curvature": 0.0,
        "infinitesimal_score_covariance": [
            [1.0, -1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        ],
        "infinitesimal_missing_rank": [1, 1],
        "candidate_measurements": [
            {"sensor": "hidden_u", "resolves_curvature": True},
            {"sensor": "independent_noise", "resolves_curvature": False},
        ],
        "rank_scope": (
            "rank-one conditional covariance of infinitesimal scores; "
            "not a PSD claim about finite Boolean curvature"
        ),
    }


def main() -> None:
    result = {
        "running_example": running_example(),
        "parity_orientation_failure": parity_example(),
        "latent_conservation": latent_conservation(),
        "implementation_inconsistency": implementation_inconsistency(),
        "hidden_sensor_tomography": hidden_sensor_tomography(),
    }
    with (ART / "exact_results.json").open("w") as f:
        json.dump(result, f, indent=2, sort_keys=True)
        f.write("\n")
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
