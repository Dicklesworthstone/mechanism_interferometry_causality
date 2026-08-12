# Agent Contract

This repository is a scientific audit system. Correctness evidence outranks feature count.

## Non-negotiable rules

1. Preserve the distinction between density curvature and conditional design covariance.
2. Never run a GCM curvature test unless product sampling odds have been verified or observations have been explicitly reweighted to a product design.
3. Never call failure to reject a deletion discrepancy evidence of invariance. Use equivalence bounds.
4. Never self-normalize compositional weights without also reporting the raw normalizer residual.
5. Never infer a target when the deletion pass count is zero, greater than one, or statistically undetermined.
6. Never treat cells, requests, or time steps as independent if randomization occurred at a higher unit.
7. Every stochastic operation must receive a deterministic seed recorded in the evidence ledger.
8. Every optimization must preserve exact-algebra conformance fixtures and strict-mode failure behavior.
9. `unsafe` is forbidden throughout this workspace.
10. Franken* integrations belong behind adapter traits. Core causal contracts may not depend on unstable implementation details of sibling repositories.

## Required validation before a change is complete

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
python scripts/check_repo.py
python scripts/generate_simulations.py
cd paper && latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
```

Any unavailable validation must be recorded honestly in the commit or pull request. Do not replace it with an assertion that the code "should compile."
