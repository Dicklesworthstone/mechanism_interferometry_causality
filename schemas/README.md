# Schemas

- `experiment_manifest.schema.json` validates analysis inputs and design declarations.
- `evidence_finding.schema.json` defines stable reason-coded findings.
- `audit_report.schema.json` defines the final content-addressed report envelope.

JSON Schema cannot enforce that corner sampling proportions sum to one or that all design points share one dimension. Both constraints are checked by `scripts/check_repo.py` and `mic-data`.
