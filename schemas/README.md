# Schemas

- `experiment_manifest.schema.json` validates analysis inputs and design declarations.
- `evidence_finding.schema.json` defines stable reason-coded findings.
- `audit_report.schema.json` defines the final content-addressed report envelope.
- `four_law_report.schema.json` defines the histogram four-law tabular diagnostic (never a certificate).

JSON Schema cannot enforce that corner sampling proportions sum to one or that all design points share one dimension. Both constraints are checked by `scripts/check_repo.py` and `mic-data`.
