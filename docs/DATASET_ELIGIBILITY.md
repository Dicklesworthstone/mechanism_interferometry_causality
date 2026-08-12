# Public-dataset eligibility matrix

This document maps three public factorial designs onto
`schemas/experiment_manifest.schema.json`. It is a **design-review artifact**,
not a certificate. None of the source tables are bundled. A missing file is a
fail-closed ingest error, not a synthetic stand-in.

The only datasets considered here already have, or can be reduced to, a
four-corner square: control, A, B, and AB. Observational DAGs, GWAS, and
single-arm A/Bs are ineligible. No square, no \(\kappa\). The Stage 0–1
survey names the missing arms on incomplete designs (`missing_corners`)
and does not invent them.

## Shared mapping

| Manifest field | Meaning | Fail-closed if |
|---|---|---|
| `inference_track` | `four_law`, `product_factorial`, or `both` | `product_factorial` / `both` without product odds |
| `selection` | within-regime inclusion contract | `unknown` or `state_dependent_unmodeled` in strict mode |
| `cluster_column` | **randomization unit**, never the measurement | a cluster appears in more than one regime |
| `regime_column` | corner label; bit strings such as `00` are accepted | unknown label |
| `state_columns` | current observed state \(X\) | empty |
| `candidate_state_blocks` | held-out expansion blocks, not in \(X\) | overlap with `state_columns` |
| `regimes[].sampling_proportion` | known state-independent quotas \(\rho\) | they do not sum to 1 |
| `seed` | recorded in the ledger | omitted |

Four-law mode does **not** require product assignment. That is the wedge for
public data whose corner quotas are known but not balanced. GCM remains
illegal until `mic design odds` reports product sampling or an explicit
reweighting plan exists.

**Selection wall (H-002, now a proposition).** Selected rows plus a known
inclusion rate do not identify whether inclusion is state-independent.
Binary witness: source \(P=(1/2,1/2)\) with \(\pi=(2/3,2/3)\) and source
\(P'=(2/3,1/3)\) with \(\pi'=(1/2,1)\) both produce selected masses
\((1/3,1/3)\) and inclusion \(2/3\). A false `state_independent` declaration
is undetectable from the table. `--allow-unvalidated-selection-model` is
DiagnosticOnly, never Ready.

The std-CSV reader (`mic_data::load_csv_table`, `mic-tabular`) is **not**
Packet 1 FrankenPandas ingest. Packet 1 should attach to the same
`IngestReport` / fingerprint types.

## 1. Norman et al. 2019 combinatorial Perturb-seq

- Accession: GEO `GSE133344`.
- Template: [`examples/datasets/norman2019/manifest.json`](../examples/datasets/norman2019/manifest.json).
- Why it is a square: control, single-gene, and dual-gene perturbations exist
  for published pairs.
- A priori questions MIC can separate:
  - same-complex pair: strong phenotype epistasis, \(\kappa\) should shrink
    after the complex/state is observed;
  - linear pathway \(A\to B\to Y\): unique deletion invariance should name the
    target;
  - delivery artifact: \(\kappa\) that shrinks after `guide_efficiency` is
    implementation curvature, not biology.
- **Unit:** `replicate_id` or construct/batch. Cells are measurements.
- Recommended track: `four_law` until product assignment at the construct
  level is verified.
- `selection`: `state_independent_within_regime` only after QC that inclusion
  is not a function of the measured transcriptome within regime.
- Candidate blocks: `guide_efficiency`; `latent_cell_state_proxy`; both.
- Confirmation: alternate guides and held-out biological replicates. Pathway
  graphs may only propose pairs (`authority: proposal_only`).
- Honest status today: **eligible as a design**. Data are not in-repo.
  `mic-tabular ingest` against the template path abstains with
  `data file not found`.

## 2. NCI-ALMANAC / DrugComb combination screens

- Sources: NCI-ALMANAC; DrugComb / DrugCombDB for pair lookup.
- Template: [`examples/datasets/nci_almanac/manifest.json`](../examples/datasets/nci_almanac/manifest.json).
- Why it is a square: vehicle, drug A, drug B, and the combination, when those
  four arms exist for a cell line.
- A priori questions:
  - shared-target pair: \(\kappa\) should shrink after adding target-engagement;
  - independent-pathway pair: large viability synergy with \(\kappa\approx 0\)
    on a design-sufficient PK/PD state;
  - direction: is A inhibiting B's metabolism, or do they hit one node?
    Orient on PK/PD state, not on viability alone.
- **Unit:** `plate_id` or biological replicate, never the well if the plate
  was the randomized block.
- Recommended track: `four_law`. Published combo screens are almost never
  product-factorial at the analysis unit.
- Candidate blocks: `target_engagement`; `metabolic_activity`.
- Honest status today: **conditionally eligible**. Many published rows are
  incomplete squares. The adapter must drop pairs that lack all four corners
  rather than impute them.

## 3. Graduation / bundled anti-poverty factorial RCT

- Representative design: Banerjee et al. multi-arm Graduation programs and
  related 2×2 cash × training experiments.
- Template: [`examples/datasets/graduation_rct/manifest.json`](../examples/datasets/graduation_rct/manifest.json).
- Why it is a square: control, cash-only, training-only, cash+training.
- A priori question: does cash work *through* training, or are they
  autonomous mechanisms? That is locality + deletion, not an interaction
  coefficient on consumption.
- **Unit:** `household_id` or `village_id` as actually randomized. Never
  person-period.
- Recommended track: `product_factorial` only if assignment was independent
  at that unit; otherwise `four_law`.
- Candidate blocks: `endline_mediator_block` (labor supply, business assets)
  reserved for state expansion, not for the primary \(X\).
- Honest status today: **eligible as a design**. Microdata are license-gated
  and must not enter `site/` or `paper/`.

## What the software will do with a real extract

Once a CSV exists at the declared path:

1. `mic-tabular ingest` fingerprints bytes, clusters, and cluster-level folds.
2. `mic-tabular four-law` builds a cluster-weighted histogram projection,
   reports \(\kappa(x)\), \(E_0[r_j]\) **and** the raw normalizer residual,
   and the scalar / signed four-law moments.
3. `mic-tabular report` prints Markdown that **starts with certificate
   status and abstentions**. Histogram four-law never issues `passed`.
4. Product-factorial GCM is not computed. Packet 3 remains Packet 3.

## Ineligible, with reasons

| Tempting source | Why it is refused |
|---|---|
| Observational DAG learner output | No intervention square; scores are proposals only |
| GWAS / eQTL | No factorial soft-intervention family |
| Single-arm A/B | No second primitive, so no \(\kappa_{AB}\) |
| Perturb-seq treated as iid cells | Randomization unit is construct/replicate |
| Hard knockouts with empty common support | Soft-intervention calculus does not apply |

## Adapter contract for Packet 1

ProudGull's FrankenPandas slice should produce the same
`mic_data::IngestReport` fields:

- stable `row_id`, `cluster_id`, cluster-only folds;
- regime ids resolved from either the manifest id or the design bit string;
- `clusters_spanning_regimes` as a hard error;
- content and cluster fingerprints in the ledger;
- no cell-level iid assumption anywhere downstream.
