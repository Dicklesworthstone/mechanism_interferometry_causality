# Static website

No build step is required to **serve** the site, and it has no remote dependencies.

```bash
python -m http.server 8765 --directory site
```

Open `http://127.0.0.1:8765`.

There is one build step to **rebuild the audit module**, which is only needed when the Rust
changes:

```bash
./scripts/build_wasm.sh          # writes site/pkg/, enforces the size budget
python scripts/generate_repository_manifest.py
```

`site/pkg/` is committed on purpose. A gitignored build output is how a sibling project shipped
an artifact that went 79 commits stale without anything noticing, and `wasm-pack` writes a
`.gitignore` into its own output directory that the build script deletes on every run.

## Contents

| File | Role |
|---|---|
| `index.html` | The whole page: problem, certificate, partial designs, interferometer, failure modes, conservation laws, orientation, inference, evidence boundary, pipeline, software, empirical path |
| `styles.css` | Design tokens, light and dark themes, layout, and the classes that colour every diagram |
| `app.js` | Theme, scroll behaviour, and seven interactive figures. Plain script, no modules |
| `pkg/` | The audit system compiled to WebAssembly by `scripts/build_wasm.sh`, from `crates/mic-wasm` |
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

- **Nothing is fetched from anywhere else.** No CDN, no analytics, no remote fonts.
  `scripts/check_repo.py` fails the build if a remote `src` or `href` appears in `index.html`.
  The deployed Content-Security-Policy allows `connect-src 'self'` rather than `'none'` only
  because `wasm-bindgen` loads the module with `fetch()`; there is no origin API to call, so
  nothing leaves the page either way.
- **The audit system runs here rather than being imitated here.** The design audit, the
  interaction-aliasing split, the preflight gates and the estimator lens battery are the compiled
  Rust, not JavaScript reimplementations of it. That distinction is not cosmetic: while the
  reimplementations existed, one of them reported `READY` for an hour after the engine had begun
  reporting `diagnostic_only` for the same input.
- **Colour is semantic, everywhere.** Green means the square closes and the curvature is zero; amber
  means curvature is present and the result is diagnostic only; red means fail-closed; violet marks a
  mechanism or a primitive ratio. `app.js` contains no colour literals: diagram elements are given
  class names and coloured by the stylesheet, so both themes work without a redraw.
- **Dark bands redeclare the design tokens locally** rather than carrying a parallel set of component
  rules, so any widget can sit on any band and inherit the right values through the cascade.
- **JavaScript is an enhancement for the prose, and a requirement for the audits.** With scripts
  disabled the prose, the equations and the fixture numbers are all still present, and the figures
  with static counterparts fall back to the simulation-generated images from the paper. The three
  figures that call the module (the design cube, the lens battery and the live preflight) cannot
  degrade that way, because their whole point is that a real audit produced the answer. Each says so
  in a `<noscript>` note rather than presenting an empty panel.
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
