# Oregon identical-byte authority ablation

This fixture specifies the benchmark contract without redistributing the Oregon
public-use data. The repeated placeholder digests stand for one frozen source
table and one neutralized routing table; a real adapter must replace them with
retrieval-time SHA-256 digests.

The router receives `routing_view.json`. In the authorized condition it also
receives `authorized_design_receipt.json`; in the blind condition that file is
replaced by `blind_design_receipt.json`. The analysis bytes, neutral column IDs,
queries, and unit partition remain identical. `oracle.json` is unavailable until
scoring.

Expected behavior:

- `q_offer` may route to household-clustered offer ITT only with the supplied
  design receipt.
- `q_coverage` may route to a complier LATE only with the lottery receipt and the
  separately stated IV premises.
- with the design authority withheld, both queries remain proposals and report
  `design_authority_withheld`; balance, first-stage strength, or outcome patterns
  cannot recreate the missing authority.

These files exercise schema and authority separation. They are not estimates of
the Oregon experiment and do not claim that an external premise has been proved
from rows.
