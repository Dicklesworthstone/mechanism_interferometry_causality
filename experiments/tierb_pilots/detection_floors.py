"""Measured detection floors for the master-marginal cross-route QA battery.

Check: T_w = ( mean_0[w(X)*r_hat(X)] - mean_e[w(X)] ) studentized; fire when |z| > 3.
Population: X uniform on {-1,0,+1}; true ratio r_e = [0.7, 1.0, 1.3] (E0[r_e]=1).
Corruption: multiplicative bias, r_hat = r_e * (1+delta) (route-1 estimator bias).
Null calibration first (delta=0): measured false-positive rate must be reported
alongside power, else the apparatus has never been shown able to report "nothing".
Deterministic LCG; no numpy.
"""
SEED = 20260812

def lcg(seed):
    state = seed
    while True:
        state = (state * 6364136223846793005 + 1442695040888963407) % (1 << 64)
        yield state / (1 << 64)

def draw(u):
    return 0 if u < 1/3 else (1 if u < 2/3 else 2)

R_TRUE = [0.7, 1.0, 1.3]
WITNESSES = {"constant": [1.0, 1.0, 1.0], "sign(X)": [-1.0, 0.0, 1.0], "1{X=+1}": [0.0, 0.0, 1.0]}

def run(n, delta, witness, trials, rng):
    fires = 0
    w = WITNESSES[witness]
    r_hat = [r * (1 + delta) for r in R_TRUE]
    for _ in range(trials):
        # route 1: baseline sample, terms w(x)*r_hat(x)
        t1 = [w[draw(next(rng))] * r_hat[draw(next(rng))] for _ in range(n)]  # wrong: two draws
        # fix: one draw per observation
        t1 = []
        for _ in range(n):
            x = draw(next(rng))
            t1.append(w[x] * r_hat[x])
        # route 2: regime-e sample (density prop to r_true), terms w(x)
        t2 = []
        while len(t2) < n:
            x = draw(next(rng))
            if next(rng) < R_TRUE[x] / 1.3:  # rejection sample regime law
                t2.append(w[x])
        m1 = sum(t1) / n
        m2 = sum(t2) / n
        v1 = sum((t - m1) ** 2 for t in t1) / (n - 1)
        v2 = sum((t - m2) ** 2 for t in t2) / (n - 1)
        se = ((v1 + v2) / n) ** 0.5
        if se > 0 and abs(m1 - m2) / se > 3.0:
            fires += 1
    return fires / trials

rng = lcg(SEED)
TRIALS = 400
print(f"{'witness':10s} {'n':>5s}  " + "  ".join(f"d={d:4.2f}" for d in [0.0, 0.02, 0.05, 0.10, 0.20]))
for witness in WITNESSES:
    for n in [200, 1000, 5000]:
        rates = [run(n, d, witness, TRIALS, rng) for d in [0.0, 0.02, 0.05, 0.10, 0.20]]
        print(f"{witness:10s} {n:5d}  " + "  ".join(f"{r:5.1%}" for r in rates))
