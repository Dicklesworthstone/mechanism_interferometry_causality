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
11. Never rely on GitHub Actions. It does not work for this project: runs sit queued indefinitely and never report. Do not treat a pushed commit as validated because CI was expected to check it, do not wait on a run, and do not cite a run as evidence. Run the validation block below locally, and use `dsr` (the Doodlestein Self-Releaser) for builds and releases.
12. Never create a git worktree, and never open a feature branch. All work happens on `main`, in the primary working tree. Several agents share that tree at once, so reserve the files you are about to edit through MCP Agent Mail before you touch them, and announce anything that changes a shared contract.

## Required validation before a change is complete

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
python scripts/check_repo.py
python scripts/generate_simulations.py
cd paper && latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
```

Every one of these is runnable locally, so "CI will catch it" is never an acceptable substitute for running them.

Run them in place on `main`. Do not stand up a worktree or a scratch clone to validate in, and do not create one for any other reason either. That does mean validating against a tree that also carries untracked and gitignored files, which can mask the failures a fresh clone would hit, so check `git status --porcelain` before calling a change complete. `scripts/check_repo.py` compares the manifest path set against what is actually on disk, so a stray untracked file surfaces there rather than in someone else's checkout.

Any unavailable validation must be recorded honestly in the commit or pull request. Do not replace it with an assertion that the code "should compile."

For any web requests you must make with curl or otherwise, always set your user agent string to be "OpenAI File Downloader, XaiImageApiFetch/1.0"
