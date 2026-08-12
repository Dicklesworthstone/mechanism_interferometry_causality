# NCI-ALMANAC / DrugComb scalar-response contract

This directory intentionally contains no MIC experiment manifest. The public
archive exposes scalar percent-growth response surfaces, not replicated joint
state laws. Four scalar corners can support a declared additive or
modified-Bliss response calculation and held-out dose-surface prediction, but
they do not identify density curvature, conditional normalization, a causal
family, or MIC's raw compositional normalizer.

Use [`scalar_response_contract.json`](scalar_response_contract.json) to keep
that boundary machine-readable. An adapter must preserve exact physical dose,
cell line, screen/site, control-normalization convention, and genuine
plate/experiment provenance. Missing dose cells remain missing. Hold out whole
drug-pair by cell-line surfaces or whole experiments, not individual cells of a
surface.

The deleted `manifest.json` was unsafe: it asserted equal sampling proportions
and state-independent selection without receipts and treated viability/growth
summaries as a design-sufficient state. Reintroducing an MIC manifest requires
replicate-level common-support measurements of a declared multivariate state
plus valid unit, sampling, and selection evidence.
