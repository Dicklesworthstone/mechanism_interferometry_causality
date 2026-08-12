"""Exact refutation of the mode-D weighted-conservation 'identity', over a VALID population.

All arithmetic uses fractions.Fraction, so every equality below is exact
rational computation, not floating-point approximation.

Validity requires E0[ra] = E0[rb] = E0[rab] = 1 (normalized regime densities)
and positivity. Under flatness (kappa == 0), rab = ra*rb, so validity forces
E0[ra*rb] = 1, i.e. Cov(ra, rb) = 0: the SCALAR law holds. Question: does the
weighted form E0[w*(ra-1)*(rb-1)] = 0 hold for every witness w?

Construction: X uniform on {-1,0,1}; ra = [7/10, 1, 13/10] (E0 = 1);
rb = [b, 3-2b, b] with b = 6/5, which satisfies E0[rb] = 1 and E0[ra*rb] = 1
for every b by the symmetry of ra around 1. All four densities are valid.
"""
from fractions import Fraction as F

ra = [F(7, 10), F(1), F(13, 10)]
b = F(6, 5)
rb = [b, 3 - 2 * b, b]
rab = [x * y for x, y in zip(ra, rb)]
n = 3

def mean(values):
    return sum(values) / n

assert mean(ra) == 1 and mean(rb) == 1 and mean(rab) == 1, "population invalid"
scalar_cov = mean([x * y for x, y in zip(ra, rb)]) - mean(ra) * mean(rb)
assert scalar_cov == 0, "scalar law must hold exactly under flatness"
print(f"validity: E[ra]=E[rb]=E[rab]=1 exactly; scalar law: Cov(ra,rb) = {scalar_cov} exactly")

for name, w in [("constant", [F(1)] * 3), ("1{X=+1}", [F(0), F(0), F(1)]), ("sign(X)", [F(-1), F(0), F(1)])]:
    lhs = mean([wi * (x - 1) * (y - 1) for wi, x, y in zip(w, ra, rb)])
    # kappa == 0 exactly, so the balance side is exactly zero for every witness.
    holds = lhs == 0
    print(f"witness {name:9s}: weighted-cov side = {lhs!s:>6s}   balance side = 0   identity: {holds}")

print("\n=> mode-D form REFUTED over valid populations by exact rational computation.")
print("   The audited general battery is the master-marginal identity used cross-route:")
print("   E0[w * r_hat_e] vs E_e[w], an exact identity for every witness and regime,")
print("   with the two sides estimated through genuinely separate routes.")
