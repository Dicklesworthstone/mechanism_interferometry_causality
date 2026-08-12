# Static website

No build step and no remote dependencies are required.

```bash
python -m http.server 8765 --directory site
```

Open `http://127.0.0.1:8765`.

## Contents

| File | Role |
|---|---|
| `index.html` | The whole page. Section order: problem, certificate, interferometer, failure modes, conservation laws, orientation, inference, pipeline, software, empirical path |
| `styles.css` | Design tokens, light and dark themes, layout, and the classes that colour every diagram |
| `app.js` | Theme, scroll behaviour, and five interactive figures. Plain script, no modules |
| `assets/fonts/` | Inter and JetBrains Mono variable subsets, self-hosted so the page makes no network request |
| `mechanism_interferometry.pdf` | Byte-identical copy of `paper/main.pdf`, refreshed by `scripts/build_all.sh` |

## Conventions

- **Nothing is fetched.** No CDN, no analytics, no remote fonts. `scripts/check_repo.py` fails the
  build if a remote `src` or `href` appears in `index.html`.
- **Colour is semantic, everywhere.** Green means the square closes and the curvature is zero; amber
  means curvature is present and the result is diagnostic only; red means fail-closed; violet marks a
  mechanism or a primitive ratio. `app.js` contains no colour literals at all: diagram elements are
  given class names and coloured by the stylesheet, so both themes work without a redraw.
- **Every figure computes its own numbers** from the closed forms in the paper. The defaults
  reproduce `artifacts/simulations/exact_results.json` exactly, including the outcome synergy of
  `0.3`, the observed and hidden covariances of `-0.09` and `+0.09`, the implementation normalizer
  `1.063`, and the parity deletion pass count of `2`.
- **JavaScript is an enhancement.** With scripts disabled the prose, the equations and the numbers
  are all still present, and the three interactive figures fall back to the corresponding
  simulation-generated figures from the paper.
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
