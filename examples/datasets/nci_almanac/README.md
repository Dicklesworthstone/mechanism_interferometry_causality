# NCI-ALMANAC / DrugComb template

Keep only cell-line × drug-pair rows that have vehicle, A, B, and AB.
Incomplete squares are dropped, never imputed.

Cluster at `plate_id` (or the actual randomized block). Viability is an
outcome summary, not the design-sufficient state. Put PK/PD or
target-engagement measurements in `state_columns` or in a candidate block.
