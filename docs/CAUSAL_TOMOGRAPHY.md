# Causal mechanism tomography

The reference implementation now includes a held-out, common-cohort
state-expansion ranking primitive. Candidate blocks and discovery priorities
must already be frozen. Each candidate supplies coarse and expanded
simultaneous discrepancy bounds, overlap-retaining cohort fractions, a nested
conservation residual, a positive cost, and the same untouched-cohort content
commitment. The ranking distinguishes equivalence under supplied bounds,
partial explanation, worsening, apparent improvement caused by support loss,
and inconclusive conservation. This is `diagnostic_only`: the primitive cannot
validate the supplied intervals, cohort receipt, or upstream block selection.

## North star

The self-driving objective is not to fit a DAG to one stationary table. A single
observational law generally does not contain enough information to distinguish
causal direction. The objective is to learn from a **family of changes**:

> Discover recurring distribution shifts, factor them into candidate autonomous
> mechanism changes, infer the partial order in which those changes propagate,
> test whether they compose, and request the cheapest new environment or
> measurement that separates the surviving explanations.

This is a proposal-and-falsification system. It may produce a ranked causal
model, but it may issue a certificate only when the ordinary locality,
conditional-normalization, flatness, orientation, selection, overlap, and
randomization-unit contracts are independently satisfied.

The central scientific bet is that causality reveals itself through **stable
modular generative mechanisms plus asymmetric response under perturbation**.
Weather, gene perturbations, industrial controls, policies, sites, and time
segments are possible sources of environments. None is causal merely because it
is named an environment.

## Environment algebra

Let `e` index environments and let `p_e` be their laws on the measured state
`X`. Relative to a reference environment `0`, define the log-law transport

```text
H_e(x) = log p_e(x) - log p_0(x).
```

If all environments share a DAG and environment `e` changes only a subset of
its causal conditionals, then

```text
H_e(x) = sum over changed nodes v of delta_e,v(x_v, x_pa(v)).
```

Each term is local to one causal family and its ratio form is conditionally
normalized over its target. Sparse environment changes therefore produce a
block-sparse functional decomposition of the log-law family. This is the
functional analogue of the sparse mechanism shift hypothesis; arbitrary
low-rank factorization is not enough.

It is useful to view the observed environment family as a discrete connection:

- environment laws are vertices;
- log-density ratios are edge transports;
- a repeated primitive produces the same edge transport across backgrounds;
- locality gives an edge transport a minimal state support;
- conditional normalization distinguishes a candidate target inside that
  support;
- square curvature is the mixed finite-difference integrability obstruction;
- flat loops mean the result is path-independent and the primitive changes
  compose;
- state expansion asks which missing measurement makes a curved connection
  flatter.

This geometric language does not add authority. It organizes the exact algebra
already implemented by mechanism interferometry.

## What is identifiable, and from what

### Arbitrary heterogeneous laws: nothing causal

Without restrictions on how environments differ, every `p_e` can be described
as an environment-specific global mechanism on the whole state. A matrix or
tensor factorization of `H_e` is non-unique under invertible mixing of its
factors. Sparsity, low rank, neural disentanglement, or a compact code length do
not by themselves turn those factors into causal mechanisms.

### Where directional information can enter

Causality is not one statistical asymmetry. It is the claim that some part of a
generative system can be changed while other mechanisms retain their meaning,
and that the resulting response transports across contexts. A general-purpose
system should therefore search for several distinct **sources of directional
information**, preserve their different premises, and intersect their surviving
model classes:

