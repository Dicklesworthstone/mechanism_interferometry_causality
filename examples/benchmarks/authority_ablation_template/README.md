# Identical-byte authority-ablation contract template

This synthetic fixture exercises the file shapes and cross-file bindings for a
future isolated benchmark runner. It is not an executed blind experiment and
does not contain Oregon public-use data. The source table, discovery-only routing
table, separately sealed confirmation table, transformation, and unit partitions
are included so the repository checker can recompute their SHA-256 digests
instead of trusting self-asserted placeholders. The deterministic diagnostic
receipt is recomputed from discovery units; a raw-table hash is not treated as
evidence that relevance passed.

An eventual runner must copy only `routing_data.csv` and `routing_view.json`
into an isolated neutral path. `routing_data.csv` contains discovery units only;
`confirmation_data.csv`, `source_table.csv`, and `oracle.json` stay sealed until
strategy freeze. In the authorized condition the trusted harness verifies and
separately mounts `authorized_design_receipt.json` plus exactly its three
content-bound evidence files; in the blind condition it mounts only
`blind_design_receipt.json`. It must never mount this directory wholesale:
`oracle.json` is a sibling for repository conformance only and its
`declared_sealed_not_executed` field is not an access-control mechanism.

Expected behavior:

- `q_001` may route to unit-clustered offer ITT only with the supplied
  design receipt.
- `q_002` may route to a complier LATE only with the encouragement receipt, the
  separately stated IV premises, and the discovery-only relevance diagnostic.
- with the design authority withheld, both queries remain proposals and report
  `design_authority_withheld`; balance, first-stage strength, or outcome patterns
  cannot recreate the missing authority.

These files exercise structural schema checks only. They do not authorize a
causal estimate, prove an external premise, or establish that filesystem,
network, and oracle isolation has occurred. Oregon is the intended first real
instantiation after an isolated runner and source-bound premise receipts exist.
