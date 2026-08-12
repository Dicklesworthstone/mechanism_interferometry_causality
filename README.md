# Mechanism Interferometry

**A gauge-invariant soft-intervention certificate for causal modularity.**

This repository contains the formal paper, an explanatory static website, exact simulation fixtures, and the architecture of a memory-safe Rust audit system for deciding whether empirical perturbations compose as autonomous causal mechanisms.

The core certificate is

\[
\text{locality}+\text{conditional normalization}+\text{square flatness}
\iff
\text{a modular soft-intervention representation exists}.
\]

For a pair of perturbations,

\[
\kappa_{AB}(x)=\log\frac{p_{AB}(x)p_0(x)}{p_A(x)p_B(x)}
\]

is the gauge-invariant mechanism-curvature field. It separates nonlinear response from actual coupling between mechanism changes and from curvature created by incomplete observation.

## Repository map

| Path | Contents |
|---|---|
| `paper/` | Complete LaTeX paper, bibliography, verified PDF, and figures |
| `site/` | Zero-build static explanatory website with an interactive running example |
| `docs/` | Formal specification, inference protocol, experiment designs, Franken* integration, and implementation roadmap |
| `crates/` | Safe-Rust workspace for exact algebra, design audits, inference primitives, proposal adapters, models, simulations, and CLI |
| `schemas/` | Machine-readable experiment and audit-report contracts |
| `examples/` | Feature-flag, Perturb-seq, simulation, orientation, and proposal artifacts |
| `scripts/` | Reproducible simulation, paper, site, and repository checks |
| `artifacts/simulations/` | Exact-population fixtures used by the paper and conformance tests |
| `REPOSITORY_MANIFEST.json` | SHA-256 content inventory covering every release-relevant file |

## Read first

- [Paper PDF](paper/main.pdf)
- [Formal specification](docs/FORMAL_SPEC.md)
- [Implementation blueprint](docs/IMPLEMENTATION_BLUEPRINT.md)
- [Inference protocol](docs/INFERENCE_PROTOCOL.md)
- [Proposal-adapter boundary](docs/PROPOSAL_ADAPTERS.md)
- [Franken ecosystem integration](docs/FRANKEN_INTEGRATION.md)
- [Experimental protocol](docs/EXPERIMENTAL_PROTOCOL.md)

## Build and inspect

```bash
./scripts/build_all.sh
./scripts/serve_site.sh
cargo test --workspace --no-default-features
cargo run -p mic-cli -- simulate all --output artifacts/simulations/rust_exact_results.json
cargo run -p mic-cli -- design audit examples/configs/feature_flag_pilot.json
cargo run -p mic-cli -- preflight examples/configs/feature_flag_pilot.json
cargo run -p mic-cli -- orient examples/orientation/parity_demo.json
cargo run -p mic-cli -- propose-tilt examples/proposal_inputs/parity_active_tilt.json

# After committing the repository, produce source, website, and git-bundle releases.
./scripts/package_release.sh ./dist

# Create or update the GitHub repository after gh authentication.
# Second argument selects visibility and defaults to public.
./scripts/publish_repo.sh Dicklesworthstone/mechanism_interferometry_causality public
```

The Rust workspace is pinned to `nightly-2026-07-20`, using the newest explicit toolchain pin among the four reviewed Franken* revisions recorded in [`docs/FRANKEN_INTEGRATION.md`](docs/FRANKEN_INTEGRATION.md).

## Statistical contract

The software has two inference modes and never silently substitutes one for the other:

1. **Four-law mode** estimates functionals of the normalized regime laws and permits arbitrary known, state-independent corner quotas.
2. **Product-factorial mode** uses GCM/wGCM residual products and requires product assignment odds or explicit reweighting to a product design. Every reference GCM result carries a self-validating serialized evidence grade; unverified calculations are permanently marked `diagnostic_only`.

Within-regime state-dependent selection invalidates both modes unless modeled. Strict mode fails closed on this and on inadequate overlap, unresolved deletion orientation, non-product GCM sampling, unidentifiable design contrasts, or inconsistent randomization units. The executable orientation path also rejects pointwise interval collections, mixed equivalence tolerances, or missing interval-method, randomization-unit, source-fingerprint, and seed provenance.

## Current implementation status

The repository includes the complete mathematical paper, website, exact simulation generators, schemas, runnable example datasets, architectural contracts, and a safe-Rust reference core implementing the exact population algebra, partial-design geometry, fail-closed preflight, and deterministic audit primitives. The production estimators that depend on the evolving Franken* numerical APIs are isolated behind feature-gated adapters and specified packet-by-packet in the roadmap. This keeps the mathematical contracts stable while allowing the four sibling projects to advance without contaminating the causal API.

Passive DAG learners, parsimony searches, residual heuristics, and previous audit runs may be connected only as proposal adapters. They can prioritize candidate supports, measurements, or follow-up interventions, but their scores never count as certificate evidence and data-adaptive proposals require independent confirmation.

The `mic-proposal` crate now implements that boundary for active follow-up design. Given a multiple-pass orientation state, it validates a same-primitive candidate-tilt library, rejects candidates without delivery, common support, or the planned Product-Factorial design evidence, and ranks the remainder by worst-case predicted hypothesis separation. Every result is serialized with `authority: proposal_only`, an explicit recommendation-or-abstention status, a SHA-256 fingerprint of the full ordered candidate library, complete adapter provenance, a deterministic seed, the frozen tie rule, and explicit rejection reasons. The `mic propose-tilt` command turns the checked input contract into that deterministic artifact; it never alters the unresolved orientation verdict.

## Citation

Citation metadata is provided in [`CITATION.cff`](CITATION.cff), and the complete scholarly bibliography is in [`paper/references.bib`](paper/references.bib). The individual ingredients are classical or have direct precedents; the claimed contribution is the assembled biconditional certificate, its conservation laws, normalization orientation, and the integrated inferential protocol.
