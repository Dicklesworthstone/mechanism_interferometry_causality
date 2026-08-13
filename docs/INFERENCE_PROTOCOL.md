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

An experiment manifest records a selection declaration, not proof of that
declaration. Strict readiness requires a separately resolved, content-bound
selection-evidence receipt; rows alone cannot establish state-independent
inclusion. The exact sensitivity algebra uses distinct validated types for
inclusion probabilities and regime inclusion rates. A normalizer interaction
supplied without such a receipt is serialized as `declared_unverified` and has
diagnostic authority only.

The executable resolver accepts the receipt and authority source through
separate paths, hashes the analyzed table by streaming rather than loading it
into memory, and creates an opaque readiness token only when the receipt binds
all three byte identities and its evidence class matches the manifest
declaration. This verifies provenance and declared authority, not the scientific
truth of an arbitrary source document; the source must itself be an eligible
external sampling record or validated selection-model artifact.

Selection sensitivity also accepts validated regime enrollment rates or a
model-derived normalizer contrast together with a `sha256:` receipt commitment.
Those routes derive `Delta log Z` rather than accepting a second naked scalar,
but the core algebra labels the reference `*_receipt_unresolved` until an engine
resolver matches the receipt to the manifest and analyzed bytes. Every resulting
Gamma interval therefore remains diagnostic-only.

## 2. Cross-fitting graph

All learned nuisances are trained out of fold. The fold assignment is deterministic from a recorded seed and keeps the highest declared dependence unit intact. Assignment episodes nested in one dependence unit may carry different regimes, as in crossover or longitudinal designs, but they remain in the same fold. The reference closure diagnostic gives each dependence unit equal mass within corner before dividing that mass over its episodes and rows. These roles remain declarations until external design evidence resolves them. No observation may be used both to construct an adaptive witness and to test that witness.

Recommended layers:

- outer folds: scientific evaluation and adaptive-witness separation;
- inner folds: nuisance tuning;
- dependence-unit bootstrap: uncertainty at the highest dependence unit, with assignment episodes retained intact.

## 3. Track I: four-law moment functionals

For prespecified witnesses `w_m`, estimate

\[
M_w=E_{AB}[w]-E_A[w\hat r_B]
\]

and its A/B-swapped analogue. Combine symmetrically. Primitive ratios are normalized on held-out baseline data. Report raw and truncated estimates, effective sample sizes, and the sensitivity curve across truncation thresholds.

This track is valid for arbitrary known state-independent regime quotas. It does not require product assignment.

The reference finite/discrete adaptive slice is deliberately two-stage. On
discovery units, `learn_discrete_closure_witness` averages residual
contributions within declared dependence unit and state, then maps each state
mean through a frozen bounded `tanh` transform. Its API has no confirmation-row
input and emits a content fingerprint consumed by
`four_law_cluster_multiplier_bounds`. Unseen confirmation states receive zero
weight. This proves API-level separation and unit-duplication invariance for the
reference representation; it does not verify the physical unit, the discovery
partition, or representation choice, and its multiplier bounds remain
`diagnostic_only` until the coverage gauntlet is complete.

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

The reference `mic_stats::gcm_projection` API cannot be called without a `ProductDesignEvidence` value, and every returned `GcmEstimate` serializes that evidence. `from_sampling_odds_audit` requires a completed-audit identifier and a SHA-256 fingerprint of its declared allocation source, then recomputes the pooled log odds and refuses non-product inputs. `from_reweighting_audit` likewise requires an audit identifier and artifact fingerprint, not a future plan. These constructors validate syntax and arithmetic; they cannot establish that the referenced audit exists or belongs to the projection's manifest and data. Deserialization reruns those structural and numerical checks, so a malformed or numerically non-product object cannot acquire authority by bypassing the constructor. Every engine preflight and tabular report now carries `manifest_canonical_sha256`, the SHA-256 of the complete validated manifest's canonical Rust serialization; this binds design corners, declared proportions, units, state, track, selection, data reference, and seed independently of source-file whitespace. It still does not prove the origin of declared allocation proportions or bind a GCM reference to the analyzed table. Packet 0A must resolve those references against this manifest binding, the data fingerprint, and the evidence ledger before either non-diagnostic grade can support a certificate. Until then, a syntactically valid identifier remains an unresolved reference, empirical counts that merely look product remain diagnostic, and production reports must not promote the projection to certificate evidence.

