# Self-driving causal tomography campaign

Status date: 2026-08-12

This document is the execution ledger for turning Mechanism Interferometry into
a useful system for large, heterogeneous tabular datasets. The objective is not
to force a DAG out of every table. It is to discover the strongest causal object
licensed by the available changes, expose every premise that rows cannot
establish, and recommend the cheapest observation or intervention that would
separate the surviving explanations.

The governing principle is:

> Sparse, recurring distribution changes nominate causal coordinates. Local
> conditional normalization, composition, design provenance, and independent
> confirmation determine whether those coordinates earn causal authority.

Change sparsity is a proposal principle, not a definition of causality. Fat
interventions, hidden common causes, selection, measurement drift, deterministic
proxies, and structure-preserving reparameterizations are all capable of
defeating a naive sparse-shift score.

## Product loop

The end-to-end product is a sequence of typed artifacts:

1. `atlas`: inventory columns, candidate environment axes, units, time blocks,
   missingness, support, complete and incomplete factorial structures.
2. `contract_request`: name the external facts required before a proposed route
   can carry authority.
3. `shift_factorization_proposal`: localize recurring changes on discovery
   units and freeze the complete considered and rejected candidate library.
4. `mechanism_family_proposal`: test candidate factors for support locality and
   conditional-normalization patterns without promoting them to targets.
5. `effect_route_proposal`: nominate a query-specific RCT, IV, RD, DiD,
   propagation, proximal, anchored-invariance, or observational-equivalence
   route independently of mechanism-family recovery.
6. `authorized_audit`: open a content-bound authority receipt and untouched
   confirmation units, then run only the tests licensed by that receipt.
7. `partial_structure_report`: return certified statements, proposal-only
   relations, conflicts, and unresolved components as different types.
8. `next_query`: rank a missing corner, new tilt, new measurement, negative
   control, or authority document by expected separation of surviving models.

No proposal artifact can be converted into an audit verdict. A later audit must
resolve its own data, unit, design, seed, selection, and premise evidence.

## Six distinct identification layers

### L0. Data and authority isolation

Inputs:

- immutable source bytes and SHA-256 fingerprint;
- stable column and value encodings;
- declared or explicitly unverified dependence unit;
- caller-supplied deterministic split seed;
- time/block contract when rows are dependent in time or space;
- neutral discovery view with study identity, codebook semantics, assignment
  law, published answers, confirmation outcomes, and oracle withheld;
- separately mounted, content-addressed authority receipt;
- sealed confirmation view and sealed oracle.

Hard failures:

- identifier normalization changes a regime or unit token;
- a unit spans incompatible regimes;
- a unique row identifier is presented as assignment-unit evidence;
- discovery sees confirmation outcomes or oracle metadata;
- a seed is derived from outcome bytes;
- the discovery and authorized/blind conditions do not consume identical
  discovery bytes and unit partitions;
- a premise evidence reference does not resolve to the bytes it names.

### L1. Algebra of changes

For a reference law `p0`, define the environment transport
`H_e(x) = log(p_e(x) / p0(x))`. Candidate recurring primitives satisfy
`H = A Phi` under a fixed reference convention.

Allowed conclusions:

- descriptive transport rank;
- complete-cube or complete-parallelotope geometry when every vertex is
  observed and edge correspondence is recoverable;
- estimable lack-of-fit contrasts on partial designs;
- missing versus observed-but-dropped corners;
- mixed finite-difference curvature as an obstruction to the proposed
  context-independent additive factors;
- raw product normalizer residuals;
- held-out prediction of an environment law.

Forbidden conclusions:

- low rank implies causality;
- a convenient quadruple is a causal square;
- null-space flatness on a partial design is a modular certificate;
- zero curvature establishes locality, normalization, a target, or a DAG;
- a scalar response interaction is density curvature.

Identifiability boundary:

- known full-column-rank incidence identifies algebraic factors pointwise;
- a complete unlabeled nondegenerate cube identifies axes only up to its cube
  gauge; a marked factorial zero corner fixes origin and edge signs, while
  coordinate permutation remains;
- a partial unlabeled point cloud retains general factor-mixing ambiguity;
- nonbinary dosage coefficients need a separately normalized parametric
  mechanism path.

### L2. Causal-family proposals

For each factor, distinguish two support semantics:

- `regime_information_support`: the smallest variables sufficient to predict
  the environment or represent its ratio;
- `marginal_shift_set`: coordinates whose marginal law changes under a
  contrast.

Relations across these support types are invalid. Equal response signatures do
not establish equal targets, and inclusion among local family supports is not a
causal order.

Candidate family checks:

