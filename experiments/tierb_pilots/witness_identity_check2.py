"""Corrected refutation of the mode-D weighted-conservation 'identity', over a VALID population.

Validity requires E0[ra]=E0[rb]=E0[rab]=1 (normalized regime densities) and positivity.
Under flatness (kappa == 0), rab = ra*rb, so validity forces E0[ra*rb] = 1, i.e. Cov(ra,rb)=0:
the SCALAR law holds. Question: does the weighted form E0[w(ra-1)(rb-1)] = 0 hold for all w?

Construction: X uniform on {-1,0,1}; ra=[0.7,1.0,1.3] (E=1); rb=[b,3-2b,b] (E=1 and
E[ra*rb]=1 for every b by symmetry); pick b=1.2 for positivity. All four densities valid.
"""
xs = [0, 1, 2]
ra = [0.7, 1.0, 1.3]
b = 1.2
rb = [b, 3 - 2 * b, b]
rab = [x * y for x, y in zip(ra, rb)]
n = 3
mean = lambda v: sum(v) / n
print(f"validity: E[ra]={mean(ra):.3f} E[rb]={mean(rb):.3f} E[rab]={mean(rab):.3f} (all must be 1)")
print(f"scalar law: Cov(ra,rb) = {mean([x*y for x,y in zip(ra,rb)]) - mean(ra)*mean(rb):+.6f} (must be 0 under flatness) OK")
for name, w in [("constant", [1,1,1]), ("1{X=+1}", [0,0,1]), ("sign(X)", [-1,0,1])]:
    lhs = mean([wi * (x - 1.0) * (y - 1.0) for wi, x, y in zip(w, ra, rb)])
    print(f"witness {name:9s}: weighted-cov side = {lhs:+.6f}   balance side = +0.000000 (kappa==0)   identity: {abs(lhs) < 1e-12}")
print("\n=> mode-D form REFUTED over valid populations (exact finite computation).")
print("   Correct general battery: cross-route master-marginal checks E0[w*r_e] vs E_e[w],")
print("   an exact identity (paper Lemma, master marginal) for EVERY witness and regime.")
