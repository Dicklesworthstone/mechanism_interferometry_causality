# Proposal Adapters

## 1. Purpose

The mechanism-interferometry certificate is evaluated relative to proposed causal families and target assignments. Domain graphs, passive discovery systems, parsimony searches, and previous audit runs can make that proposal step much more efficient. They do not establish any certificate condition.

The proposal layer therefore has one narrow authority:

> It may decide what to test next. It may not decide what is true.

All external proposal systems remain behind local adapter traits. `mic-core` and the certificate state machine receive only validated, serializable candidate sets; they never depend on an external learner's implementation details or causal claims.

## 2. Epistemic boundary

| Proposal output | Permitted use | Prohibited interpretation |
|---|---|---|
| Candidate support or DAG edge | Order localization and deletion tests | Confirmed parent, target, or causal edge |
| Parsimony-frontier inclusion frequency | Stability diagnostic and search priority | Calibrated edge probability |
| Residual or model-importance score | Exploratory witness or candidate generator | Evidence of causal direction |
| Candidate measurement block | Queue a held-out state-expansion comparison | Explanation of curvature |
| Candidate follow-up tilt or design corner | Plan a new randomized experiment | Resolution of an existing ambiguity |

Only the ordinary audit pipeline can promote a proposal: locality evidence, simultaneous deletion equivalence, design-eligible curvature inference, overlap and selection gates, held-out composition, and negative controls. A proposal score never changes an equivalence bound, significance threshold, strict-mode gate, or final certificate status.

## 3. Proposal record

Every proposal batch should serialize at least:

- a stable proposal and adapter identifier;
- source kind: preregistered domain graph, passive learner, previous independent audit, or exploratory search;
- candidate supports, optional target labels, and ordering scores;
- the exact meaning and direction of every score;
- whether a score is calibrated, and the calibration population if it is;
- data, row, cluster, and fold fingerprints used to construct the proposal;
- model family, hyperparameter policy, implementation revision, and feature flags;
- every random seed and deterministic tie-breaking rule;
- warnings, failed candidates, and unresolved ties.

Uncalibrated selection frequencies, model importances, Bayes-free score ratios, and normalized vote counts must be named `score`, `frequency`, or `priority`, never `probability` or `confidence`.

## 4. Discovery and confirmation

Data-adaptive proposals create a selection event. Strict mode uses one of two valid routes:

1. Freeze the proposal before observing the confirmatory data, preferably from domain knowledge or a previous independent experiment.
2. Allocate outer folds at the true randomization unit before proposal fitting. Construct proposals only on discovery folds and evaluate certificate-bearing hypotheses only on untouched confirmation folds.

The same observation, cell, request, time step, or cluster may not both select a support, witness, measurement block, or tilt and provide its confirmatory evidence. Inner cross-validation does not repair this reuse. If no independent confirmation sample remains and selective inference has not been implemented, the result is `diagnostic_only`.

All splitting and resampling operates at the assignment unit. A proposal search over cells does not create cell-level independent evidence when constructs, subjects, hosts, or deployments were randomized.

## 5. Parsimony-frontier support proposals

A safe adaptation of passive parsimony search uses the regime label, not an arbitrary outcome, as the prediction target:

1. On discovery folds, evaluate candidate state supports with the same held-out proper scoring rule.
2. Retain the smallest supports whose loss is within a preregistered absolute or relative tolerance of the best completed candidate on that split.
3. Repeat across deterministic cluster-level splits and deliberately different learner families.
4. Average each variable's inclusion indicator within split before averaging across splits, so splits with more tied supports do not receive more weight.
5. Emit the union or a ranked family of stable supports for the ordinary localization and orientation audit.

The frontier threshold is computed after all candidates for a split finish. It may not depend on evaluation order. Model byte size and parameter count are not comparable description lengths across learner families; support cardinality is the primary common complexity axis, while learner-specific complexity remains a sensitivity diagnostic.

Locality gives this heuristic a principled target: a minimal support that preserves regime information is a candidate for `{target} union parents`. It is still only a candidate. Descendant contamination, proxies, symmetries, insufficient tilts, and finite-sample error are resolved by the fail-closed deletion audit rather than by the frontier score.

## 6. Passive DAG and residual adapters