## 5. Localization

Run Design-IAMB with the regime label as the response:

1. forward addition by reproducible cross-fitted proper-score gain or residual-product dependence;
2. backward pruning conditional on the remaining set;
3. repeated splits and stability frequencies;
4. conservative support union for orientation audit.

The output is a candidate Markov boundary with inclusion frequencies and unresolved variables, not a guaranteed causal family.

An ensemble alternative scores many randomly sampled candidate supports by held-out proper regime-prediction loss, then keeps the parsimony frontier: every support within a preregistered loss tolerance of the ensemble best, ordered first by support cardinality and only then by learner-specific complexity. Under ratio faithfulness and adequate learner capacity, locality makes the true support the smallest support carrying full regime information, so the smallest frontier member is the preferred localization proposal; without faithfulness an inactive true parent can be omitted. The reference primitive is `mic_stats::parsimony_frontier`. Mandatory rules: the frontier threshold is computed once over the completed ensemble, never incrementally while results accumulate; inclusion frequencies are descriptive frequencies over the designed candidate ensemble, not probabilities, normalized per variable; with repeated splits, average within each split first and then across splits rather than weighting splits by frontier size. Frontier membership and inclusion frequencies are recorded in the evidence ledger as localization stability paths. Because the support is selected adaptively, certificate-grade conclusions about it require an outer held-out confirmation sample; same-sample results remain exploratory.

## 6. Deletion equivalence

For localized support `S`, compute a nonnegative two-sample discrepancy for each deletion. MMD and energy distance are the initial engines. A biased/V-statistic MMD squared is nonnegative and may be used as the point discrepancy. The unbiased MMD squared U-statistic is a signed finite-sample estimator that can be negative under the null; it may be used inside a calibrated resampling procedure, but its raw value must not be passed to the nonnegative relative-discrepancy API or clipped post hoc without accounting for that transformation. Normalize the chosen nonnegative point discrepancy by the full-support discrepancy:

\[
R_v=D_v/(D_{full}+\eta).
\]

Use a joint multiplier or cluster bootstrap to produce simultaneous lower and upper confidence bounds. With tolerance `epsilon`:

- `upper < epsilon`: certified invariant;
- `lower > epsilon`: certified changed;
- otherwise: undetermined.

The target is returned only if exactly one coordinate is certified invariant and every competitor is certified changed. This pass pattern has orientation authority only under separately justified single-target intervention semantics and deletion faithfulness; the pass count cannot establish those premises from the same marginals.

Reference primitives: `mic_stats::classify_deletion` performs the tri-state comparison, `mic_stats::simultaneous_mean_bounds` supplies deterministic reference Rademacher multiplier bounds for mean-type discrepancy vectors at the cluster level (not a finite-sample coverage guarantee; U-statistic degeneracy corrections remain future work), and `mic_stats::orient_from_deletions` runs the five-state pass-count machine. The reference multiplier routine rejects every zero-scale or nonfinite-scale column: until a valid degenerate correction exists, a constant empirical discrepancy cannot receive a zero-width interval or certify equivalence. The state machine rejects row-specific equivalence tolerances: every deletion in a declared family uses the same preregistered `epsilon`. `mic_engine::audit_orientation` records the verdict: the unique-target state is informational, and every other state is a blocking `orientation_unresolved` error, so strict runs abstain rather than force an orientation. An underpowered intervention abstains before any counting. The `mic orient` subcommand exposes this audit on a schema-validated JSON bounds file, but accepts it only with declared simultaneous coverage, interval method, randomization unit, source fingerprint, and deterministic seed; it derives and records a SHA-256 hash of the exact input artifact. `examples/orientation/parity_demo.json` reproduces the parity ambiguity end to end. Ratio-weight overlap is gated separately by `mic_engine::audit_overlap`, which blocks with `overlap_failure` when the effective-sample-size ratio falls below the policy floor.

