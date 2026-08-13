# Public-extract builders

Session workspace (raw files, tables, receipts):
`~/brennerbot_sessions/mic-public-extracts/`

These scripts do not mint certificates. They download bounded public files,
build token-context CSVs, run `mic-tabular survey`, and write receipts.

```bash
python3 scripts/public_extracts/process_extracts.py
python3 scripts/public_extracts/process_alts.py
python3 scripts/public_extracts/process_replacements.py
```

Do not commit NOAA/NASA/Zenodo payloads into `site/` or `paper/`.
Do not treat a complete square on an abstain dataset as a direction.
