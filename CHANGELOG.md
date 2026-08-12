# Changelog

## Unreleased

- Added the MDL/functional-sparsity positioning, active tilt selection, estimator-family agreement, and parsimony-frontier localization sections to the paper, with new bibliography entries.
- Added the estimator lens-battery sensitivity screen (`mic_engine::audit_lens_battery`) with the `estimator_family_disagreement` reason code and fail-closed handling of degenerate standard errors.
- Added the parsimony-frontier localization primitive (`mic_stats::parsimony_frontier`) with cardinality-first ordering and per-variable descriptive inclusion frequencies.
- Added the deletion-equivalence machinery: tri-state `classify_deletion`, the five-state pass-count machine `orient_from_deletions`, and the deterministic Rademacher multiplier bootstrap `simultaneous_mean_bounds`.
- Added `mic_engine::audit_orientation` and `mic_engine::audit_overlap`, activating the reserved `orientation_unresolved` and `overlap_failure` reason codes.
- Added the `mic orient` CLI subcommand and the `examples/orientation/parity_demo.json` walkthrough.
- Derived the parity fixture's pass count from exact deletion marginals instead of asserting it.
- Added the proposal-layer boundary documents and the quarantined active-tilt proposal contract, including `mic propose-tilt`, input/output schemas, a derived candidate-library fingerprint, deterministic tie-policy provenance, and explicit no-eligible-candidate abstention.
- Fixed the evidence-ledger digest for sha2 0.11, which no longer implements hexadecimal formatting on digest arrays.

## 0.1.0 — 2026-08-12

- Added the complete formal paper and verified 34-page PDF.
- Added the zero-build explanatory website and interactive running example.
- Added exact simulation fixtures for marginal curvature, parity ambiguity, latent conservation, and implementation inconsistency.
- Added a safe-Rust workspace for exact algebra, partial-design geometry, manifests, inference primitives, posterior-odds reconstruction, audit ledgers, preflight orchestration, simulations, CLI, and conformance.
- Added pinned integration architecture for FrankenPandas, FrankenNumPy, FrankenSciPy, and FrankenTorch.
- Added schemas, deterministic datasets, example manifests, CI, and repository integrity validation.