When more than one deletion is certified invariant, the report must propose the next experiment rather than force a verdict. Each surviving hypothesis predicts which deletion marginals stay invariant under a further tilt of the same target mechanism. The recommended follow-up tilt maximizes the worst-case predicted separation between surviving hypotheses under the declared discrepancy. The admissible class contains only replacement conditionals for the proposed target; intervening on a different node is a different primitive, not a disambiguating tilt. In the parity fixture, any asymmetric replacement that moves the target's own marginal separates the two hypotheses in one additional regime. Disambiguation always requires confirmatory data from the new regime; the proposal itself is exploratory output. `mic propose-tilt` emits this recommendation with `proposal_only` authority, a derived fingerprint over the full candidate library, and an explicit experiment-design abstention when no candidate is eligible.

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

The implemented finite-state reference path performs this construction in the
log domain from complete `00`, `10`, and `01` probability tables and never uses
`11` to form the prediction. It reports raw `Z`, `Z-1`, maximum log normalized
importance ratio, asymptotic ESS fraction, and held-out total variation and
Hellinger distance. This exact oracle is a conformance target for the future
fitted ratio path; it is not a calibrated finite-sample test. The implemented
linear joint-regime path also provides deterministic outer cluster folds and
held-out proper-loss comparison. Folds are stratified by corner, training mass
matches the exact declared classifier pooling proportions, and held-out
baseline clusters produce signed mean, mean-absolute, RMS, and maximum-absolute
summaries of the regularized linear interaction projection. They also report
held-out posterior-boundary alarms, implied density-ratio cluster ESS, and
maximum finite log-ratio magnitude. The interaction projection equals density
curvature only under correct model specification. These summaries remain
uncalibrated. For an independently frozen witness family,
`mic_stats::four_law_cluster_multiplier_bounds` implements the symmetrized
arbitrary-quota moment with law-stratified, declared-unit Rademacher multiplier
max bounds and a recorded seed. Its output remains `diagnostic_only` and
explicitly `reference_multiplier_not_coverage_validated`: adaptive witness
training and coverage over the cluster-size, overlap, and nuisance-
misspecification gauntlet remain outstanding.

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

Population curvature functionals are gauge invariant, but every empirical projection inherits the inductive bias of its nuisance learner. Certified projections are therefore repeated across a declared battery of deliberately dissimilar model families, for example a regularized linear model, a kernel or nearest-neighbour method, and a boosted tree or neural learner, each inside its own cross-fitting scheme. The reference primitive is `mic_engine::audit_lens_battery`: each family contributes a point estimate and a finite, strictly positive standard error for the same estimand; each pairwise gap is scaled by the root sum of squared standard errors; and the maximum scaled gap is compared with the policy tolerance `lens_gap_tolerance`. The families normally share data and folds, so the scaled gap is a preregistered robustness heuristic, not a calibrated joint test statistic; a calibrated verdict-level gate belongs where joint equivalence bounds exist. The audit is asymmetric by design. Disagreement is recorded with reason code `estimator_family_disagreement` and blocks certification in strict mode, because a learner-dependent projection is evidence about the estimator, not the system. Agreement is recorded as an informational finding only: consensus among families is diagnostic, never certifying, and cannot repair a violated sampling contract. Degenerate inputs, including non-positive standard errors, fail closed rather than producing non-finite metrics, so every audit artifact remains serializable. The sampling gates of Section 1 run first.

## 13. Final certificate derivation

`EvidenceLedger::status` accepts a complete `CertificateGates` value rather than
a Boolean. The gate object records locality, conditional normalization, square
flatness, and unique orientation independently. `unresolved` is used for absent,
invalid, underpowered, or indeterminate evidence and never means a population
refutation. A strict `failed` result requires a valid refutation of at least one
necessary implication; otherwise incomplete evidence abstains. Blocking ledger
errors precede both pass and failure because a violated selection, design, unit,
or calibration contract invalidates the purported scientific verdict.

Report constructors derive status internally from the ledger and gate object.
Serialized audit reports carry the same gates, and the JSON Schema rejects
exploratory passes, passes with unresolved gates, passes or failures with
blocking errors, failures without a refuted implication, and unexplained
abstentions. Current histogram, lens-battery, and orientation-only paths remain
non-certifying until independent producers populate every theorem gate.