- stable support across discovery folds and learner families;
- a fixed common cohort across every support and deletion comparison;
- conditional-normalization candidates
  `E0[exp(phi) | F without v] = 1` under equivalence bounds;
- explicit ratio/minimal-support faithfulness premise;
- explicit deletion-faithfulness premise;
- external, content-bound grouping for repeated tilts of one target;
- correct assignment/cluster unit and simultaneous inference.

An exactly one-pass deletion pattern is only consistent with a target after
credible single-target semantics and deletion faithfulness are separately
established. Coordinated changes can produce one pass. Zero, multiple, or
undetermined passes always abstain.

### L3. Graph and reachability assembly

Family peeling may run only when the input groups have an independently
justified one-family-per-target interpretation. It returns:

- recovered target and parent candidates;
- a partial DAG;
- a stuck remainder and exact reason;
- a separate baseline-Markov factorization check.

The response-set route is different. For an externally established single-
target group `t`, let `R_t` be the union of variables whose marginals change
under at least one admissible tilt. Reverse proper inclusion can support an
ancestry proposal only with all of:

- distinct independently established targets;
- self-response;
- no off-target effects, selection, interference, or measurement drift;
- equivalence-bounded nonchange;
- declared response-completeness/tilt-diversity if absence is used;
- proper inclusion, not equality.

Equal response sets mean indistinguishable reachability, not a two-cycle.
Transitive reduction is not adjacency in the presence of hidden nodes.

### L4. Query-specific effect routes

Effect identification and mechanism-family identification use separate typed
authority ladders.

| Route | Rows may nominate | Authority additionally requires | Honest target |
|---|---|---|---|
| Recorded randomization | assignment contrast | assignment provenance, true unit, delivery/consistency, selection contract | offer/assignment ITT |
| Encouragement IV | instrument, exposure, outcome tuple | independence, exclusion, relevance floor, monotonicity, consistency, unit | complier LATE/CACE |
| Regression discontinuity | running variable and cutoff | documented rule, continuity class, no concurrent threshold process, no manipulation | local cutoff effect |
| DiD / interrupted series | adoption or interruption pattern | defensible counterfactual trend/sensitivity class, no anticipation/spillover, stable composition | cohort-time or interruption effect |
| Anchored invariance | environment associated with one input | exogeneity, exclusion/no bypass, no hidden confounding or selection, positivity, relevance | direction within the declared two-candidate anchored class |
| Temporal propagation | source, time, downstream trace | verified actuation/delivery, clocks, latency, no bypass/common drive/interference | ancestry or path response, usually not adjacency |
| Observational CI | separating sets and colliders | Markov, faithfulness, acyclicity, causal sufficiency/no selection or an explicit latent model | CPDAG or PAG, never forced completion |
| Negative-control/proximal | proxy role tuples | proxy exclusions, relevance, completeness, stable bridge, positivity | bridge-identified effect under the proxy model |
| Restricted structural | ANM, non-Gaussian, linear-shift, equilibrium model | the route-specific functional and noise assumptions plus falsifiers | only the route-specific estimand |

Rows can reject observable implications. They cannot prove IV exclusion, RD
counterfactual continuity, DiD parallel counterfactual trends, proximal
completeness, or the physical meaning of an actuator.

### L5. Active measurement and experiment design

Every unresolved report should propose one of:

- collect a never-observed factorial corner;
- replicate an observed-but-under-supported corner;
- run an asymmetric second tilt of a declared target to separate multiple
  deletion passes;
- add a candidate measurement block and test the nested curvature-conservation
  identity on held-out discovery subfolds;
- obtain a missing unit, selection, assignment, timing, delivery, or exclusion
  receipt;
- run a negative control or mirrored actuator;
- increase a weak intervention past a preregistered detection floor;
- collect a combination intervention absent from a single-target archive.

Ranking is by expected separation among explicitly serialized surviving
hypotheses, subject to feasibility, delivery, overlap, and design eligibility.
It never resolves the existing ambiguity by itself.

## Machine-readable isolation contract

The self-driving request must bind:

- schema and engine versions;
- source-table SHA-256;
- neutralized discovery-table SHA-256;
- sealed confirmation-table SHA-256;
- discovery and confirmation unit-list SHA-256 values;
- disjoint, exhaustive unit partition;
- caller-supplied split seed and deterministic fold algorithm;
- unit basis and time/block basis;
- candidate enumeration order and budget;
- common-cohort policy;
- support semantics;
- equivalence margin, detection floor, learner battery, and tie rule;
- complete ordered considered/rejected candidate-library fingerprint;
- authority receipt references, or explicit missing-receipt codes;
- oracle access status and execution receipt.