| Source | Observable leverage | What supplies direction | No-claim boundary |
|---|---|---|---|
| Delivered action or natural experiment | randomized offers, cutoffs, outages, controller commands, policy timing | externally bound assignment or boundary-condition semantics | rows may discover the opportunity but cannot manufacture the assignment contract |
| Observational CI geometry | separating sets, unshielded colliders, and compelled orientations | Markov, faithfulness, acyclicity, and either causal sufficiency/no selection or an explicit latent-variable model | return a CPDAG or PAG; never force reversible or undetermined edges into a DAG |
| Recurrent environment shifts | sparse changes repeated across backgrounds, invariant conditionals, flat combinations | stability of autonomous mechanisms plus sufficiently diverse shifts | arbitrary low rank or environment predictability is not a causal factorization |
| Temporal/spatial propagation | known source time and location, response fronts, impulse recurrence | an exogenous source and verified forward paths with calibrated sensor latency | lag prediction alone gives neither direction nor adjacency |
| Local mechanism algebra | conditional normalization, deletion equivalence, common support, curvature closure | existence of a localized normalized replacement and, separately, target semantics | one deletion pass cannot establish that only one mechanism changed |
| Specialized structural model | additive noise, linear shift interventions, non-Gaussianity, proxy bridges, monotone dynamics | an explicit restricted model class with its own falsifiers | model-family agreement is sensitivity evidence, not universal causal authority |
| Active disambiguation | a newly randomized tilt or measurement chosen to separate surviving models | intervention delivery and an independently collected confirmation law | acquisition scores never become evidence about the data used to choose them |

These routes are complementary. Joint Causal Inference makes context variables
explicit; invariant prediction searches for predictors whose response mechanism
survives environments; sparse-mechanism methods exploit the number and diversity
of changed conditionals; and BackShift identifies a linear cyclic system from
covariance changes under a particular unknown-shift model. Each can be a scout
adapter. None is silently promoted into the complete-cube MIC theorem, and a
restricted-model result carries its model class in the answer type.

Observational conditional-independence geometry is a distinct route rather than
a special case of a functional/noise asymmetry. With a perfect CI oracle,
acyclic DAG semantics, the causal Markov and faithfulness assumptions, causal
sufficiency, and no selection, it identifies a Markov equivalence class: the
skeleton, unshielded colliders, and orientations compelled by acyclicity. The
honest output is a CPDAG, not a completed DAG. With latent confounding or
selection, an explicitly chosen FCI-family model instead targets PAG-level
ancestral information.

Faithfulness is load-bearing. Let independent standard Gaussian noises generate
`A = eps_A`, `B = A + eps_B`, and `C = B - A + eps_C`. The true graph contains
`A -> B`, `B -> C`, and `A -> C`, yet the coefficients cancel so that `A` and
`C` are exactly independent. A CI procedure that treats that independence as a
missing edge sees the path `A - B - C` and can orient the false collider
`A -> B <- C`, deleting one true edge and reversing another. Near-cancellation,
uncertain CI decisions, hidden variables, and selection therefore remain typed
abstention or partial-orientation conditions rather than small confidence
penalties.

This yields a practical definition of “self-driving”: inspect a pile for all
available sources above, freeze a library of candidate questions on discovery
units, kill candidates whose observable implications fail, and return the
strongest still-licensed claim. The output may be an oriented mechanism family,
an ancestor relation, a query-specific effect, an equivalence class, or a request
for one missing action or measurement. Refusing to output a total DAG is correct
when the information needed to choose among observationally equivalent worlds is
not present.

### Sparse stable mechanisms: a graph-scoring signal

