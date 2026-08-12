# Static website

No build step and no remote dependencies are required.

```bash
python -m http.server 8765 --directory site
```

Open `http://127.0.0.1:8765`.

## Contents

| File | Role |
|---|---|
| `index.html` | The whole page: problem, certificate, partial designs, interferometer, failure modes, conservation laws, orientation, inference, evidence boundary, pipeline, software, empirical path |
| `styles.css` | Design tokens, light and dark themes, layout, and the classes that colour every diagram |
| `app.js` | Theme, scroll behaviour, and seven interactive figures. Plain script, no modules |
| `assets/fonts/` | Inter and JetBrains Mono variable subsets, self-hosted so the page makes no network request |
| `mechanism_interferometry.pdf` | Byte-identical copy of `paper/main.pdf`, refreshed by `scripts/build_all.sh` |

## The interactive figures

Each one computes its own numbers from the closed forms in the paper, or reimplements the
corresponding Rust logic. None of them display a stored result.

| Figure | What it computes |
|---|---|
| Interferometer | Complete-state ratios beside `kappa(y) = log[1 + ab tanh(y / sigma^2)]`, at the paper fixture by default |
| Partial designs | Main-effects rank, lack-of-fit dimension, complete square faces and square-contrast rank over any subset of a three-factor cube, matching `mic-design` |
| Scenario gallery | The four exact fixtures in `artifacts/simulations/exact_results.json` |
| Deletion evidence | Equivalence intervals against a movable tolerance, driving the five-state pass-count machine |
| Preflight report | The selection gate and product-odds gate from `mic-engine`, including `--allow-unvalidated-selection-model` |
| Estimator lens battery | `audit_lens_battery`: pairwise gaps scaled by the root sum of squared standard errors, asymmetric verdict, fail-closed on a non-positive standard error |

## Conventions

- **Nothing is fetched.** No CDN, no analytics, no remote fonts. `scripts/check_repo.py` fails the
  build if a remote `src` or `href` appears in `index.html`.
- **Colour is semantic, everywhere.** Green means the square closes and the curvature is zero; amber
  means curvature is present and the result is diagnostic only; red means fail-closed; violet marks a
  mechanism or a primitive ratio. `app.js` contains no colour literals: diagram elements are given
  class names and coloured by the stylesheet, so both themes work without a redraw.
- **Dark bands redeclare the design tokens locally** rather than carrying a parallel set of component
  rules, so any widget can sit on any band and inherit the right values through the cascade.
- **JavaScript is an enhancement.** With scripts disabled the prose, the equations and the numbers
  are all still present, and the three interactive figures with static counterparts fall back to the
  simulation-generated figures from the paper.
- **Claims stay inside what the paper supports.** The certificate is an existence result, agreement
  across estimator families certifies nothing, inclusion frequencies are not probabilities, and
  proposal-adapter scores are never evidence.
- **Themes.** The page follows the operating-system preference and remembers an explicit choice in
  `localStorage` under `mi-theme`.

## After editing

Site files are covered by the repository content inventory, so any change requires:

```bash
python scripts/generate_repository_manifest.py
python scripts/check_repo.py
```

Continuous integration does not regenerate the manifest. A site commit without a refreshed
`REPOSITORY_MANIFEST.json` fails the repository check.