The scout never opens outer confirmation data. It emits no calibrated interval,
certificate gate, `passed`, `established`, `certified`, or `unique_target`
field. A separate audit process consumes the frozen proposal and confirmation
view.

## Exact adversarial worlds

These are required conformance cases, not optional examples.

- `flat_noncausal_cube`: exact rank-two, globally normalized, curvature-zero
  cube whose factors fail every local conditional-normalization choice.
- `coordinated_unique_pass`: two changed mechanisms produce exactly one
  deletion pass.
- `selection_twins`: identical selected rows and known inclusion rate arise
  from state-independent versus state-dependent selection.
- `iv_sign_twins`: identical `Z,D,Y` rows support +1 under exclusion and -1
  with a direct instrument bypass.
- `rd_sign_twins`: identical cutoff rows support opposite treatment effects
  under different unobserved continuity premises.
- `did_sign_twins`: identical group-time rows support opposite effects under
  different untreated counterfactual trends.
- `faithfulness_cancellation`: a true three-edge Gaussian DAG yields a marginal
  independence that makes a naive CI learner create a false collider.
- `proxy_duplicate`: deterministic copy makes minimal support and target
  nonunique.
- `blind_ohms_law`: voltage-driven and current-driven systems have identical
  blind `V,I` laws and opposite physical directions.
- `latency_reversal`: delayed sensor timestamps reverse the apparent temporal
  order.
- `common_driver_delay`: one signal leads and predicts another without causing
  it.
- `scalar_density_twins`: identical four-corner scalar means arise from flat
  versus curved hidden state laws.
- `three_node_cube`: exact `A -> B -> C` eight-law world exercises factor
  recovery, conditional normalization, peeling, reachability, and mutations.

Any confident arrow or effect sign on an observationally identical blind twin
is a hard failure. Ambiguous cases are scored for correct abstention, not for
guessing the oracle arrow.

## Public-data execution matrix

| Dataset | Immediate executable product | Deliberately unavailable claim | Status |
|---|---|---|---|
| Causal Chambers `lt_interventions_standard_v1` | checksum-verified single-target environment atlas, repeated-strength diagnostics, held-out environment prediction | joint-composition curvature; archive contains no combined target shifts | adapter in progress |
| Oregon Health Insurance Experiment | identical-bytes authority ablation; offer ITT versus coverage IV/LATE routing | coverage ATE without IV premises; commercial use | synthetic contract exists; real research-only runner pending |
| LaLonde NSW | randomized-receipt versus blind-receipt ITT routing; NSW versus CPS selection challenge | assignment authority from balance or a `treat` column | direct public text extract available; receipt adapter pending |
| NCI-ALMANAC | named scalar interaction and held-out dose-surface prediction | MIC density curvature, target orientation, conditional normalization | proposal-only scalar contract shipped; data adapter pending |
| EarthScope PH5 | verified source-action to receiver-trace ancestry and arrival-time diagnostics | receiver-to-receiver causality or subsurface adjacency | bounded adapter pending experiment-specific timing/QC receipt |
| Airfoil Self-Noise | deterministic ingest and held-out configuration prediction | independent experimental units or actuation for every input | bounded adapter pending |
| NutNet N/P/K | protocol-bound randomized-block factorial contract | execution without an accessible licensed row-level extract | access/data receipt blocked |
| Causal Chambers Remote Lab | designed joint interventions for composition | execution without laboratory access | external access blocked |
| GHCN January/July prototype | fixed-panel mean-shift and shuffle-control behavior | elevation-to-temperature causal direction | executed diagnostic; arrow claim rejected |
| Norman Perturb-seq identity extract | eligibility/overlap/selection diagnostics | stable lead curvature figure or product design | executed and killed by fold instability |

Large tables enter this matrix only if they exercise a named contract the
current suite cannot already test. Dataset size or an intuitive arrow is not a
qualification.

## Implementation checklist

### Delivered foundations

- [x] Exact complete-cube modularity theorem and partial-design limitation.
- [x] Stable log-domain curvature and sampling-odds algebra.
- [x] Typed certificate-gate derivation and immutable report authority fields.
- [x] Product-odds gate for GCM and content-reference requirements.
- [x] Equivalence-based deletion orientation with simultaneous provenance.
- [x] Raw normalizer residual reporting.
- [x] Proposal-only unsupervised square atlas with missing versus dropped arms.
- [x] Parsimony-frontier localization primitive and estimator-family disagreement gate.
- [x] Active asymmetric-tilt ranking with complete library fingerprint.
- [x] Many-environment algebra and exact flat-but-noncausal witness in the paper.
- [x] Typed effect-route opportunity map and authority-ablation schemas.
- [x] Browser executes the Rust design/lens/preflight core; static authority
  panels initialize fail closed.
