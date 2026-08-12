# Contributing

Every change must preserve the mathematical contracts in `docs/FORMAL_SPEC.md`, deterministic exact fixtures, safe-Rust policy, and fail-closed inference semantics.

Before committing:

```bash
./scripts/build_all.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
```

Scientific behavior changes require a reason-coded conformance fixture. Performance changes require before/after measurements and may not weaken numerical tolerances, overlap gates, calibration checks, or evidence provenance.
