# Public-dataset eligibility matrix

This document reviews two public factorial designs for
`schemas/experiment_manifest.schema.json` and records one tempting scalar screen
that is explicitly **not** eligible for the density-law audit. It is a
**design-review artifact**, not a certificate. None of the source tables are
bundled. A missing file is a fail-closed ingest error, not a synthetic stand-in.

The only datasets considered here already have, or can be reduced to, a
four-corner square: control, A, B, and AB. Observational DAGs, GWAS, and
single-arm A/Bs are ineligible. No square, no \(\kappa\). The Stage 0–1
survey splits never-seen arms (`missing_corners`) from
under-supported arms (`dropped_corners`) and does not invent either.

## Shared mapping

| Manifest field | Meaning | Fail-closed if |
|---|---|---|
| `inference_track` | `four_law`, `product_factorial`, or `both` | `product_factorial` / `both` without product odds |
| `selection` | caller declaration about within-regime inclusion | strict readiness until external evidence is resolved; `unknown` and unmodeled dependence also block diagnostics |
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
  `data file not found`. Identities-only UMI/coverage *mean* κ on CBL/CNN1
  sign-flips across gemgroups as a fold-specific scalar (slate bb-ule).
  The four-corner `+1.020` is post-selection row-count odds, not assignment
  odds. Raw `Z=1.160`.

## 2. NCI-ALMANAC / DrugComb scalar combination screens: not MIC-law eligible

- Sources: NCI-ALMANAC; DrugComb / DrugCombDB for pair lookup.
- Proposal contract:
  [`examples/datasets/nci_almanac/scalar_response_contract.json`](../examples/datasets/nci_almanac/scalar_response_contract.json).
- Vehicle, drug A, drug B, and AB may form a factorial square on a declared
  percent-growth response scale. They do **not** supply four joint state laws.
- Exact nonidentifiability witness: for state `Z in {0,1,2}`, four identical
  uniform laws have scalar mean one and zero density curvature. Replacing only
  the AB law by `(1/4,1/2,1/4)` preserves the same scalar mean one but yields
  nonzero curvature. The released scalar corners cannot distinguish the worlds.
- Allowed work: reproduce an explicitly named additive or modified-Bliss
  response summary; predict held-out combination percent growth; inventory
  missing dose cells; test sensitivity to response scale and null definition.
- Forbidden work: density curvature, conditional normalization, target-family
  orientation, or a raw compositional normalizer. Adding jitter or treating a
  scalar as a degenerate distribution only invents a state law and violates
  common-support semantics.
- **Unit:** the true screen/plate/experiment block if released and externally
  documented. Dose cells and NCI-60 cell lines are not automatic replicates.
- Honest status today: **proposal-only scalar benchmark**. The former strict
  MIC manifest was removed because it declared state-independent selection and
  equal regime proportions without evidence and mislabeled scalar viability as
  the complete state.

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
| NCI-ALMANAC scalar response corners | Factorial scalar endpoints do not identify joint state laws or density curvature |
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