- [x] Scalar-response contract that quarantines NCI-ALMANAC from MIC density claims.
- [x] Finite categorical mechanism-family geometry and all-background rectangle enumeration.
- [x] Two-axis partial-design result: causal completion status is separate from algebraic testability.
- [x] Exact selection-transport identity and diagnostic Gamma-sensitivity interval.
- [x] Cost-aware missing-corner ranking by identified-set reduction.

### Research and estimator tranche

- [x] State the complete finite categorical mechanism-family certificate in the paper and formal spec.
- [x] Define the nonlinear partial-design causal-completion fiber and exact empty/singleton/multiple classification.
- [x] Record counterexamples separating rank, flatness testability, causal feasibility, and uniqueness.
- [x] Implement the full-rank finite-state completion reference for a fixed DAG
  and distinct target assignment: observed-law Markov checks, exact design-image
  feasibility, identified locality, and conditional normalization.
- [ ] Extend the finite-state solver across rank-deficient nonlinear fibers;
  current output conservatively reports the causal completion as unresolved except
  where a Markov or design-image necessary condition already refutes it.
- [x] State only solver properties supported by the finite-state parameterization;
  keep nonparametric finite-sample classification unresolved.
- [x] Make the linear closure cross-fit pooling-honest and corner-stratified;
  export signed and non-cancelling out-of-fold baseline curvature moments while
  retaining `calibrated_test: false`.
- [ ] Specify interventional mechanism dictionary learning with sparse environment codes, anchors, local support, normalization, and label/gauge ambiguity.
- [ ] Prove or delimit dictionary identifiability under sparse combinations and repeated backgrounds.
- [ ] Specify curvature tomography only under an infinitesimal or explicit latent-score factor model.
- [x] Add an exact hidden-sensor conformance world with complete-state flatness,
  omission curvature, rank-one infinitesimal score covariance, and a resolving
  versus irrelevant sensor comparison.
- [ ] Add sampled cluster-level discovery/confirmation splits and verify
  held-out curvature collapse after the nominated sensor is revealed.
- [x] Implement the exact selected-law curvature decomposition and minimum selection interaction needed to explain a null.
- [ ] Add identified curvature intervals using external enrollment fractions, margins, negative controls, or measurement encouragement.
- [ ] Add a separate singular-support intervention route rather than forcing density ratios across disjoint support.
- [x] Implement deterministic tied-main-effect and interaction-augmented
  four-corner joint regime models with explicit sampling offsets and held-out
  logarithmic-loss comparison.
- [x] Add deterministic cluster-level outer folds, equal total weight per
  declared unit, untouched restricted-versus-interaction loss comparison, a
  recorded seed, and a deterministic fold-plan fingerprint for the reference
  closure model.
- [ ] Add adaptive witness separation and calibrated cluster-level inference
  around the global closure comparison; held-out proper-loss advantage remains
  diagnostic until this lands.
- [x] Implement exact finite-state leave-the-entire-combination-arm-out law
  prediction with mandatory raw normalizer/residual, log overlap concentration,
  asymptotic ESS fraction, and nonnegative held-out TV/Hellinger discrepancies.
- [x] Extend held-out combination prediction to a fitted two-stage reference
  path: the primitive API accepts only `00/10/01`; the CLI freezes it before
  opening the separate `11` file; ratios remain in the log domain; global and
  per-fold raw normalizers, cluster ESS, a linear-time held-out proper score,
  separate stage fingerprints, and optional exact weighted energy distance are
  serialized with diagnostic-only authority.
- [ ] Add cluster-resampled full-pipeline refits and calibrated weighted-law
  equivalence/change inference; ordinary row permutations are invalid for the
  cross-fitted weighted empirical law.
- [ ] Calibrate cluster-level type-I error and coverage before adding flexible learner families.
- [ ] Add one controlled-real factorial experiment before making general discovery claims.
- [ ] Add Python, Arrow, Parquet, Polars/pandas, and AnnData surfaces only after the reference estimator is end to end.

### Stage A: isolation and proposal artifacts

- [x] Add closed self-driving-request schema.
- [x] Add private-field, serialize-only shift-factorization proposal types.
- [x] Add support-semantics enum and reject cross-semantics relations.
- [ ] Add explicit `not_run`, `recommended`, `inconclusive`, and `abstained`
  statuses with stable reasons; never use `null` for swallowed failures.
