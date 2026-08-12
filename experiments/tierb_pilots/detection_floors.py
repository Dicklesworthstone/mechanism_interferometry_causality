"""Measured detection behavior for the master-marginal cross-route QA check.

Check: T_w = mean_0[w(X)*r_hat(X)] - mean_e[w(X)], studentized; fire when |z| > 3.
Population: X uniform on {-1,0,+1}; true ratio r_e = [0.7, 1.0, 1.3] (E0[r_e]=1).
Corruption family (author-selected, uniform multiplicative): r_hat = r_e*(1+delta).

Epistemic status (audited): EMPIRICAL CALIBRATION, not exact computation.
Per-cell detection rates are binomial over TRIALS seeded replays; Wilson 95%
intervals are printed alongside every rate. The 9 null cells (delta = 0) are the
measured false-positive control. No value here is a preregistered "floor": a
confirmatory floor requires a floor rule (target power + CI bound) declared
before the run and a corruption *family*, not one author-chosen shape.

Determinism: each (witness, n, delta) cell derives its own seed from the master
seed and the cell coordinates, so cell ordering cannot change any result.
"""
SEED = 20260812
TRIALS = 400

def lcg(seed):
    state = seed % (1 << 64)
    while True:
        state = (state * 6364136223846793005 + 1442695040888963407) % (1 << 64)
        yield state / (1 << 64)

def cell_seed(master, witness_index, n, delta_index):
    return master ^ (witness_index * 0x9E3779B9) ^ (n * 0x85EBCA6B) ^ (delta_index * 0xC2B2AE35)

def draw(u):
    return 0 if u < 1 / 3 else (1 if u < 2 / 3 else 2)

def wilson(successes, trials, z=1.96):
    if trials == 0:
        return (0.0, 1.0)
    p = successes / trials
    denom = 1 + z * z / trials
    center = (p + z * z / (2 * trials)) / denom
    half = z * ((p * (1 - p) / trials + z * z / (4 * trials * trials)) ** 0.5) / denom
    return (max(0.0, center - half), min(1.0, center + half))

R_TRUE = [0.7, 1.0, 1.3]
WITNESSES = {"constant": [1.0, 1.0, 1.0], "sign(X)": [-1.0, 0.0, 1.0], "1{X=+1}": [0.0, 0.0, 1.0]}
DELTAS = [0.0, 0.02, 0.05, 0.10, 0.20]

def run_cell(n, delta, weights, rng):
    fires = 0
    r_hat = [r * (1 + delta) for r in R_TRUE]
    for _ in range(TRIALS):
        t1 = []
        for _ in range(n):
            x = draw(next(rng))
            t1.append(weights[x] * r_hat[x])
        t2 = []
        while len(t2) < n:
            x = draw(next(rng))
            if next(rng) < R_TRUE[x] / 1.3:
                t2.append(weights[x])
        m1 = sum(t1) / n
        m2 = sum(t2) / n
        v1 = sum((t - m1) ** 2 for t in t1) / (n - 1)
        v2 = sum((t - m2) ** 2 for t in t2) / (n - 1)
        se = ((v1 + v2) / n) ** 0.5
        if se > 0 and abs(m1 - m2) / se > 3.0:
            fires += 1
    return fires

header = f"{'witness':10s} {'n':>5s}  " + "  ".join(f"{'d=' + format(d, '4.2f'):>18s}" for d in DELTAS)
print(header)
for wi, (witness, weights) in enumerate(WITNESSES.items()):
    for n in [200, 1000, 5000]:
        cells = []
        for di, delta in enumerate(DELTAS):
            rng = lcg(cell_seed(SEED, wi, n, di))
            fires = run_cell(n, delta, weights, rng)
            low, high = wilson(fires, TRIALS)
            cells.append(f"{fires / TRIALS:5.1%} [{low:4.1%},{high:5.1%}]")
        print(f"{witness:10s} {n:5d}  " + "  ".join(f"{cell:>18s}" for cell in cells))
