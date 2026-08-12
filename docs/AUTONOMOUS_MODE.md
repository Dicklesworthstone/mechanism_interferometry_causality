# Autonomous Mode: Unsupervised Mechanism Survey

Goal: point the system at a large pile of tabular data with no manifest, no declared design, and no prior graph, and recover as much audited causal-mechanism structure as the data can honestly support, with every remaining gap converted into a concrete contract request or experiment proposal.

Autonomous mode is a crawler over the certificate machinery, not a new inference principle. Nothing in this mode weakens the agent contract. What cannot be verified from data alone is never certified; it is surfaced, graded, and attached to an explicit upgrade path.

## The central obstruction and its resolution

The certificate machinery consumes regimes. Raw data does not arrive with declared regimes, sampling proportions, selection contracts, or randomization units. Three of these four gaps have data-driven partial resolutions; one does not.

1. **Regimes** can be discovered: many tables contain natural context variables (flags, arms, batches, environments, versions, time segments) whose values induce candidate regime laws. This is the Joint Causal Inference setting entered from the data side.
2. **Sampling proportions** can be estimated from corner counts, and product odds can be tested empirically. Estimated quotas support diagnostic four-law functionals; they are not known quotas.
3. **Randomization units** can be conservatively guessed (identifier columns, temporal blocks) and every audit run at the coarsest plausible clustering; ambiguity is recorded and the worst-case grade prevails.
4. **State-independent selection cannot be established from the data alone.** No pattern in observed rows proves that inclusion did not depend on state within regime. This is the permanent wall between autonomous surveying and certification, and the system says so on every artifact.

Consequently every autonomous output carries one of three authority tiers:

| Tier | Name | Requirements | Serialized as |
|---|---|---|---|
| A | Certified | Declared sampling, selection, and clustering contracts supplied by a human or upstream system | `certificate_status` from the strict ledger |
| B | Corroborated diagnostic | All internally checkable audits pass: design estimability, four-law form or empirically-product-consistent residual products, overlap, lens battery, conservation cross-checks, negative controls where available | `diagnostic_only` plus a typed checklist of individual audit outcomes; deliberately no scalar score and no total ordering, so a reader always sees which gates passed and which were inapplicable |
| C | Proposal | Passive-learner output, quarantined per `PROPOSAL_ADAPTERS.md` | `proposal_only` |

Tier B is the novel operating point: far stronger than a discovery algorithm's edge weight, deliberately weaker than a certificate. The tier system is what lets the crawler be unleashed without supervision: it cannot overclaim, because overclaiming is a type error.

## Pipeline

### Stage 0: column triage

Partition columns into candidate context variables, candidate state variables, candidate cluster identifiers, and excluded columns.

- Context candidates: low-cardinality columns whose value is plausibly assigned rather than caused. Scored, not assumed: a context score combines cardinality, regime-classifier signal (can a cross-fitted classifier distinguish rows by this column from state alone; strong separation is evidence of a real regime difference, and perfect separation is an overlap alarm, not a discovery), temporal precedence when timestamps exist, and stability across data splits.
- Cluster candidates: identifier-shaped columns and coarse temporal blocks. The audit runs at the coarsest surviving candidate; finer-grained claims require a declared unit.
- Everything else is state. Nothing is dropped silently; exclusions are findings.

### Stage 1: design discovery

For each small set of binary or binarized context variables (singletons, pairs, then triples under a budget):

1. map observed value combinations to design corners; a corner is observed when it holds at least the minimum cluster count;
2. run `audit_design` and `audit_interaction_aliasing` on the observed corners: this yields, before any estimation, exactly which pairwise mechanisms are testable on this data (`fully_aliased` pairs are reported as untestable and generate design proposals rather than estimates);
3. estimate corner proportions from cluster counts and run `audit_sampling_odds`: empirically product corner counts license residual-product diagnostics at tier B only — observed frequencies that happen to look product do not verify product assignment odds, so autonomous runs never construct verified product-design evidence and never call this output a GCM curvature test; four-law functionals with estimated quotas are the default tier B path;
4. record the empirical overlap profile per corner pair via ratio-weight effective sample size.

The output is a testability atlas: which context pairs form usable interferometers on this data, which are aliased, and which need more corners.