- [ ] Add caller-seeded split-before-discovery orchestration at the declared unit.
- [x] Add immutable request and candidate-library fingerprints; common-cohort execution binding remains open.
- [x] Add confirmation-outcome and oracle isolation contract tests; trusted-harness execution remains open.
- [ ] Add header/candidate-order permutation invariance under a budget.
- [ ] Add row-duplication-within-unit invariance.

### Stage B: many-environment scout

- [ ] Compute environment transport diagnostics under a fixed reference convention.
- [ ] Detect complete cubes/parallelotopes and explicit gauge ambiguity.
- [ ] Report partial-design estimable contrasts and missing primitive corners.
- [ ] Run parsimony-frontier support localization on inner discovery folds.
- [ ] Run a heterogeneous learner battery and emit disagreement, never a vote.
- [ ] Freeze all considered and rejected candidates before confirmation.
- [ ] Compute held-out discovery-subfold law-prediction diagnostics.
- [ ] Never mint product-design evidence from balanced empirical counts.

### Stage C: causal families and graph proposals

- [ ] Add conditional-normalization candidate diagnostics with nonnegative
  discrepancy and simultaneous equivalence bounds.
- [ ] Keep raw pass counts separate from authority-level target verdicts.
- [ ] Require external same-target group receipts.
- [ ] Feed only qualified rich families to peeling.
- [ ] Return a partial/stuck graph and baseline-Markov check.
- [ ] Add separately typed marginal response sets and reachability proposals.

### Stage D: active next-query engine

- [x] Rank missing corners by identified-set reduction and cost; dropped-corner replication remains separately typed.
- [ ] Rank asymmetric tilts for multiple-pass disambiguation.
- [ ] Rank candidate state additions using nested held-out curvature accounting.
- [x] Emit missing authority-contract requests as first-class proposals.
- [ ] Bind every ranking to serialized hypotheses, predictions, feasibility,
  delivery, overlap, seed, and deterministic tie rule.

### Stage E: effect router

- [ ] Separate `EffectIdentificationAuthority` from
  `MechanismFamilyAuthority` in code and schemas.
- [ ] Implement offer-ITT and IV/LATE route contracts.
- [ ] Implement RD and DiD candidate contracts with observational twins.
- [ ] Implement anchored-invariance and temporal-propagation contracts.
- [ ] Implement CPDAG/PAG proposal output without forced completion.
- [ ] Implement negative-control/proximal bridge proposal and ill-posedness audit.
- [ ] Keep ANM and other structural assumptions isolated behind adapters.

### Stage F: public adapters

- [ ] Causal Chambers fetch, checksum, normalize, atlas, and benchmark.
- [ ] Oregon real-data research-only runner and identical-byte ablation.
- [ ] LaLonde NSW fetch/hash and randomized-versus-blind receipt twin.
- [ ] NCI-ALMANAC scalar response fetch/normalize/predict adapter.
- [ ] EarthScope bounded active-source fixture and timing/QC receipt.
- [ ] Airfoil bounded table adapter and configuration-block prediction.
- [ ] NutNet contract scaffold and explicit access blocker.
- [ ] Mirrored voltage/current actuator receipts and blind-law fixture.

### Stage G: product surfaces and validation

- [ ] Expose `atlas -> request -> scout -> audit -> structure -> next` in the CLI.
- [ ] Add schema versions, semantic cross-file validators, and negative fixtures.
- [ ] Bind browser WASM source/lock/artifact fingerprints and fail closed on load.
- [ ] Remove every authoritative JavaScript fallback.
- [ ] Add report rendering for proposal, abstention, conflict, partial structure,
  identified effect, and next-query outputs.
- [ ] Update paper, README, formal/inference/autonomous docs, examples, and manifest.
- [ ] Run formatting, strict clippy, all tests, repository checks, simulations,
  and LaTeX build.
- [ ] Obtain independent mathematical and authority review.

## Completion criterion

The campaign is complete only when a user can point the CLI at a large table
and receive a deterministic, replayable artifact that does all of the following:

- discovers candidate changes without reading confirmation outcomes;
- says which causal/effect routes are structurally possible;
- states which premises are verified, externally asserted, empirically checked,
  refuted, or absent;
- runs only licensed tests at the correct dependence unit;
- returns partial truth and explicit abstention rather than a compulsory DAG;
- survives the observational twins and mutation suite;
- recommends the next measurement, intervention, or authority document;
- reproduces from content hashes, schema versions, and recorded seeds.

That is the killer app: not an oracle that always draws arrows, but a system
that automatically extracts every defensible causal implication from a pile of
data and tells the user exactly what would be needed to learn more.
