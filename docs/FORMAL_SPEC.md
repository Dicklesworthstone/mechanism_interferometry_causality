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

Given a proposed DAG `G` and distinct proposed primitive targets `t_j`, the regime family has a context-invariant modular soft-intervention representation if and only if:

1. **Locality:** `r_j` is measurable with respect to `{t_j} ∪ pa(t_j)`.
2. **Conditional normalization:** `E_0[r_j | X_pa(t_j)] = 1`.
3. **Square flatness:** every defined two-dimensional design face has zero curvature almost everywhere.

Square flatness implies global additivity because every edge increment is invariant under flips of the other design coordinates. Higher-order Möbius tests are redundant on a complete cube, though they may be useful diagnostics under estimation error.

## 3. Master marginal identity

For any regime `e` and coordinate set `A`,

\[
E_0[r_e(X)\mid X_A]=\frac{p_e(X_A)}{p_0(X_A)}.
\]

Consequences:

- `E_0[r_e]=1` pins a discriminative score's additive constant.
- Deleting the true target from a localized family leaves an invariant marginal.
- Orientation should be estimated directly by low-dimensional two-sample equivalence, not by conditional means of an estimated ratio.

## 4. Pass-count state machine

The orientation result is one of:

- `UNIQUE_TARGET`: exactly one deletion is certified invariant and all competitors are certified changed.
- `NO_PASS`: no deletion is certified invariant.
- `MULTIPLE_PASSES`: more than one deletion is certified invariant.
- `UNDERPOWERED`: intervention discrepancy or effective sample size is below threshold.
- `UNDETERMINED`: simultaneous intervals overlap the equivalence boundary.

Only `UNIQUE_TARGET` produces an oriented family.

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

For observed corners `D`, define the main-effects design matrix `M_D=[1,s_1,...,s_K]`. Pointwise flatness is `h_D(x) ∈ col(M_D)`. The complete testable contrast space is `ker(M_D^T)`. A report must distinguish:

- estimable flatness contrasts;
- aliased contrasts;
- unobserved faces;
- the rank and dimension of the lack-of-fit space.

## 9. Longitudinal data

Ratios are estimated at the transition level. Whole-trajectory ratio products are prohibited as the primary estimator because their variance can grow exponentially with horizon. Resampling and uncertainty operate at the trajectory or randomization-unit level.
