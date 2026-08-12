"""Attack on q001-thread H-001: the 'declared selection' premise is the load-bearing wall.

Data (RoseLark primary-source check, GSE133344 filtered_cell_identities, position-matched):
  00 NegCtrl0_NegCtrl0: 2536   10 CBL_NegCtrl0: 663
  01 NegCtrl0_CNN1:      480   11 CBL_CNN1:     348
"""
import math
n = {"00": 2536, "10": 663, "01": 480, "11": 348}
total = sum(n.values())
p = {k: v / total for k, v in n.items()}
log_or = math.log(p["11"] * p["00"] / (p["10"] * p["01"]))
print(f"total cells {total}; pooled log OR = {log_or:+.3f} (OR = {math.exp(log_or):.3f})")

pA = p["10"] + p["11"]; pB = p["01"] + p["11"]
expected_11 = pA * pB * total
print(f"marginal P(CBL guide) = {pA:.3f}, P(CNN1 guide) = {pB:.3f}")
print(f"under independent delivery, expected doubles = {expected_11:.0f}; observed = {n['11']}")
print(f"double-transduction enrichment = {n['11']/expected_11:.2f}x")

# If cell-level assignment were treated as the randomization unit with these pooled
# odds, the GCM null is shifted by exactly -log OR(rho):
print(f"\nGCM null shift if quotas ignored: kappa tested against {-log_or:+.3f}, not 0")
