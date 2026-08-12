# Norman et al. 2019 template

Public source: GEO `GSE133344` (Norman, Horlbeck, et al., *Science* 2019).

This directory is a **manifest template**. The expression table is not
bundled. Point `data.path` at a local extract that has exactly four corners
for one gene pair, then:

```bash
cargo run -p mic-engine --bin mic-tabular -- ingest examples/datasets/norman2019/manifest.json --base-dir .
cargo run -p mic-engine --bin mic-tabular -- report examples/datasets/norman2019/manifest.json --base-dir .
```

Required columns after your extract:

| Column | Role |
|---|---|
| `replicate_id` | randomization / biological-replicate unit |
| `regime` | `00` / `10` / `01` / `11` or the manifest ids |
| `expr_*` | measured state |
| `guide_efficiency` | candidate expansion, not in `state_columns` |
| `included` | 1 if the cell/construct passes QC |

Do not split folds on `cell_id`.
