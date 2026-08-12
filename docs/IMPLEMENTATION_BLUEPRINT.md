# Implementation Blueprint

## 1. Product philosophy

The program is an evidence-producing causal audit system, not a generic modeling framework. It should make invalid analyses difficult to express, make assumptions machine-readable, and fail closed when a certificate condition is statistically unresolved.

The implementation is CPU-first, safe Rust, deterministic by default, and designed to exploit Apple Silicon and high-core AMD systems through the sibling Franken* libraries. Neural components are optional; every core theorem and exact simulation remains runnable without them.

## 2. Workspace architecture

### `mic-core`

Owns immutable mathematical contracts:

- normalized ratios and log ratios;
- pair and arbitrary design contrasts;
- curvature balance and latent conservation;
- nested state expansion;
- self-normalized compositional weights;
- typed numerical and contract errors.

It has no dataframe, neural, or runtime dependency.

### `mic-design`

Owns experiment geometry:

- Boolean design points and face enumeration;
- product-odds verification;
- main-effects design matrices;
- rank, null-space, and alias analysis;
- observed-cycle contrast generation;
- randomization-unit and clustering metadata.

### `mic-data`

Owns validated tables and manifests:

- regime labels and design-bit columns;
- feature blocks and candidate-state groups;
- selection and inclusion fields;
- cluster identifiers and fold assignments;
- CSV, Parquet, Arrow, and JSON interchange through FrankenPandas.

### `mic-stats`

Owns reusable inference primitives:

- cross-fitting and deterministic fold plans;
- GCM/wGCM residual products;
- MMD and energy U-statistics;
- multiplier, permutation, and cluster bootstraps;
- simultaneous equivalence intervals;
- effective sample size and overlap metrics;
- adaptive witness splitting.

### `mic-model`

Owns learned nuisance and diagnostic models:

- calibrated binary and multinomial regime prediction;
- hierarchical main-effect and interaction fields;
- primitive ratio extraction and normalization;
- representation sufficiency probes;
- local adapter traits for exploratory support, graph, measurement, and tilt proposals;
- optional FrankenTorch CPU/Metal backends.

Proposal adapters may order candidate tests but never implement certificate policy. Their serialized outputs use explicit score semantics and flow through the discovery/confirmation boundary in [`PROPOSAL_ADAPTERS.md`](PROPOSAL_ADAPTERS.md). External passive-discovery or residual-asymmetry code cannot become a dependency of `mic-core`.

### `mic-audit`

Owns policy and evidence:

- strict versus exploratory execution modes;
- fail-closed gates;
- provenance ledger and content hashes;
- warnings, abstentions, and reason codes;
- JSON and human-readable report generation.

### `mic-engine`

Owns deterministic orchestration:

- manifest and selection-contract preflight;
- partial-design rank and lack-of-fit audits;
- product-odds gates on every observed face;
- inference-track eligibility and fail-closed status;
- stage DAG construction and report-envelope handoff.

### `mic-sim`

Owns exact and finite-sample scenarios:

- nonlinear complete-versus-marginal example;
- parity multiple-pass failure;
- latent conservation;
- implementation inconsistency and negative controls;
- partial-design alias fixtures.

### `mic-cli`

Owns the user workflow:

```text
mic validate-design manifest.json
mic localize manifest.json
mic orient manifest.json
mic curvature manifest.json
mic conserve manifest.json
mic predict manifest.json
mic expand-state manifest.json
mic simulate all
mic report run-directory
```

### `mic-conformance`

Owns cross-crate golden journeys, exact algebra fixtures, differential tests against a small Python oracle, and failure-mode tests.

## 3. Execution DAG

A run is represented as a deterministic DAG:

```text
manifest -> schema validation -> design audit -> data fingerprint
         -> cluster/fold plan -> optional proposal adapters on discovery folds
         -> localization -> deletion equivalence on confirmation folds
         -> moment battery -> curvature tracks -> joint model
         -> held-out composition -> state expansion -> report bundle
```

Each node records input hashes, code revision, feature flags, seed, wall time, numerical policy, and output hashes.

## 4. Strict and exploratory modes

Strict mode returns no causal certificate when any of the following holds:

- unknown or x-dependent within-regime selection;
- inadequate common support;
- GCM requested under non-product sampling without reweighting;
- design contrast aliased or unidentifiable;
- no unique deletion orientation;
- cluster unit missing or inconsistent;
- model calibration outside tolerance;
- self-normalization residual exceeds policy;
- negative controls show implementation curvature above threshold;
- a data-adaptive proposal reuses its discovery observations for confirmatory evidence.

Exploratory mode may continue, but every affected result is watermarked as diagnostic and cannot be serialized as `certificate_status: passed`.

## 5. Numerical policy

- Use log space for density ratios and `log1p`/`expm1` for small curvature.
- Normalize by log-sum-exp or compensated summation.
- Store both raw and normalized composition weights.
- Detect nonfinite values at every adapter boundary.
- Use deterministic reduction order when reproducibility mode is enabled.
- Preserve a high-precision oracle path for conformance fixtures.

## 6. Parallelism

Independent folds, faces, witnesses, bootstrap replicates, and candidate-state expansions form natural tasks. Use structured concurrency through the runtime adapter, bounded by explicit memory and CPU budgets. Never parallelize a deterministic reduction in a way that changes the published reproducibility contract without recording the numerical mode.

## 7. Report bundle

A completed run emits:

```text
run/
  manifest.resolved.json
  provenance.json
  design_audit.json
  proposals.json
  overlap.json
  localization.json
  orientation.json
  moments.json
  curvature.json
  composition.json
  state_expansion.json
  negative_controls.json
  report.html
  report.md
  evidence.jsonl
```

The report begins with assumptions and abstentions, not with a p-value table.

## 8. Performance priorities

1. avoid repeated parsing and materialization;
2. store regime rows contiguously while preserving stable row IDs;
3. cache fold-specific feature matrices and kernel blocks;
4. stream bootstrap statistics rather than retaining all replicates;
5. exploit vectorized residual products and symmetric Gram matrices;
6. batch neural regime predictions;
7. profile before changing numerical kernels;
8. retain differential conformance after every optimization.
