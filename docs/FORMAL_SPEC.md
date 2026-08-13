# Formal Specification

## 1. Objects

A run analyzes a finite collection of normalized regime laws \(\{P_s:s\in D\}\), where `D` is a subset of the Boolean intervention cube. A reference corner is a coordinate convenience, not a scientific baseline. All density-ratio operations require the declared common-support region.

For a complete cube, primitive regime ratios are

\[
r_j(x)=\frac{p_{e_j}(x)}{p_0(x)},\qquad \ell_j(x)=\log r_j(x).
\]

For any observed square face with base `b` and coordinates `j,k`,

\[
\kappa_{jk}^b(x)=h_{b+e_j+e_k}(x)-h_{b+e_j}(x)-h_{b+e_k}(x)+h_b(x),
\]

where \(h_s=\log p_s\).

## 2. Population certificate

Assume a complete common-support regime cube, a proposed DAG `G`, distinct
proposed primitive targets `t_j`, and a reference law `p_0` that factorizes over
`G`. Under these standing premises, the regime family has a context-invariant
modular soft-intervention representation if and only if:

1. **Locality:** `r_j` is measurable with respect to `{t_j} ∪ pa(t_j)`.
2. **Conditional normalization:** `E_0[r_j | X_pa(t_j)] = 1`.
3. **Square flatness:** every defined two-dimensional design face has zero curvature almost everywhere.

Square flatness implies global additivity because every edge increment is invariant under flips of the other design coordinates. Higher-order Möbius tests are redundant on a complete cube, though they may be useful diagnostics under estimation error.

### 2.1 Finite categorical mechanism families

Factor \(j\) may instead have a finite level set \(\mathcal A_j\) with marked
baseline \(0_j\). Nonbaseline levels are mutually exclusive alternative
replacements of one proposed target; distinct factors still target distinct
mechanisms. Retain the common-support and baseline-factorization premises from
Section 2. On the complete product grid, define

\[
r_{j,a}(x)=\frac{p_{(0,\ldots,a,\ldots,0)}(x)}{p_0(x)}.
\]

The grid has a modular representation if and only if every \(r_{j,a}\) is local
to \(\{t_j\}\cup\operatorname{pa}(t_j)\), every level conditionally normalizes
over \(t_j\), and every cross-family rectangle vanishes at every background:

\[
h_{a,b,u}+h_{a',b',u}-h_{a,b',u}-h_{a',b,u}=0.
\]

It suffices to use \(a'=0_j,b'=0_k\) only when those rectangles are checked at
every background \(u\). Levels within one family need not lie on a linear or
ordered dose path. A continuous dosage model requires a separately normalized
parametric mechanism path.

## 3. Master marginal identity

For any regime `e` and coordinate set `A`,

\[
E_0[r_e(X)\mid X_A]=\frac{p_e(X_A)}{p_0(X_A)}.
\]

Consequences:

- `E_0[r_e]=1` pins a discriminative score's additive constant.
- Under a credible single-target intervention premise, deleting the true target from a localized family leaves an invariant marginal.
- Orientation should be estimated directly by low-dimensional two-sample equivalence, not by conditional means of an estimated ratio.

A unique deletion pass does not establish the single-target premise. A regime
that changes two mechanisms can preserve one deletion marginal and therefore
produce exactly one pass. Orientation authority requires the intervention
semantics and deletion-faithfulness premise to be justified separately.

## 4. Pass-count state machine

The numerical deletion audit is one of:

- `UNIQUE_PASS_PATTERN`: exactly one deletion is certified invariant and all competitors are certified changed.
- `NO_PASS`: no deletion is certified invariant.
- `MULTIPLE_PASSES`: more than one deletion is certified invariant.
- `UNDERPOWERED`: intervention discrepancy or effective sample size is below threshold.
- `UNDETERMINED`: simultaneous intervals overlap the equivalence boundary.

`UNIQUE_PASS_PATTERN` is still proposal-level numerical evidence. It cannot name
a target or produce an oriented family by itself. A separate authority-bearing
object may orient only after resolving the independently supplied single-target
intervention semantics and deletion-faithfulness premises.

Support undercoverage cannot create a unique wrong target while the true target
remains in the support, but it can create extra passes. Deletion faithfulness on
the full causal family therefore does not automatically transfer to a restricted
candidate support.

## 5. Curvature and sampling

Density curvature is a functional of four normalized laws:

\[
\kappa(x)=\log\frac{p_{11}(x)p_{00}(x)}{p_{10}(x)p_{01}(x)}.
\]

If samples are pooled with known, state-independent proportions `rho`,

\[
\kappa(x)=\log OR_\rho(A,B\mid X=x)-\log OR(\rho).
\]

A residual-product test uses `Cov(A,B|X)`. It characterizes zero density curvature only when `OR(rho)=1`, i.e. under product sampling odds. Arbitrary corner quotas must be reweighted or analyzed by four-law functionals.

Within-regime selection depending on `X` changes the regime laws and invalidates the identity unless the selection model is incorporated.

## 6. Conservation laws

With `r_AB = r_A r_B exp(kappa)`:

