"""Quickie pilot: when does the three-way conservation check detect estimation error?

Fixture: latent_conservation(a=0.3) exact population values on X in {-1,+1}, uniform.
  ra = [0.7, 1.3], rb = [1.3, 0.7], rab = [1.0, 1.0]
Identity: Cov(ra, rb) = -E[ra*rb*(e^kappa - 1)] with kappa = ln(rab/(ra*rb)).

Corruption modes:
  A) independent-route: perturb the marginal-ratio estimates (ra,rb) by relative bias
     delta while keeping the joint-ratio estimate rab at truth (separate estimators).
  B) coherent: perturb ra,rb the same way but recompute rab := ra*rb*e^kappa_true
     (kappa reconstructed algebraically from the same fitted ratios).
Metric: |lhs - rhs| of the identity (residual), relative to |lhs|.
"""
import math

def residual(ra, rb, rab):
    n = len(ra)
    mean = lambda v: sum(v) / n
    cov = mean([x * y for x, y in zip(ra, rb)]) - mean(ra) * mean(rb)
    rhs_terms = []
    for a, b, ab in zip(ra, rb, rab):
        kappa = math.log(ab / (a * b))
        rhs_terms.append(a * b * (math.exp(kappa) - 1.0))
    rhs = -mean(rhs_terms)
    return cov, rhs, abs(cov - rhs)

a = 0.3
ra0 = [1 - a, 1 + a]
rb0 = [1 + a, 1 - a]
rab0 = [1.0, 1.0]
kappa_true = [math.log(ab / (x * y)) for x, y, ab in zip(ra0, rb0, rab0)]

cov0, rhs0, res0 = residual(ra0, rb0, rab0)
print(f"exact fixture: cov={cov0:+.6f} rhs={rhs0:+.6f} residual={res0:.2e}")

print("\nmode A: independent-route corruption (ra biased, rab held at truth)")
for delta in [0.01, 0.02, 0.05, 0.10, 0.20]:
    ra = [x * (1 + delta) for x in ra0]
    cov, rhs, res = residual(ra, rb0, rab0)
    print(f"  delta={delta:4.2f}  cov={cov:+.6f} rhs={rhs:+.6f} residual={res:.4f}  rel={res/abs(cov):6.1%}")

print("\nmode B: coherent corruption (rab recomputed from same corrupted ratios)")
for delta in [0.01, 0.05, 0.20]:
    ra = [x * (1 + delta) for x in ra0]
    rab = [x * y * math.exp(k) for x, y, k in zip(ra, rb0, kappa_true)]
    cov, rhs, res = residual(ra, rb0, rab)
    print(f"  delta={delta:4.2f}  cov={cov:+.6f} rhs={rhs:+.6f} residual={res:.2e}  (invisible)")

print("\nmode C: sign-varying corruption that cancels in the mean (adversarial)")
ra = [ra0[0] * 1.10, ra0[1] * (ra0[0] * ra0[1] * 0.10 / (ra0[1] ** 2) * -1 + 1)]
cov, rhs, res = residual(ra, rb0, rab0)
print(f"  crafted     cov={cov:+.6f} rhs={rhs:+.6f} residual={res:.4f}  rel={res/abs(cov) if cov else float('inf'):6.1%}")

print("\nmode D: witness battery closes the kernel on this fixture")
def weighted_residual(ra, rb, rab, w):
    n = len(ra)
    mean = lambda v: sum(v) / n
    # Weighted conservation form: E[w*(ra-E[ra])*(rb-E[rb])] + E[w*ra*rb*(e^k - 1)]
    ma, mb = mean(ra), mean(rb)
    lhs = mean([wi * (x - ma) * (y - mb) for wi, x, y in zip(w, ra, rb)])
    rhs = -mean([
        wi * x * y * (math.exp(math.log(ab / (x * y))) - 1.0)
        for wi, x, y, ab in zip(w, ra, rb, rab)
    ])
    return abs(lhs - rhs)

witnesses = {"constant": [1.0, 1.0], "sign(X)": [-1.0, 1.0]}
ra_crafted = [ra0[0] * 1.10, ra0[1] * (ra0[0] * ra0[1] * 0.10 / (ra0[1] ** 2) * -1 + 1)]
for name, w in witnesses.items():
    res_honest = weighted_residual(ra0, rb0, rab0, w)
    res_crafted = weighted_residual(ra_crafted, rb0, rab0, w)
    print(f"  witness {name:9s}: honest residual={res_honest:.2e}  crafted-corruption residual={res_crafted:.4f}")
