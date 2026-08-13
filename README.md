# Mechanism Interferometry

**Auditing the compositionality of soft interventions.**

This repository contains the formal paper, an explanatory website, exact simulation fixtures, and the architecture of a memory-safe Rust audit system for deciding whether empirical perturbations compose as autonomous causal mechanisms.

**Live site: <https://mechanism-interferometry.pages.dev>** — the interactive figures there are not illustrations. The design auditor, preflight gate, square-face geometry, interaction-aliasing check and estimator lens battery call the actual `mic` Rust engine compiled to WebAssembly, so a refusal you see in the browser is the same refusal the CLI produces, from the same code.

On a complete common-support factorial cube, relative to a proposed DAG and
distinct target assignment, the core certificate is

\[
\text{locality}+\text{conditional normalization}+\text{square flatness}
\iff
\text{a modular soft-intervention representation exists}.
\]

For a pair of perturbations,

\[
\kappa_{AB}(x)=\log\frac{p_{AB}(x)p_0(x)}{p_A(x)p_B(x)}
\]

is the gauge-invariant mechanism-curvature field. It separates outcome-scale
nonlinearity from failure of closure at the chosen state representation.
Curvature alone does not distinguish genuine mechanism coupling from omitted
state; state expansion, intervention metadata, and implementation controls are
the discriminative follow-up evidence.

## Repository map

| Path | Contents |
|---|---|
| `paper/` | Complete LaTeX paper, bibliography, verified PDF, and figures |
| `site/` | Static explanatory website; interactive widgets call `mic-wasm`, the engine compiled to WebAssembly |
| `docs/` | Formal specification, inference protocol, experiment designs, Franken* integration, and implementation roadmap |
| `crates/` | Safe-Rust workspace for exact algebra, design audits, inference primitives, proposal adapters, models, simulations, CLI, and the `mic-wasm` browser boundary |
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
- [Autonomous survey boundary](docs/AUTONOMOUS_MODE.md)
- [Causal mechanism tomography](docs/CAUSAL_TOMOGRAPHY.md)
- [Self-driving campaign ledger](docs/SELF_DRIVING_CAMPAIGN.md)
- [Public causal dataset gauntlet](docs/DATASET_GAUNTLET.md)
- [Public-dataset eligibility map](docs/DATASET_ELIGIBILITY.md)
- [Franken ecosystem integration](docs/FRANKEN_INTEGRATION.md)
- [Experimental protocol](docs/EXPERIMENTAL_PROTOCOL.md)

## Build and inspect