An observational DAG learner may propose family sets or intervention priorities. Its Markov-equivalence ambiguity, latent-confounding assumptions, acyclicity policy, and score calibration must survive in the proposal record. The engine must not convert edge frequency into a certificate finding.

Residual-noise asymmetry can be exposed as an optional exploratory adapter only when its functional assumptions are explicit. Residual normality, low autocorrelation, additive-noise fit, or agreement among regressors is not a mechanism-interferometry orientation test. In particular, it may not break a `MULTIPLE_PASSES`, `NO_PASS`, or `UNDETERMINED` deletion result.

## 7. Active follow-up proposals

Ambiguity can generate a new experiment rather than a forced verdict. For surviving orientation hypotheses `H` and a feasible candidate-tilt library `Q`, the planner may rank tilts by a preregistered acquisition rule such as the minimum predicted pairwise separation of deletion discrepancies:

```text
priority(q) = min over h != h' in H of Separation(prediction(q, h), prediction(q, h'))
```

Other allowed objectives include expected information gain, cost-adjusted separation, and power subject to overlap. The planner records the full candidate library, rejected constraints, prediction uncertainty, acquisition seed, and ties.

The proposed tilt must preserve the asserted primitive target and must have measurable delivery. In the balanced parity example, biasing the parent is a different intervention; an admissible same-target follow-up is an asymmetric replacement of the target conditional that is predicted to separate the deletion hypotheses. The chosen tilt is then run as a new independently randomized regime and analyzed from preflight onward. Its predicted separation is not evidence until those data exist.

Candidate design corners are subject to the same product-odds rule as every other GCM analysis. If the follow-up allocation is not product, the engine must use four-law inference or explicitly reweight to a declared product design.

### Implemented active-tilt surface

`mic_proposal::rank_active_tilts` is the reference proposal-layer primitive. Its request consumes the surviving labels from a multiple-pass orientation audit, the primitive intervention that every admissible replacement must preserve, a planned analysis track, complete adapter provenance, and the deterministic seed used upstream. Each candidate supplies a complete table of predicted separations over every unordered pair of surviving hypotheses.

Before ranking, the primitive rejects candidates that change a different intervention, lack measurable delivery or common support, omit or duplicate a hypothesis pair, produce nonfinite scores, or propose Product-Factorial analysis without a referenced product-odds audit or explicit reweighting plan. Accepted candidates are ranked by their minimum predicted pairwise separation. Experimental cost breaks exact score ties but never changes the acquisition objective: a declared finite cost sorts before an unspecified cost, then canonical candidate identifier breaks the remaining tie.

The result has fixed authority `proposal_only`. It stores raw score semantics, rejection summaries and reason codes, source/fold/assignment-unit fingerprints, the upstream seed, the exact deterministic tie rule, and a derived SHA-256 fingerprint binding the complete ordered candidate library, including candidates that fail feasibility. Its status is explicit: `recommended` when at least one candidate survives, otherwise `abstained_no_eligible_candidate`. The latter is an experiment-design abstention, not an orientation outcome.

`mic propose-tilt INPUT.json` exposes this boundary without adding certificate authority. [`examples/proposal_inputs/parity_active_tilt.json`](../examples/proposal_inputs/parity_active_tilt.json) and [`schemas/active_tilt_input.schema.json`](../schemas/active_tilt_input.schema.json) define the executable input; [`examples/proposals/parity_active_tilt.json`](../examples/proposals/parity_active_tilt.json) and [`schemas/proposal_batch.schema.json`](../schemas/proposal_batch.schema.json) define its deterministic output. The chosen candidate remains a recommendation for a new randomized regime, not a resolution of the current audit.

## 8. Audit artifacts

The report bundle should include `proposals.json` whenever proposal adapters affect search order. It records:

- proposal inputs and hashes;
- discovery and confirmation boundaries;
- candidate rankings and raw scores;
- adapter and learner-family disagreement;
- which proposals were tested, rejected, unresolved, or not reached;
- follow-up measurements, tilts, or corners recommended for a future run.

Reports place this artifact under exploratory evidence. A certificate remains unchanged if proposal ordering is permuted while the tested hypothesis family and confirmatory data are held fixed.
