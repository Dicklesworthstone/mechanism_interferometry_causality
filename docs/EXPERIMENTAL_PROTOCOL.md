# Experimental Protocol

## 1. Feature-flag pilot

### Goal

Demonstrate a pair with strong nonlinear outcome interaction but flat full-state mechanisms, and a second pair whose curvature is explained by an omitted shared-resource measurement.

### Design

- Choose 4 to 8 flags with known implementation boundaries.
- Use independent product randomization at the deployment unit.
- Reserve negative-control pairs from physically separate modules.
- Reserve selected pair corners as held-out compositional tests.
- Record both assignment and actual-delivery status.

### State

Collect local module outputs, queue depths, CPU/memory pressure, request topology, latency quantiles, retries, errors, downstream outcome summaries, and batch/deployment identifiers. Predeclare candidate state-expansion blocks.

### Analysis

1. validate product assignment and inclusion;
2. freeze any domain or passive-model proposals before opening confirmatory deployments;
3. run Design-IAMB for every primitive flag;
4. orient candidate families by simultaneous deletion equivalence;
5. run moment screening and wGCM curvature tests;
6. fit a joint multinomial diagnostic model;
7. predict held-out pair distributions from singles;
8. use state expansion on reproducible curvature;
9. require negative-control curvature below the implementation floor.

### Success criteria

- exact or approximate square closure for negative controls;
- at least one strong outcome interaction with curvature equivalent to zero;
- one intentionally shared-resource pair with detectable curvature that shrinks after adding the resource state;
- calibrated held-out distributional prediction on flat pairs.

If orientation has multiple certified deletion passes, the current run abstains. A proposal adapter may rank feasible follow-up replacements by worst-case predicted separation among the surviving hypotheses. The selected replacement must preserve the same primitive target and is evaluated in a new independently randomized deployment; its acquisition score is never reused as evidence.

## 2. Perturb-seq study

### Unit of inference

Biological replicate, construct, or batch as dictated by assignment. Cells are measurements, not independent interventions.

### Controls

- alternate guides per target;
- guide-order swaps;
- non-targeting and duplicated controls;
- expression/knockdown measurements;
- dose and multiplicity matching;
- construct-level random effects or clustered resampling.

### Primary comparisons

Cross-classify gene pairs by conventional epistasis and full-distribution curvature. Test the hypothesis that same-complex or same-pathway pairs are enriched for curvature only after implementation effects are controlled. Identify flat pairs with strong phenotype synergy as the clearest demonstration of the conceptual distinction.

### Compositional prediction

Train on control and singles; predict paired cell-state distributions using self-normalized primitive ratios. Compare against CPA-style baselines and report where latent additivity succeeds or fails relative to the flatness audit.

Passive pathway graphs, expression-based DAG learners, and residual heuristics may prioritize candidate families only on discovery replicates. Alternate guides, guide-order swaps, and held-out biological replicates supply confirmation. Cell-level splitting cannot substitute for replicate- or construct-level separation.

## 3. Proposal-driven audit workflow

When the proposed DAG or family sets are not supplied by a preregistered domain model:

1. allocate discovery and confirmation clusters before fitting any proposal model;
2. generate ranked candidate supports or graphs on discovery clusters;
3. preserve raw scores and their semantics rather than renaming votes as probabilities;
4. freeze the candidate family and multiplicity correction before opening confirmation clusters;
5. run the normal locality, deletion-equivalence, curvature, and composition pipeline;
6. serialize rejected and unresolved proposals as well as selected ones;
7. use unresolved results to propose new measurements or interventions for a future run.

The detailed quarantine contract is in [`PROPOSAL_ADAPTERS.md`](PROPOSAL_ADAPTERS.md).

## 4. Simulation suite

Every release runs:

1. the complete versus Y-only multiplicative response example;
2. the balanced parity multiple-pass orientation failure;
3. the latent conservation example;
4. the implementation-inconsistency example with an exact negative control;
5. product versus non-product corner sampling for the GCM null;
6. partial-design aliasing fixtures;
7. weak-overlap stress tests;
8. state-dependent selection counterexamples.
