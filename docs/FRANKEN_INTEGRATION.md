# Franken Ecosystem Integration

The workspace pins the sibling projects at reviewed revisions so every scientific run can record an exact numerical substrate.

| Project | Pinned revision | Role |
|---|---|---|
| `Dicklesworthstone/frankenpandas` | `a9f8d86c9e52923b9b2082d00a65841862d5ca9a` | Typed tabular ingestion, Arrow/Parquet/CSV interchange, grouping, fold and report tables |
| `Dicklesworthstone/franken_numpy` | `6964e776528f1e492620ebd627d78d4f958220f4` | Arrays, broadcasting, deterministic random streams, linear algebra, vectorized kernels |
| `Dicklesworthstone/frankenscipy` | `e259ed002eec05a2eca08d38a0763e0e58b0623c` | Statistical distances, optimization, bootstrap/permutation, special functions |
| `Dicklesworthstone/frankentorch` | `5a3a0e70a2854c08e42ae02d816a78b8f88d912d` | Multinomial/ratio models, autograd, representation probes, CPU and Metal execution |

All four projects use Rust 2024, but their current toolchain policies differ: FrankenPandas and FrankenNumPy pin `nightly-2026-07-05`, FrankenSciPy pins `nightly-2026-07-20`, and FrankenTorch currently tracks floating nightly. This workspace pins `nightly-2026-07-20`, the newest explicit sibling pin, and records that compatibility choice in every scientific run. Integration is feature-gated and isolated behind local traits so the causal core does not depend on unstable sibling APIs.

## FrankenPandas adapter

The feature-gated adapter now shares the standard reader's semantic validation
path and independently compares every returned cell with the source token. That
comparison is load-bearing: at the pinned `a9f8d86` revision, the CSV backend
parses numeric-looking values before storing them as UTF-8, so identifiers such
as `00` and `007` are returned as `0` and `7`. This can alias regimes or
randomization units. The adapter therefore **fails closed on ordinary MIC
bit-string regimes at this pin**; use the standard-library CSV reader until the
sibling preserves requested string cells byte-for-byte. `DType::Utf8` alone is
not evidence of lexical fidelity.

The adapter contract is to:

- read CSV, JSONL, Parquet, Feather/Arrow, and SQL-backed inputs;
- preserve stable row IDs and declared cluster IDs;
- perform regime-stratified grouped validation;
- expose zero-copy numeric blocks where possible;
- write every audit table in both JSON and Parquet;
- use the runtime evidence ledger to record compatibility and hardening decisions.

The current implementation covers guarded CSV ingestion only. JSONL, Parquet,
Feather/Arrow, SQL inputs, and tabular output remain roadmap items rather than
available capabilities.

The top-level package exposes a unified `frankenpandas::prelude::*`; production code should still import explicit symbols in library crates to keep API drift visible.

## FrankenNumPy adapter

Use `fnp-ndarray` as the canonical dense numeric carrier and specialized sibling crates for:

- vectorized residual products;
- pairwise squared distances and Gram matrices;
- batched log-ratio and softmax transforms;
- deterministic randomization and bootstrap index streams;
- rank and null-space calculations where the linalg surface is mature.

Avoid hidden conversion through `Vec<Vec<f64>>` in hot paths. Adapter tests must compare shape, stride, dtype, and reduction semantics against exact fixtures.

## FrankenSciPy adapter

Initial targets:

- `fsci-stats`: MMD, energy distance, permutation and bootstrap utilities;
- `fsci-opt`: calibration, constrained odds-ratio fitting, and witness optimization;
- `fsci-linalg`: stable design-rank and null-space routines;
- `fsci-special`: logistic, log-sum-exp, and tail calculations;
- `fsci-runtime`: structured concurrency and evidence-compatible execution.

Where a required statistic is not yet exposed, implement it first in `mic-stats` against a minimal array trait, add differential fixtures, and upstream the generic primitive to FrankenSciPy rather than creating an opaque permanent fork.

## FrankenTorch adapter

Use FrankenTorch only for learned components:

- one multinomial classifier across all observed corners;
- main-effect-only and hierarchical interaction parameterizations;
- calibrated posterior odds including corner intercepts;
- primitive ratio extraction and self-normalization;
- adaptive witness estimation;
- invertible or design-sufficient representation probes.

The adapter must export ordinary arrays and evidence records. The certificate never depends on a hidden tensor state. CPU is the reference backend; Metal acceleration is accepted only after posterior, gradient, and curvature-field conformance.

## Dependency policy

- Pin exact revisions in scientific releases.
- Record the four revisions in every run bundle.
- Update one sibling at a time through an explicit compatibility packet.
- Run exact simulations, differential inference fixtures, and performance baselines before accepting a pin change.
- Never use a broad third-party dataframe or tensor stack as a silent fallback when a Franken feature is unavailable. Strict mode should report the missing capability.
