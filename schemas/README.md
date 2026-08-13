# Schemas

- `experiment_manifest.schema.json` validates analysis inputs and design declarations.
- `selection_evidence_receipt.schema.json` binds a state-independence or validated-model premise to the exact manifest, analyzed data, and external authority-source bytes. The receipt is resolved by the engine; the manifest declaration alone never satisfies readiness.
- `evidence_finding.schema.json` defines stable reason-coded findings.
- `audit_report.schema.json` defines the final content-addressed report envelope.
- `four_law_report.schema.json` defines the histogram four-law tabular diagnostic (never a certificate).
- `closure_crossfit_request.schema.json` separates the highest declared
  dependence unit from nested assignment episodes; the former is never split
  across folds, while the diagnostic verifies episode nesting but not physical
  unit provenance.
- `benchmark_routing_view.schema.json` defines the neutral, proposal-only view visible to a strategy router.
- `design_authority_receipt.schema.json` defines the separately supplied or explicitly withheld assignment/design authority.
- `benchmark_oracle.schema.json` defines study identity and expected routes that remain sealed until scoring.
- `design_diagnostic_receipt.schema.json` defines deterministic, discovery-only relevance and positivity diagnostics; raw table bytes alone cannot claim those checks passed.
- `finite_completion_request.schema.json` defines bounded finite-state fixed-DAG/target exact-or-simulated population-table diagnostics; estimated point tables require a separate uncertainty model and cannot emit population completion statuses. Its output is model-relative and never certificate authority.
- `dictionary_search_plan.schema.json` freezes the reference convention, code family, folds, rank grid, ranking rule, and hard search budget before dictionary fitting.
- `transport_dictionary_draft.schema.json` closes the complete adapter attempt library, including completed, rejected, and unexecuted parameterizations.
- `mechanism_dictionary_proposal.schema.json` defines the serialize-only transport-dictionary proposal. Despite the historical filename, its `artifact_kind` is `transport_dictionary_proposal`; all causal-family, mechanism, target, grouping, selection, and edge authority remains unavailable.

JSON Schema cannot enforce that corner sampling proportions sum to one or that all design points share one dimension. Both constraints are checked by `scripts/check_repo.py` and `mic-data`.

The benchmark schemas deliberately use three views. Combining them would let a
router recover the study, strategy, or published result from the same artifact
that it is supposed to analyze blindly. `scripts/check_repo.py` checks the
synthetic contract template's content bindings, referential integrity, premise
evidence, strategy/estimand truth table, and authorized/blind byte identity. It
does not enforce runtime isolation; an eventual runner must produce a separate
execution receipt before any fixture can be called a blind benchmark.
