"""Self-audit of C-004's mode-D construction.

Question: is E[w(ra-E[ra])(rb-E[rb])] = -E[w*ra*rb*(e^kappa - 1)] a population
identity under exact modularity (kappa = 0), for arbitrary witness w?

Counterexample attempt: X uniform on {-1, 0, +1}, ra = 1+aX, rb = 1+bX,
rab = ra*rb (flat, kappa == 0). Then RHS = 0 identically. LHS should be 0 for
the identity to hold for every witness.
"""
a, b = 0.3, 0.2
xs = [-1.0, 0.0, 1.0]
ra = [1 + a * x for x in xs]
rb = [1 + b * x for x in xs]
n = len(xs)
mean = lambda v: sum(v) / n
ma, mb = mean(ra), mean(rb)

for name, w in [("constant", [1, 1, 1]), ("sign(X)", [-1, 0, 1]), ("1{X=1}", [0, 0, 1])]:
    lhs = mean([wi * (x - ma) * (y - mb) for wi, x, y in zip(w, ra, rb)])
    rhs = 0.0  # kappa == 0 exactly, so the balance side vanishes for every witness
    print(f"witness {name:9s}: weighted-cov side = {lhs:+.6f}   balance side = {rhs:+.6f}   identity holds: {abs(lhs-rhs) < 1e-12}")

print("\nCorrect general form: cross-route agreement of the four-law moment")
print("M_w = E[w*(rab - ra*rb)] (route 1: joint-ratio estimate) vs")
print("M_w = E[w*ra*rb*(e^kappa_hat - 1)] (route 2: separately estimated curvature).")
kappa_hat = [0.0, 0.0, 0.0]
import math
for name, w in [("constant", [1, 1, 1]), ("sign(X)", [-1, 0, 1]), ("1{X=1}", [0, 0, 1])]:
    rab = [x * y for x, y in zip(ra, rb)]
    route1 = mean([wi * (ab - x * y) for wi, x, y, ab in zip(w, ra, rb, rab)])
    route2 = mean([wi * x * y * (math.exp(k) - 1) for wi, x, y, k in zip(w, ra, rb, kappa_hat)])
    print(f"witness {name:9s}: route1 = {route1:+.6f}  route2 = {route2:+.6f}  agree: {abs(route1-route2) < 1e-12}")