```bash
./scripts/build_all.sh
./scripts/serve_site.sh
cargo test --workspace --no-default-features
cargo run -p mic-cli -- simulate all --output artifacts/simulations/rust_exact_results.json
cargo run -p mic-cli -- simulate hidden-sensor
cargo run -p mic-cli -- design audit examples/configs/feature_flag_pilot.json
cargo run -p mic-cli -- preflight examples/configs/feature_flag_pilot.json
cargo run -p mic-cli -- orient examples/orientation/parity_demo.json
cargo run -p mic-cli -- propose-tilt examples/proposal_inputs/parity_active_tilt.json
cargo run -p mic-cli -- freeze-scout examples/scout_inputs/self_driving_request.json examples/scout_inputs/shift_factorization_draft.json
cargo run -p mic-engine --bin mic-tabular -- report examples/configs/four_law_discrete.json --base-dir .
cargo run -p mic-engine --bin mic-tabular -- survey examples/data/four_law_discrete.csv --cluster cluster_id --base-dir .

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
2. **Product-factorial mode** uses GCM/wGCM residual products and requires product assignment odds or explicit reweighting to a product design. Every reference GCM result carries serialized references to a completed audit identifier and SHA-256 source fingerprint; those fields have no certificate authority until the engine resolves them against the evidence ledger and analyzed artifact. Unverified calculations are permanently marked `diagnostic_only`.

Within-regime state-dependent selection invalidates both modes unless modeled. Strict mode fails closed on this and on inadequate overlap, unresolved deletion orientation, non-product GCM sampling, unidentifiable design contrasts, or a manifest-declared cluster identifier that spans regimes. Rows alone do not prove that the declared cluster column is the true assignment unit—a unique row identifier can still masquerade as one—so assignment-unit basis evidence and content binding remain explicit Packet 0A work rather than a current certificate claim. The executable orientation path also rejects pointwise interval collections, mixed equivalence tolerances, or missing interval-method, randomization-unit, source-fingerprint, and seed provenance.

## Current implementation status

The design layer now covers finite categorical mechanism families as well as
Boolean squares. It treatment-codes alternative family levels, enumerates every
observed cross-family rectangle, reports algebraic identified-set and lack-of-fit
dimensions separately, and never promotes geometry alone to a unique causal
completion.

The repository includes the complete mathematical paper, website, exact simulation generators, schemas, runnable example datasets, architectural contracts, and a safe-Rust reference core implementing the exact population algebra, partial-design geometry, fail-closed preflight, and deterministic audit primitives. A standard-library CSV path now produces cluster-weighted histogram four-law diagnostics, and a proposal-only survey inventories candidate squares without claiming selection, assignment, or orientation. Neither is the production FrankenPandas estimator stack: the histogram path never issues a passed certificate, and an autonomous survey can only recommend the next declared audit. Final status is derived from an opaque typed gate summary rather than a caller Boolean; only the deliberately unresolved constructor is public until content-bound locality, normalization, flatness, and orientation producers exist. The production estimators that depend on the evolving Franken* numerical APIs remain isolated behind feature-gated adapters and specified packet-by-packet in the roadmap. This keeps the mathematical contracts stable while allowing the four sibling projects to advance without contaminating the causal API.

Passive DAG learners, parsimony searches, residual heuristics, and previous audit runs may be connected only as proposal adapters. They can prioritize candidate supports, measurements, or follow-up interventions, but their scores never count as certificate evidence and data-adaptive proposals require independent confirmation.

The longer-term self-driving campaign is **causal mechanism tomography**, not a
generic DAG fit to one table. It treats a rich collection of environments as an
algebra of distribution changes: localize recurring log-law transports, group
candidate mechanisms, use explicit identification strategies to orient what can
be oriented, test whether independently inferred changes compose, and use
curvature to rank missing measurements or the next experiment. The exact
three-node tomography cube now provides the first executable conformance world;
the public-data gauntlet spans controlled physics, simulated known graphs,
single-cell interventions, industrial and river propagation, randomized trials,
and mandatory-abstention cases. Every autonomous artifact remains proposal-only
until it is frozen and tested on independent units under the ordinary audit
contracts.

The `mic-proposal` crate now implements that boundary for active follow-up design. Given a multiple-pass orientation state, it validates a same-primitive candidate-tilt library, rejects candidates without delivery, common support, or the planned Product-Factorial design evidence, and ranks the remainder by worst-case predicted hypothesis separation. Every result is serialized with `authority: proposal_only`, an explicit recommendation-or-abstention status, a SHA-256 fingerprint of the full ordered candidate library, complete adapter provenance, a deterministic seed, the frozen tie rule, and explicit rejection reasons. The `mic propose-tilt` command turns the checked input contract into that deterministic artifact; it never alters the unresolved orientation verdict.

The `mic freeze-scout` command freezes a many-environment shift-factorization
proposal. It accepts opaque discovery identifiers, treats partition, unit, and
isolation inputs as unresolved caller claims, derives mandatory blockers, binds
the complete ordered candidate library and request, and cannot deserialize or
convert into certificate gates. It does not read confirmation outcomes or claim
that the supplied isolation facts are true.

## Citation

Citation metadata is provided in [`CITATION.cff`](CITATION.cff), and the complete scholarly bibliography is in [`paper/references.bib`](paper/references.bib). The individual ingredients are classical or have direct precedents; the claimed contribution is the scoped complete-cube biconditional certificate, its conservation laws, equivalence-based orientation protocol under explicit intervention-family premises, and the integrated inferential architecture.
