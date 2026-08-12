# Security, Reproducibility, and Auditability

- Safe Rust is enforced with `unsafe_code = "forbid"`.
- Every run uses a resolved manifest and content-addressed input fingerprints.
- Seeds, folds, cluster IDs, sampling offsets, feature flags, and dependency revisions are serialized before model fitting.
- Strict mode is fail-closed. Exploratory overrides are explicit and survive into every report.
- Numerical overflows, nonfinite log ratios, singular design matrices, and insufficient overlap are typed errors.
- Private datasets are never embedded in the website or paper build.
- Report HTML is static and contains no remote JavaScript.
- Artifact paths are relative and portable; no user home-directory paths are serialized.
- The conformance corpus contains only synthetic or redistribution-safe fixtures.