\[
Cov_0(r_A,r_B)=-E_0[r_A r_B(e^\kappa-1)].
\]

If flat complete-state factors `L_A,L_B` are marginalized to `X`,

\[
e^\kappa-1=\frac{Cov(L_A,L_B\mid X)}{r_A r_B}
\]

and

\[
Cov(r_A,r_B)=-E[Cov(L_A,L_B\mid X)].
\]

For candidate state expansion `W`,

\[
r_A^Xr_B^X(e^{\kappa_X}-1)
=E[r_A^{XW}r_B^{XW}(e^{\kappa_{XW}}-1)\mid X]
+Cov(r_A^{XW},r_B^{XW}\mid X).
\]

## 7. Representation-level testing

Curvature is invariant under invertible transformations. It is also preserved by any representation `Z=phi(O)` satisfying design sufficiency `S ⟂ O | Z`. Flatness may therefore be tested on a learned representation only after invertibility or design sufficiency has been established on held-out data. Flatness is never an optimization reward.

## 8. Partial designs

For observed corners \(D\), define the main-effects design matrix
\(M_D=[1,s_1,\ldots,s_K]\); for categorical families use one treatment-coded
column per nonbaseline level. Pointwise flatness is
\(h_D(x)\in\operatorname{col}(M_D)\). The complete testable contrast space is
\(\ker(M_D^\top)\). A report must distinguish:

- estimable flatness contrasts;
- aliased contrasts;
- unobserved faces;
- the rank and dimension of the lack-of-fit space.

This null-space test is the complete flatness audit on `D`, not by itself a
partial-design modularity certificate. If `D` omits primitive corners, locality
and conditional normalization may be unidentifiable even when the lack-of-fit
space is trivial. Certificate-grade use requires observed or otherwise
identified primitive potentials with separate locality and normalization
evidence, or an explicit existence/feasibility test for such potentials.

For a fixed proposed graph and target assignment, define the causal completion
fiber as all main-effect potential systems reproducing \(h_D\) while satisfying
baseline factorization, locality, conditional normalization, positivity, and
common support. Report two separate objects:

- **Completion status:** infeasible, point identified, or set identified,
  relative to a named query and declared label/gauge symmetries.
- **Design testability:** main-effects rank, left-null lack-of-fit dimension,
  identified potential subspace, observed closure rectangles, and untested
  directions.

Untestable is not a mutually exclusive completion status. For example,
\(D=\{00,10,01\}\) can uniquely identify the two primitive potentials and
predict \(11\) while leaving composition untested because no combination law
was observed. Conversely, \(D=\{00,11\}\) has no flatness contrast but can be
causally infeasible under distinct-root replacements.

### 8.1 Fixed-DAG kernel completion with a supplied baseline

There is a stronger exact result when the query fixes the entire finite DAG,
the distinct target map, and a strictly positive baseline law. Factor every
supplied regime over the fixed DAG and write its node-conditional kernels as
\(k_v^{(s)}(x_v\mid x_{\mathrm{pa}(v)})\). A modular completion exists if and
only if:

1. the baseline and every observed regime factorize over the fixed DAG;
2. every node not actively targeted in a regime retains its baseline kernel;
3. every repeated nonbaseline family-level exposes the same target kernel in
   every observed background.

Necessity is cancellation in the DAG product. For sufficiency, define each
observed replacement kernel from any regime carrying that level; condition 3
makes the choice well-defined, condition 2 reconstructs each supplied regime,
and arbitrary positive normalized kernels complete never-observed levels.
Relative to the named query “the entire unrestricted kernel dictionary and all
grid laws,” the completion is point identified exactly when every nonbaseline
family-level occurs at least once; otherwise it is set identified. Background
repetition supplies overidentifying reuse tests, which remain separate from
level coverage and identification.

The premises are load-bearing. Without a supplied baseline, every level zero
must instead be covered somewhere. Targets must be distinct (alternative
replacements of one target belong to one categorical family), positivity must
cover every declared parent configuration, and kernel equality is only
identified almost everywhere on common parent support in general spaces. The
result is conditional on the proposed DAG and target labels and provides no
graph, target, selection, or intervention-semantics authority.

## 9. Longitudinal data

Ratios are estimated at the transition level. Whole-trajectory ratio products are prohibited as the primary estimator because their variance can grow exponentially with horizon. Resampling and uncertainty operate at the trajectory or randomization-unit level.

## 10. Certificate authority

Final status is derived from a closed typed gate set, never from a caller Boolean:

- locality, conditional normalization, and square flatness are each
  `established`, `refuted`, or `unresolved`;
- orientation is either `established` or `unresolved`; `established` means both
  a unique deletion pass and the single-target/deletion-faithfulness premises
  were justified under the declared evidence contract, while ambiguity does not
  refute modularity;
- `passed` requires all three implications established and unique orientation;
- `failed` requires at least one valid implication refutation;
- missing, invalid, underpowered, or ambiguous evidence yields `abstained`;
- any blocking evidence-contract error takes precedence over a purported
  refutation, and exploratory runs are always `diagnostic_only`.
