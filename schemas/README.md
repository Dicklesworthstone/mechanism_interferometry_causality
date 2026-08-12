# Schemas

- `experiment_manifest.schema.json` validates analysis inputs and design declarations.
- `evidence_finding.schema.json` defines stable reason-coded findings.
- `audit_report.schema.json` defines the final content-addressed report envelope.
- `four_law_report.schema.json` defines the histogram four-law tabular diagnostic (never a certificate).
- `benchmark_routing_view.schema.json` defines the neutral, proposal-only view visible to a strategy router.
- `design_authority_receipt.schema.json` defines the separately supplied or explicitly withheld assignment/design authority.
- `benchmark_oracle.schema.json` defines study identity and expected routes that remain sealed until scoring.
- `design_diagnostic_receipt.schema.json` defines deterministic, discovery-only relevance and positivity diagnostics; raw table bytes alone cannot claim those checks passed.

JSON Schema cannot enforce that corner sampling proportions sum to one or that all design points share one dimension. Both constraints are checked by `scripts/check_repo.py` and `mic-data`.

The benchmark schemas deliberately use three views. Combining them would let a
router recover the study, strategy, or published result from the same artifact
that it is supposed to analyze blindly. `scripts/check_repo.py` checks the
synthetic contract template's content bindings, referential integrity, premise
evidence, strategy/estimand truth table, and authorized/blind byte identity. It
does not enforce runtime isolation; an eventual runner must produce a separate
execution receipt before any fixture can be called a blind benchmark.
