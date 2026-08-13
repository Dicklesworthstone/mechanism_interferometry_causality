#!/usr/bin/env python3
"""Receipts for blocked, DUA, and explicitly excluded sources."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import write_receipt

BLOCKS = [
    {
        "name": "heartsteps_mrt",
        "status": "blocked",
        "source_url": "https://clinicaltrials.gov/study/NCT03225521",
        "license": "investigator / DUA; not walk-up public",
        "cluster_unit": "decision_point (not person-period)",
        "ground_truth_authority": "micro-randomized assignment, if a receipted extract exists",
        "note": "Design template only. Do not scrape unpublished trial microdata.",
        "first_falsifier": "treating person-period rows as iid assignment units",
    },
    {
        "name": "ohiot1dm",
        "status": "blocked",
        "source_url": "https://webpages.charlotte.edu/rbunescu/data/ohiot1dm/OhioT1DM-dataset.html",
        "license": "DUA / email request",
        "note": "Right physiology for insulin→glucose. Use UVA/Padova simulator until a DUA receipt exists.",
        "first_falsifier": "downloading a Kaggle mirror as if it were the official release",
    },
    {
        "name": "uva_padova_t1d",
        "status": "blocked",
        "source_url": "https://www.ncbi.nlm.nih.gov/pmc/articles/PMC4454102/",
        "license": "academic simulator distribution; not a CSV dump",
        "note": "Exact insulin→glucose world once the simulator binary/license is obtained. Not fetched this session.",
        "first_falsifier": "claiming a glucose arrow without the simulator license",
    },
    {
        "name": "fluxnet2015",
        "status": "blocked",
        "source_url": "https://fluxnet.org/data/fluxnet2015-dataset/",
        "license": "Tier 1 vs Tier 2 per site-year; registration required",
        "note": "Ecological sibling of SURFRAD. No site-year downloaded without a tier receipt.",
        "first_falsifier": "pooling Tier 2 site-years into a public extract",
    },
    {
        "name": "replogle_2022_gwps",
        "status": "blocked",
        "source_url": "https://gwps.wi.mit.edu/",
        "license": "GEO / study portal; genome-scale matrices are too large for this session",
        "note": "Accession receipt only. Single-gene tilts only; no dual-guide κ unless AB exists.",
        "first_falsifier": "a genome-wide DAG from CRISPRi screens",
    },
    {
        "name": "gbif_ebird",
        "status": "excluded",
        "source_url": "https://www.gbif.org/",
        "note": "Inclusion is the phenomenon. Selection wall.",
        "first_falsifier": "an occurrence→climate arrow from presence-only rows",
    },
    {
        "name": "mimic_ukb_gwas",
        "status": "excluded",
        "source_url": "n/a",
        "note": "MIMIC/UKB not walk-up public. GTEx/GWAS/eQTL have no factorial soft-intervention family (DATASET_ELIGIBILITY).",
        "first_falsifier": "certifying a GWAS DAG",
    },
    {
        "name": "depmap_hard_knockouts",
        "status": "excluded",
        "source_url": "https://depmap.org/",
        "note": "Hard knockouts with empty common support. Soft-intervention calculus does not apply.",
        "first_falsifier": "four-law on a knockout with no overlapping support",
    },
]


def main() -> None:
    for item in BLOCKS:
        name = item.pop("name")
        write_receipt(name, item)
        print("wrote", name, item["status"])


if __name__ == "__main__":
    main()
