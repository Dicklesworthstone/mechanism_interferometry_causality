# Tier-B Pilot Experiments

Deterministic, stdlib-only Python scripts backing the claim series in Agent Mail
thread `RS-20260812-autonomy-tierb` (tier-B QA design for `docs/AUTONOMOUS_MODE.md`)
and the H-001 attack in thread `RS-20260812-mic-q001`. Each script prints every
number cited in the corresponding claims; replay with `python3 <script>`.

These are exploratory research artifacts, not part of the audited Rust surface.
They carry no certificate authority. Claim statuses below reflect the
independent audit recorded in the thread; the audit record is the authority.

| Script | Claims | Audited status |
|---|---|---|
| `conservation_pilot.py` | C-001 (linear detection under separate routes), C-002 (coherent route invisible), C-003 (scalar-check kernel), mode-D construction (script now opens and closes with a RETRACTED banner for mode D) | C-002 survives, scoped as a coherence identity; C-003 survives with rank qualification (finite support of size m and k witness equations leave nullity `m − rank(L)`; on general spaces the honest statement is that finite batteries leave the orthogonal complement unchecked); mode D is refuted, see below; C-001 pending an out-of-family population check |
| `witness_identity_check.py` | first refutation attempt of the mode-D form | The attempt itself was invalid: its "counterexample" used a non-normalized population (`E[r_AB] = 1.04`), breaking a premise rather than the claim. Preserved as a record of the failure |
| `witness_identity_check2.py` | C-004 refutation and revision | Mode-D form refuted over a valid population by exact rational computation (`fractions.Fraction`; residuals 1/50 and 1/25); the corrected battery is the paper's master-marginal identity used cross-route (`E_0[w·r̂_e]` vs `Ê_e[w]`), which survives audit at population level |
| `detection_floors.py` | C-005a/b/c (measured detection behavior) | Empirical calibration, not exact computation: 400-trial per-cell-seeded Monte Carlo with Wilson 95% intervals printed for every rate. The null control is the 9 delta=0 cells (measured false-positive rate <= 0.5%, Wilson upper bounds <= 1.8%). The corruption family was author-selected (uniform multiplicative); confirmatory floors require a preregistered floor rule and corruption family. An earlier revision advanced the RNG stream with a discarded computation; the fix changed per-cell streams and left every qualitative conclusion intact, with rates moving by at most ~1 percentage point |
| `norman_h001_attack.py` | q001 H-001 attack arithmetic (double-transduction enrichment, GCM null shift) | Corner counts are the independently verified GSE133344 values; verdict adjudication pending in thread |

Known limitation shared by all scripts: state spaces are two- or three-point
fixtures chosen for exactness. Any claim promoted beyond the thread must first
survive an out-of-family population (epistemic law: evidence that selected a
hypothesis cannot also test it).