### Stage 2: passive survey (quarantined, tier C generation with tier B checking)

For each usable interferometer, run the full audit battery:

- localization of each context bit's regime family by parsimony-frontier ensembles;
- deletion-equivalence orientation with multiplier bounds at the inferred cluster level, yielding the five-state pass count;
- curvature: four-law moment batteries with witness families under the estimator lens battery; residual-product diagnostics run only where empirical corner counts are product-consistent and are always labeled diagnostic, per Stage 1 — no autonomous run performs a GCM curvature test;
- conservation cross-checks: the three-way law `Cov(r_A, r_B) = −E[r_A r_B(e^κ − 1)]` relates quantities the survey estimates anyway, so every pair carries a consistency check; the two sides must be estimated through separate held-out routes (distinct folds or distinct estimator families) for the check to constitute estimation quality evidence — when both sides derive algebraically from the same fitted ratios the check can pass by construction and is recorded as algebra-consistency only, not estimator validation;
- negative controls whenever the data contains context pairs known or scored to be inert.

Conservation self-checking is the survey's distinctive advantage over generic discovery: the algebra is overdetermined, so a pile of data audits its own estimates.

### Stage 3: graph assembly under abstention

Families with unique targets orient; the peeling reconstruction assembles a partial DAG from oriented families. Everything else remains in its exact failure state: `multiple_passes`, `no_pass`, `underpowered`, `undetermined`. The assembled object is a partial graph in which every edge carries its evidence ledger, tier, pass-count state, curvature summary, and reason codes. There is no acyclicity forcing, no faithfulness assumption beyond the declared ones, and no completion heuristic; holes are the product, not a defect.

### Stage 4: curvature-driven measurement search

For every curved pair, the nested state-expansion law converts curvature into a search over candidate measurement blocks already present in the data: refit at `(X, W)`, test residual flatness on held-out folds, and verify the recovered covariance term. Blocks that flatten curvature are reported as explanatory measurements; curvature that nothing in the table explains becomes a ranked missing-measurement recommendation. This is the interferometer used as an instrument: the system tells you what to measure next.

### Stage 5: proposal emission

Every non-terminal state emits a machine-readable proposal: ambiguous orientations produce active-tilt requests; non-product corners produce reweighting or randomization proposals; aliased pairs produce design-extension proposals (which corners to add); underpowered families produce sample-size statements; and every tier B result lists the exact contracts whose declaration would re-run it at tier A. The survey's final artifact is therefore a closed loop: audited structure, plus the cheapest path to more.

## Failure honesty

- A discovered context variable can itself be an effect (conditioning on a collider manufactures dependence). Stage 0 scores lower any context candidate whose regimes are separable only through downstream-looking state, and Stage 2's deletion audit catches many such cases as `no_pass`; the residual risk is stated on every Stage 0 artifact.
- Estimated proportions are not known proportions; the empirical product-odds test has power limits recorded alongside its verdict.
- Multiple testing across the atlas is controlled family-wise per interferometer and reported globally; exploratory sweeps never promote without a confirmatory split.
- The wall stands: no autonomous run certifies. Tier A requires declared contracts, always.

## Implementation map

- Stage 0 and ingestion: the tabular slice (Packet 1) plus a context-scoring pass.
- Stage 1: implemented primitives (`audit_design`, `audit_interaction_aliasing`, `audit_sampling_odds`, `observed_design_from_rows`) composed over discovered columns.
- Stage 2: implemented primitives (`parsimony_frontier`, `classify_deletion`, `orient_from_deletions`, `simultaneous_mean_bounds`, `four_law_moment`, `gcm_projection`, `audit_lens_battery`, `audit_overlap`) plus the four-law estimation path over real tables.
- Stage 3: `mic_design::peel_families` implements the paper's peeling reconstruction with simultaneous rounds (repeated tilts of one node resolve together) and a conservative stuck state instead of forced orientation.
- Stage 4: the state-expansion loop (Packet 7) over candidate blocks.
- Stage 5: `mic-proposal` currently implements active-tilt ranking only; the design-extension and contract-request proposal kinds are roadmap items, not shipped code.

The mode ships when Stages 0 through 3 run end to end on one public dataset and the resulting atlas leads with its abstentions.
