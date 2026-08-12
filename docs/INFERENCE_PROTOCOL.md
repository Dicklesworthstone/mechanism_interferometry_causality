# Inference Protocol

## 1. Preflight

Before fitting any model, the program must establish:

1. the regime-conditioned sampling contract;
2. whether pooled corner odds are product;
3. the randomization and clustering unit;
4. whether inclusion is state-independent within regime;
5. common-support and overlap diagnostics;
6. which factorial contrasts are estimable from the observed design.

A strict run stops if any required item is unknown.

## 2. Cross-fitting graph

All learned nuisances are trained out of fold. The fold assignment is deterministic from a recorded seed and stratified by regime and cluster. No observation may be used both to construct an adaptive witness and to test that witness.

Recommended layers:

- outer folds: scientific evaluation and adaptive-witness separation;
- inner folds: nuisance tuning;
- cluster bootstrap: uncertainty at the assignment unit.

## 3. Track I: four-law moment functionals

For prespecified witnesses `w_m`, estimate

\[
M_w=E_{AB}[w]-E_A[w\hat r_B]
\]

and its A/B-swapped analogue. Combine symmetrically. Primitive ratios are normalized on held-out baseline data. Report raw and truncated estimates, effective sample sizes, and the sensitivity curve across truncation thresholds.

This track is valid for arbitrary known state-independent regime quotas. It does not require product assignment.

## 4. Track II: GCM and weighted GCM

This track is enabled only after verifying product sampling odds or reweighting to a declared product law. Estimate

\[
\theta_w=E[w(X)(A-m_A(X))(B-m_B(X))]
\]

with cross-fitted `m_A,m_B`. Use studentized GCM for a fixed witness and weighted GCM with a multiplier max statistic for a witness class. For an adaptive witness:

1. estimate `c_hat(X)` or `kappa_hat(X)` on training folds;
2. freeze `w_hat` as a bounded transform of that estimate;
3. test the residual product on an independent fold;
4. rotate folds and aggregate by the prespecified rule.

The held-out loss difference between restricted and unrestricted flexible multinomial models is diagnostic only.

## 5. Localization

Run Design-IAMB with the regime label as the response:

1. forward addition by reproducible cross-fitted proper-score gain or residual-product dependence;
2. backward pruning conditional on the remaining set;
3. repeated splits and stability frequencies;
4. conservative support union for orientation audit.

The output is a candidate Markov boundary with inclusion frequencies and unresolved variables, not a guaranteed causal family.

An ensemble alternative scores many randomly sampled candidate supports by held-out proper regime-prediction loss plus an explicit complexity measure, then keeps the parsimony frontier: every support within a preregistered loss tolerance of the ensemble best, ordered by complexity. By locality, the true support is the smallest support carrying full regime information, so the least-complex frontier member is the preferred localization. The reference primitive is `mic_stats::parsimony_frontier`. Two rules are mandatory: the frontier threshold is computed once over the completed ensemble, never incrementally while results accumulate, and inclusion frequencies are normalized per variable so they remain marginal stability estimates. Frontier membership and inclusion frequencies are recorded in the evidence ledger as localization stability paths.

## 6. Deletion equivalence

For localized support `S`, compute a two-sample U-statistic discrepancy for each deletion. MMD and energy distance are the initial engines. Normalize by the full-support discrepancy:

\[
R_v=D_v/(D_{full}+\eta).
\]

Use a joint multiplier or cluster bootstrap to produce simultaneous lower and upper confidence bounds. With tolerance `epsilon`:

- `upper < epsilon`: certified invariant;
- `lower > epsilon`: certified changed;
- otherwise: undetermined.

The target is returned only if exactly one coordinate is certified invariant and every competitor is certified changed.

When more than one deletion is certified invariant, the report must propose the next experiment rather than force a verdict. Each surviving hypothesis predicts which deletion marginals stay invariant under a further tilt of the same target mechanism. The recommended follow-up tilt maximizes the worst-case predicted separation between surviving hypotheses under the declared discrepancy. The admissible class contains only replacement conditionals for the proposed target; intervening on a different node is a different primitive, not a disambiguating tilt. In the parity fixture, any asymmetric replacement that moves the target's own marginal separates the two hypotheses in one additional regime. Disambiguation always requires confirmatory data from the new regime; the proposal itself is exploratory output.

## 7. Joint multinomial model

Fit one model over all observed design corners with a hierarchical design expansion. Store full logits, intercepts, sampling offsets, and calibrated posterior probabilities. Curvature is always reconstructed from complete posterior odds and the known pooling odds; it is never read from a centered interaction layer alone.

## 8. Moment battery

Run cheap necessary checks first:

\[
E_0\left[\prod_{j\in T}r_j\right]=1.
\]

Record that the scalar pair moment measures a signed tilted mean of curvature and can be exactly blind when primitive marginals are invariant. The running example is a mandatory conformance fixture for this warning.

## 9. Compositional prediction

For held-out combination `T`, form `w_T=prod_j r_hat_j`. Estimate the normalizer `Z_T=E_0[w_T]`, report `Z_T-1`, and evaluate the valid predicted law using `w_T/Z_T`. Compare with held-out combination data using proper scores and two-sample discrepancies.

## 10. State expansion

For each candidate measurement block `W`:

1. preserve the outer evaluation split;
2. refit primitive and interaction fields on `(X,W)`;
3. test equivalence of residual curvature to zero;
4. estimate the covariance term in the nested conservation law;
5. reject candidates that flatten only in sample, reduce design sufficiency, or create selection dependence.

## 11. Multiple testing

All deletion candidates, face contrasts, and witness functions within a declared family use a joint max-statistic procedure. Exploratory scans are labeled exploratory and cannot be promoted to certificate status without a confirmatory split or independent dataset.

## 12. Estimator lens battery

Population curvature functionals are gauge invariant, but every empirical projection inherits the inductive bias of its nuisance learner. Certified projections are therefore repeated across a declared battery of deliberately dissimilar model families, for example a regularized linear model, a kernel or nearest-neighbour method, and a boosted tree or neural learner, each inside its own cross-fitting scheme. The reference gate is `mic_engine::audit_lens_battery`: each family contributes a point estimate and standard error for the same estimand, every pairwise gap is studentized by the combined standard error, and the maximum studentized gap is compared with the policy tolerance `lens_disagreement_z`. Agreement is recorded as an informational finding. Disagreement is recorded with reason code `estimator_family_disagreement` and blocks certification in strict mode, because a learner-dependent projection is evidence about the estimator, not the system. The battery is a robustness gate, not a validity substitute: consensus among families cannot repair a violated sampling contract, and the sampling gates of Section 1 run first.