Suppose a common causally sufficient DAG generates all environments and only a
small subset of its conditionals changes in each environment. A proposed graph
can be scored by the number of conditional mechanisms it must declare changed.
With sufficiently diverse sparse shifts, the true graph can become identifiable
under additional regularity assumptions. This is the setting of the
[Sparse Mechanism Shift](https://papers.nips.cc/paper_files/paper/2022/hash/46a126492ea6fb87410e55a58df2e189-Abstract-Conference.html)
line of work. Mechanism interferometry adds an exact question that a scalar
shift score does not answer: do proposed primitive changes actually compose on
observed combination regimes?

### Complete unlabeled cubes: recoverable environment axes

There is one important unlabeled exception to generic factor ambiguity. If all
`2^K` laws of an exact flat cube occur and the `K` primitive log potentials are
linearly independent as functions, the laws are the vertices of a
parallelotope in function space. Its edge directions identify the primitive
axes up to coordinate permutation and independent coordinate bit complements.
An externally identified reference corner fixes the complements. Partial point clouds do not inherit
this result: without a complete cube or known single-change adjacency, an
invertible mixing of factors is generally observationally equivalent.

Even a recovered axis is not yet causal. Its minimal coordinate support must be
unique, and one coordinate must satisfy the nonlinear conditional-normalization
identity. Exact low rank, globally normalized laws, and zero curvature can all
hold while every proposed primitive fails conditional normalization for every
possible target.

### Interventional dictionary learning: identified cases and the hard wall

A transport-dictionary candidate relative to an arbitrary reference law is a
factorization

```text
H_e(x) = b(x) + sum_j A_ej phi_j(x)
```

under one fixed density-ratio reference convention. Here `b` transports the
reference law to the factorial zero law; it is zero only when those laws
coincide. Its columns are recurring
primitive transports, not generic latent features. A column may be active in
several backgrounds only when it denotes literally the same replacement. Two
alternative guides, doses, or implementations of one target conditional are
categorical levels of one mechanism family; they are not additive columns that
may be coactivated unless an additional normalized parametric path supplies
that semantics.

Three population cases are exact:

1. **Known full-rank codes.** If `A` is externally known and `[1,A]` has full
   column rank, a left inverse identifies `b` and every `phi_j` pointwise. If
   the density-ratio reference is the marked zero-code law, then `b=0` and
   full column rank of `A` suffices. A known factorial baseline
   plus one externally justified pure environment per column is the simplest
   instance: `phi_j = H_ej - H_e0`. If the recovered factors are linearly independent,
   every other environment has at most one real coefficient vector, and
   therefore at most one binary code, in their span. If the physical anchor
   amplitude is unknown, the observed anchor difference fixes a unit scale by
   convention; algebra alone identifies only its ray, and arbitrary rescaling
   need not remain conditionally normalized.
2. **Complete unlabeled binary grid.** The parallelotope result above recovers
   axes and codes only up to the stated cube gauge. Marking the factorial zero
   vertex fixes origin and edge signs; coordinate permutation remains.
3. **Incomplete unknown codes.** No generic identification follows from low
   rank or sparse-looking fits. For any invertible `T`, `A Phi = (A T)
   (T^-1 Phi)`. In the smallest two-row witness, choose linearly independent
   transports `h1,h2`; they admit both
   factors `(h1,h2)` with codes `(10,01)` and factors `(h1,h2-h1)` with codes
   `(10,11)`. Both exactly reproduce the laws but predict different missing
   fourth laws. A sparsity preference can rank
   them, but cannot establish which intervention vocabulary generated them.

Repeated backgrounds add a falsifier, not automatic identification. Once two
environment edges have been independently justified as the same primitive,
their transport fields must agree and their observed combination loops must
close. Discovering parallel edges by searching the same point cloud and then
using that parallelism as confirmation is circular. Unknown-code
identification outside the complete-grid case therefore needs explicit anchor
or separability assumptions, a theorem that makes sparse codes unique, and an
untouched combination or background on which the frozen dictionary predicts a
law. Those conditions are research targets, not current software claims.

After algebraic recovery, every factor still has to pass the minimal-support,
conditional-normalization, unit, selection, and held-out closure audits. The
flat rank-two noncausal cube is the locked adversary: a dictionary learner may
recover its two axes, but it must reject every causal-family interpretation. A
deterministic proxy is a second adversary: if `Z=X`, minimal support may be
written in either coordinate and target identity is not unique. The proposal
must serialize the surviving gauge/equivalence class rather than select a
convenient representative and call it a mechanism.

The complete-cube result does not automatically extend to unlabeled
multi-level grids. The same four laws `{0,f,g,f+g}` can be represented as two
binary families or one four-level categorical family. Family arities and
same-family level grouping therefore require metadata or a separate
indecomposability theorem. No such automatic grouping is claimed here.

### Repeated tilts: familywise target orientation

Consider several laws that differ from a reference law only by distinct
replacements of the same target conditional, localized to a common family `S`.
For every tilt, deleting the true target leaves `X_(S without target)` invariant.
Consequently, the true target lies in the intersection of all deletion-pass
sets. If, for every other variable in `S`, at least one sufficiently rich tilt
changes the marginal left after deleting that variable, the intersection
contains exactly the target.

This **universal-deletion** result replaces per-tilt faithfulness with a weaker
familywise diversity condition. It still assumes that the grouped laws are
single-target replacements of the same mechanism. Shared support, similar
scores, or a common deletion pass may propose that grouping; none establishes
its intervention semantics. Factorial background replication and successful
held-out combination prediction are stronger checks because they can falsify a
wrong grouping.

### Anchored shifts: direction inside a restricted model class

Let an externally justified anchor `E` affect `X`, let every path from `E` to
`Y` pass through `X`, and exclude hidden `X`-`Y` confounding and selection. With
positivity and anchor relevance, invariance of `P(Y | X, E)` across environments
combined with certified change of `P(X | Y, E)` identifies `X -> Y` inside that
anchored two-candidate class. Failure of the forward invariance rejects the
anchor model; it does not automatically establish the reverse arrow.

The assumptions are load-bearing. A direct `E -> Y` bypass can make the wrong
conditional exactly invariant, and hidden confounding can make the correct
conditional change. The scout must serialize the exclusion, relevance,
selection, overlap, and measurement-stability status rather than hide them in a
score.

### Propagation: ancestry before adjacency

In a time-unrolled system, a verified exogenous action at time `t` whose paths to
an outcome at `t+h` all pass through `X_t` can establish that `X_t` is an
ancestor of the later outcome. It does not establish a direct edge. Common
drivers with different delays, clock offsets, sensor latency, feedback, and
unmeasured fast paths can all reverse a cross-lag ranking. Unknown latency is an
abstention condition, not a nuisance to average away.

### Natural experiments: discover contracts, not just arrows

A second route to useful causality is **identification-opportunity mining**. A
large table may contain an assignment boundary, lottery, encouragement,
staggered policy, controller action, outage, or other shock that identifies a
particular effect without identifying the whole graph. The autonomous product
should find those opportunities and emit a frozen, query-specific contract.

| Candidate design | What rows can suggest | Premise that rows cannot establish alone | Legitimate target | Cheap kill |
|---|---|---|---|---|
| Recorded randomization | treatment probabilities, balance, delivery and noncompliance | that the recorded generator really governed assignment and the unit was not rerandomized downstream | offer or assignment ITT; with IV premises, a separately named complier LATE/CACE rather than a generic treatment ATE | pre-treatment imbalance by batch, delivery failure, cluster mismatch |
| Regression discontinuity | a treatment jump at a known running-variable cutoff | continuity of potential outcomes and absence of precise manipulation at the cutoff | local effect at the cutoff | sorting, covariate discontinuities, donut sensitivity, placebo cutoffs |
| Instrument or encouragement | a relevant exogenous-looking variable that shifts treatment | exclusion, independence, monotonicity, and a defensible intervention meaning | a local complier effect, not a global edge | direct outcome response, weak first stage, negative-control failure |
| Difference in differences | staggered adoption and repeated pre/post outcomes | parallel counterfactual trends, no anticipation, and stable composition | cohort- and time-specific treatment effects | pre-trends, pseudo-adoption dates, spillovers, changing measurement |
| Interrupted time series | a sharp action time with a long pre/post record | no concurrent shock, stable sensor semantics, and correctly modeled dependence | local dynamic response to the interruption | unaffected-series placebo, alternate break dates, latency instability |
| Negative-control or proximal bridge | a treatment, outcome, and treatment- and outcome-inducing proxy candidates | proxy exclusion semantics, relevance, completeness of the conditional-expectation operator, and bridge stability | an effect despite an unmeasured confounder, within the proximal model | no held-out conditional-moment solution, weak operator spectrum, forbidden proxy response, proxy-pair disagreement |

This creates a **causal-opportunity atlas** alongside the mechanism atlas. Its
nodes are typed roles such as unit, action, environment, running variable,
cutoff, time, outcome, candidate instrument, and negative control. Its edges are
proposed identification contracts, not causal edges. Strategy selection happens
on discovery units; estimation happens only after the contract, transformation,
candidate library, and unit split are frozen. Several strategies may apply, but
agreement is corroboration rather than a vote and disagreement is a first-class
finding.

More data improves overlap, power, environmental diversity, and the chance of
seeing an informative natural experiment. It does not make exclusion,
no-selection, or counterfactual continuity empirically provable. The genuinely
self-driving behavior is to discover the strongest available question, test all
observable implications of its design, state the remaining premise, and propose
the cheapest observation or intervention that would break the ambiguity.
Passing a cheap kill is serialized as `not_falsified`, never
`premise_verified`.

## The self-driving loop

### 1. Unit and environment discovery

Identify the coarsest plausible independent unit before any split. On discovery
units only, inventory explicit environment metadata and propose latent
environments through change points, recurring temporal states, site/device
groups, or independently recorded actions. Fingerprint the complete candidate
library. A partition learned using confirmation outcomes is not a confirmation
partition.

In parallel, inventory action probabilities, thresholds, adoption times,
encouragements, outages, and controller commands for the causal-opportunity
atlas. Empirical balance may nominate a randomized design but may never mint its
assignment contract; an independently recorded generator, policy rule, or
boundary-condition receipt supplies that authority.

### 2. Environment geometry

Build a graph over candidate environment laws. Estimate held-out discrepancies
and overlap for its edges. Search for approximate parallelograms and cubes in
log-law space: four environments are a candidate compositional square when

```text
H_11 - H_10 - H_01 + H_00
```

is small as a field, not merely after an arbitrary projection. Never interpret
empirical arm frequencies as known assignment odds.

### 3. Transport localization

For every well-supported edge, locate the smallest stable state support that
preserves its environment-prediction signal. Use parsimony frontiers across
cluster-level splits and deliberately different learner families. The result is
a candidate causal family, not a parent set. Proxy variables, descendants, and
hidden common causes are expected failure modes.

### 4. Mechanism vocabulary induction

Group edge transports using several falsifiable properties:

- common minimal support;
- a shared candidate conditional-normalization coordinate;
- agreement across estimator families and held-out units;
- repetition across unrelated backgrounds;
- flat observed squares when two proposed groups combine;
- accurate prediction of a held-out combined environment, including the raw
  compositional normalizer residual.

This turns many environment-specific changes into a smaller vocabulary of
reusable mechanisms. The compression claim is evaluated out of sample: a true
vocabulary should explain new environments without adding an epicycle for each
one.

### 5. Orientation by strategy contract

Run all applicable strategies, but do not collapse them into an uncalibrated
vote:

- universal deletion over repeated same-target tilts;
- conditional normalization of localized ratios;
- an exogenous-anchor invariance test;
- verified temporal or spatial propagation;
- randomized-actuator response;
- domain constraints fixed before confirmation.

Each strategy has its own premises and falsifiers. A report distinguishes
`candidate_forward`, `candidate_reverse`, `conflicted`, and `undetermined`.
Certificate-path types such as `CertifiedInvariant` and a premise-bearing
`OrientationVerdict::Established` must not appear in a proposal artifact.

### 6. Response tomography and graph assembly

For each proposed primitive, estimate both its **mechanism-change set** and its
**distribution-response set**. Under a stable causally sufficient DAG, a local
mechanism can remain invariant while its marginal distribution changes because
an ancestor moved. With suitable response faithfulness, response-set inclusion
reveals parts of the ancestor order. Assemble only the partial graph forced by
oriented families and verified ancestry. Never force acyclicity by deleting the
least convenient edge; a cycle or stuck peel is a finding.

Do not confuse either set with the **regime-information support** returned by a
parsimony frontier. A response set contains changed marginals; an information
support contains variables sufficient to discriminate environments; a
mechanism-change set names changed causal conditionals. Every serialized set
must carry its support semantics, and inclusion may not compare unlike types.

More precisely, let `R_t` be the union of variables whose marginal laws change
under a correctly grouped, admissible family of tilts of target `t`. Under
single-target modular interventions without selection, interference, or
off-target effects, `R_t` is contained in the descendants of `t`. If every
target responds to its own tilts and `R_v` is a proper subset of `R_u`, then `u` is an
ancestor of `v`: `v` belongs to `R_v`, hence to `R_u`. If tilt diversity is
response-complete, these reverse inclusions recover the reachability partial
order. Transitive reduction is justified only when hidden nodes and bypasses
have also been excluded.

Equal response sets for distinct targets are indistinguishable, not two reverse
ancestry claims. They create a tied preorder class and block strict orientation
until another tilt or measurement separates them.

This criterion is powerful but fragile. Marginal cancellations can hide a
descendant; off-target delivery can add a non-descendant; and unverified
same-target grouping makes the union meaningless. Every nonchange therefore
requires equivalence bounds, and response completeness is a declared diversity
assumption rather than a consequence of sample size. Similar response sets may
propose a grouping but can never establish one: distinct targets can have the
same downstream responders.

### 7. Composition audit

Test whether independently inferred primitives predict observed combinations.
Report density curvature, conditional design covariance only on an eligible
product design, overlap, support loss, and the raw normalizer residual. Pairwise
flatness is a stronger validation of a learned mechanism vocabulary than
training reconstruction loss because it tests a new regime law.

### 8. State and representation expansion

For unexplained curvature, add candidate measurement blocks on untouched folds
and evaluate the nested curvature identity. A block that makes curvature flatter
and recovers the missing conditional covariance is a candidate explanatory
measurement. If no recorded block works, emit a missing-sensor proposal. An
invertible or design-sufficient representation may be used; a representation
trained to minimize curvature may not certify itself.

### 9. Active causal design

Choose the next intervention, environment, or measurement to maximize the
worst-case separation among surviving causal models subject to delivery,
support, cost, randomization-unit, and product-design constraints. Candidate
generation remains `proposal_only`; new independently collected data must pass
preflight again.

## Typed promotion ladders

Mechanism-family recovery and query-specific effect identification are separate
products. Evidence for one may never be coerced into the other.

### Mechanism-family authority

| Level | Claim | Minimum evidence | Authority |
|---|---|---|---|
| 0 | environments differ | held-out two-sample change with overlap | diagnostic |
| 1 | recurring change family | stable localized transports across discovery splits | proposal only |
| 2 | reusable mechanism candidate | background repetition plus held-out composition/prediction | proposal only |
| 3 | directed family candidate | one satisfied identification strategy and no fired strategy-specific falsifier | proposal only |
| 4 | audited oriented family | independent confirmation, selection/unit contracts, equivalence bounds, locality and normalization | audit |
| 5 | modular multi-mechanism model | Level 4 families plus complete eligible curvature/closure audit | certificate |

### Effect-identification authority

| Level | Claim | Minimum evidence | Authority |
|---|---|---|---|
| E0 | candidate opportunity | row pattern plus a frozen strategy proposal | proposal only |
| E1 | design not falsified | assignment/unit receipt and all observable design checks pass | proposal only |
| E2 | identified estimand contract | query, estimand, assignment rule, units, selection and untestable premises are externally frozen | audit eligible |
| E3 | audited causal effect | E2 plus independent estimation, uncertainty, overlap, falsifiers and sensitivity analysis | audit |

An E3 ITT, LATE, RD, or proximal effect is not an oriented causal family and
does not imply a global graph. Conversely, a localized mechanism candidate does
not identify a policy effect without the appropriate estimand contract.

No amount of Level 1 evidence may be renamed Level 4. The purpose of the
self-driving system is to reach the highest justified level automatically and to
say exactly what observation would unlock the next one.

## Adversarial conformance worlds

Before a public-data claim, the scout must pass prediction-locked worlds that
exercise its assumptions rather than its preferred answers:

1. **Autonomous chain:** nonlinear `X -> Y -> Z`, multiple distinct tilts per
   node, factorial combinations, and held-out combinations. Recover the local
   families and universal-deletion targets; predict flat combinations.
2. **Coordinated two-mechanism shift:** construct a unique deletion pass even
   though two mechanisms changed. The generic scout must abstain without a
   single-target contract.
3. **Hidden parent:** omit a parent so full-state flatness becomes curved or
   orientation becomes multiple-pass. The system must recommend state expansion,
   not reverse an edge.
4. **Selection twin:** two source/selection models produce the same selected
   laws. Rows alone must not distinguish them.
5. **Mirrored actuator:** voltage-driven and current-driven circuits have the
   same passive relation `V = R I` but opposite actuation direction. With actuator
   metadata, return opposite directions; with it hidden, abstain on both.
6. **Direct-anchor bypass:** an anchor changes both candidate variables and
   makes the wrong conditional invariant. The exclusion gate must block.
7. **Common-driver delay and sensor-latency reversal:** cross-lag prediction
   points the wrong way. Unknown timing metadata must block ancestry.
8. **Non-product sampling:** conditional covariance and density curvature
   disagree. The GCM route must refuse.
9. **Estimator disagreement:** two reasonable nuisance families orient
   differently. Emit conflict, never a normalized vote.
10. **Null apparatus:** context labels are shuffled and all candidate
    environments collapse. Return no direction.
11. **Identification twins:** observationally identical RD, IV, and
    difference-in-differences tables have opposite causal effects because the
    continuity, exclusion, or parallel-trend premise is changed. The router may
    nominate the same strategy on both, but without the external premise it must
    not name the sign.

The primary loss is a wrong arrow. Among systems with the same wrong-arrow
rate, fewer abstentions and cheaper follow-up proposals are better.

## Relationship to adjacent work

This program should interoperate with, and be compared against, methods designed
for heterogeneous data rather than claim the territory is empty:

- [CD-NOD](https://arxiv.org/abs/1903.01672) uses independent changes in
  nonstationary or heterogeneous data for skeleton and direction discovery.
- [Sparse Mechanism Shift](https://papers.nips.cc/paper_files/paper/2022/hash/46a126492ea6fb87410e55a58df2e189-Abstract-Conference.html)
  scores graphs by the sparsity of changing conditionals.
- [BaCaDI](https://proceedings.mlr.press/v206/hagele23a.html) jointly infers a
  graph and unknown intervention targets in a Bayesian model.
- [Score-based causal representation learning](https://jmlr.org/papers/v26/24-0194.html)
  proves intervention-based latent-variable identifiability results, including
  settings where same-target intervention pairs are not labeled.
- [Learning unknown intervention targets](https://proceedings.mlr.press/v238/yang24d.html)
  explicitly distinguishes the causally sufficient and latent-confounded cases.

Mechanism interferometry's proposed niche is the auditable algebra between these
steps: exact closure tests on combinations, raw-normalizer and conservation
checks, familywise deletion, curvature-driven measurement search, and a typed
failure boundary that keeps a useful scout from impersonating a certificate.
