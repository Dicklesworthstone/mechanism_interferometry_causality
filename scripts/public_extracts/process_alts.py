#!/usr/bin/env python3
"""Process alternate-source extracts that landed after the first pass."""

from __future__ import annotations

import csv
import gzip
import statistics
import sys
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import RAW, TABLES, sha256_file, write_receipt
from process_extracts import survey_and_receipt, token_hi_lo, write_csv, median

ALT = RAW / "alt"


def process_heartsteps() -> None:
    path = ALT / "heartsteps/suggestions.csv"
    if not path.exists():
        write_receipt("heartsteps_v1", {"status": "blocked", "note": "suggestions.csv missing"})
        return
    rows = []
    with path.open() as handle:
        for row in csv.DictReader(handle):
            user = row.get("user.index")
            dec = row.get("decision.index")
            if not user or dec in (None, ""):
                continue
            rand = "yes" if str(row.get("is.randomized")).lower() in {"true", "1", "yes"} else "no"
            send = "yes" if str(row.get("send")).lower() in {"true", "1", "yes"} else "no"
            steps = row.get("jbsteps30") or row.get("gfsteps30") or ""
            pre = row.get("jbsteps30pre") or row.get("gfsteps30pre") or ""
            temp = row.get("dec.temperature") or ""
            try:
                float(steps)
                float(pre)
            except ValueError:
                continue
            rows.append([f"u{user}d{dec}", rand, send, steps, pre, temp or "0"])
    table = TABLES / "heartsteps_suggestions.csv"
    write_csv(
        table,
        ["cluster_id", "randomized", "send", "jbsteps30", "jbsteps30pre", "dec_temp"],
        rows,
    )
    survey_and_receipt(
        "heartsteps_v1",
        table,
        "cluster_id",
        {
            "source_url": "https://github.com/klasnja/HeartStepsV1/tree/main/data_files",
            "license": "study PI GitHub release (Klasnja / Murphy HeartSteps V1)",
            "cluster_unit": "decision point (user.index × decision.index), not person-period iid",
            "ground_truth_authority": "micro-randomized send decision when is.randomized is true",
            "note": "Public GitHub extract. Atlas may see randomized×send. Do not treat send as a discovered DAG edge.",
            "first_falsifier": "clustering at person instead of decision point",
            "alt_of": "heartsteps_mrt DUA block",
        },
    )


def process_jump() -> None:
    well_p = ALT / "jump/well.csv.gz"
    plate_p = ALT / "jump/plate.csv.gz"
    if not well_p.exists():
        write_receipt("jump_metadata", {"status": "blocked"})
        return
    plates = {}
    with gzip.open(plate_p, "rt") as handle:
        for row in csv.DictReader(handle):
            plates[row["Metadata_Plate"]] = row.get("Metadata_PlateType") or "unknown"
    rows = []
    with gzip.open(well_p, "rt") as handle:
        for index, row in enumerate(csv.DictReader(handle)):
            if index % 20 != 0:
                continue
            if len(rows) >= 8000:
                break
            ptype = plates.get(row["Metadata_Plate"], "unknown")
            kind = "compound" if "COMPOUND" in ptype and "EMPTY" not in ptype else "empty_or_other"
            src = row["Metadata_Source"]
            src_bin = "source1" if src == "source_1" else "other_source"
            rows.append(
                [
                    f"{src}_{row['Metadata_Plate']}_{row['Metadata_Well']}",
                    src_bin,
                    kind,
                    row.get("Metadata_JCP2022") or "NA",
                ]
            )
    table = TABLES / "jump_well_metadata.csv"
    write_csv(table, ["cluster_id", "source_bin", "plate_kind", "jcp_code"], rows)
    survey_and_receipt(
        "jump_metadata",
        table,
        "cluster_id",
        {
            "source_url": "https://github.com/jump-cellpainting/datasets/tree/main/metadata",
            "license": "JUMP / Cell Painting Gallery (CC0 images; metadata from public repo)",
            "cluster_unit": "well (source×plate×well)",
            "ground_truth_authority": "plate-type labels only; no morphology state in this slice",
            "note": "Metadata-only adapter. jcp_id is a token/id, not an outcome. No image embeddings.",
            "first_falsifier": "calling a compound DAG from well labels without morphology",
            "alt_of": "jump_cp image dump",
        },
    )


def process_bdg2() -> None:
    weather = ALT / "bdg2/weather.csv"
    elec = ALT / "bdg2/electricity.csv"
    if not weather.exists() or not elec.exists():
        write_receipt("bdg2_panther", {"status": "blocked"})
        return
    wrows = {}
    temps = []
    winds = []
    with weather.open() as handle:
        for row in csv.DictReader(handle):
            if row.get("site_id") != "Panther":
                continue
            ts = row["timestamp"]
            try:
                temp = float(row["airTemperature"])
                wind = float(row["windSpeed"] or "nan")
            except ValueError:
                continue
            if wind != wind:
                wind = 0.0
            wrows[ts] = (temp, wind)
            temps.append(temp)
            winds.append(wind)
    tcut, vcut = median(temps), median(winds)
    # scan electricity for Panther_education_Violet
    building = "Panther_education_Violet"
    joined = []
    with elec.open() as handle:
        reader = csv.DictReader(handle)
        if building not in (reader.fieldnames or []):
            building = [c for c in reader.fieldnames or [] if c.startswith("Panther_")][0]
        for row in reader:
            ts = row["timestamp"]
            if ts not in wrows:
                continue
            val = row.get(building) or ""
            if val in ("", "NA"):
                continue
            try:
                kwh = float(val)
            except ValueError:
                continue
            temp, wind = wrows[ts]
            joined.append((ts.replace(" ", "T"), temp, wind, kwh))
            if len(joined) >= 4000:
                break
    if len(joined) < 20:
        write_receipt("bdg2_panther", {"status": "blocked", "note": "too few joined hours"})
        return
    out = [
        [ts, token_hi_lo(temp, tcut), token_hi_lo(wind, vcut), f"{kwh:.5f}", f"{temp:.3f}"]
        for ts, temp, wind, kwh in joined
    ]
    table = TABLES / "bdg2_panther.csv"
    write_csv(table, ["cluster_id", "temp_bin", "wind_bin", "electricity", "air_temp"], out)
    survey_and_receipt(
        "bdg2_panther",
        table,
        "cluster_id",
        {
            "source_url": "https://github.com/buds-lab/building-data-genome-project-2 (LFS media, not 595MB zenodo zip)",
            "license": "CC BY 4.0",
            "cluster_unit": "building-hour (Panther_education_Violet)",
            "ground_truth_authority": "weather is exogenous to a single building meter; occupancy still confounds",
            "note": "GitHub LFS slice. Holiday/future-weather placebos still required. Building is the unit.",
            "first_falsifier": "occupancy inferred as a cause of outdoor temperature",
            "alt_of": "bdg2 595MB zenodo zip",
        },
    )


def process_ausgrid() -> None:
    path = ALT / "ausgrid/data_2011-2012.csv"
    if not path.exists() or path.stat().st_size < 1000:
        write_receipt(
            "ausgrid_customer12",
            {
                "status": "blocked",
                "note": "official zip 404; customer-12 GitHub slice missing",
                "alt_tried": "https://pierreh.eu/downloads/Ausgrid_solar_home_data.zip and GitHub customer/12",
            },
        )
        return
    rows = []
    gens = []
    parsed = []
    with path.open() as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            ts = row.get("") or row.get("timestamp") or list(row.values())[0]
            try:
                gc = float(row["GC"])
                gg = float(row["GG"])
                when = datetime.fromisoformat(ts)
            except (KeyError, ValueError):
                continue
            parsed.append((ts.replace(" ", "T"), when, gc, gg))
            gens.append(gg)
    if not parsed:
        write_receipt("ausgrid_customer12", {"status": "blocked", "note": "unparsed"})
        return
    out = []
    for ts, when, gc, gg in parsed:
        season = "winter" if when.month in {6, 7, 8} else "other"
        daypart = "day" if 7 <= when.hour <= 17 else "night"
        out.append([ts, season, daypart, f"{gc:.4f}", f"{gg:.4f}"])
    table = TABLES / "ausgrid_customer12.csv"
    write_csv(table, ["cluster_id", "season", "daypart", "load_gc", "pv_gg"], out)
    survey_and_receipt(
        "ausgrid_customer12",
        table,
        "cluster_id",
        {
            "source_url": "https://github.com/pierre-haessig/ausgrid-solar-data/blob/master/customer/12/data_2011-2012.csv",
            "license": "Ausgrid solar-home data via documented GitHub extract (official zip 404)",
            "cluster_unit": "half-hour for customer 12 (2011-2012)",
            "ground_truth_authority": "calendar daypart/season are not randomized; PV generation is the physical response to irradiance, which is not in this slice",
            "note": "Official Ausgrid zip is gone. This is customer 12 only. GG is generation, not site irradiance. Not a randomized intervention.",
            "first_falsifier": "load → irradiance or treating calendar bins as actuators",
            "alt_of": "ausgrid official 2012-2013.zip 404",
        },
    )


def process_dream4() -> None:
    base = (
        ALT
        / "dream4/unpacked/DREAM4 in-silico challenge/Size 10/DREAM4 training data/insilico_size10_1"
    )
    ko = base / "insilico_size10_1_knockouts.tsv"
    wt = base / "insilico_size10_1_wildtype.tsv"
    if not ko.exists():
        write_receipt("dream4_size10", {"status": "blocked"})
        return

    def parse_tsv(path: Path) -> list[list[str]]:
        text = path.read_text().replace('""', "\t").replace('"', "")
        lines = [line for line in text.splitlines() if line.strip()]
        return [line.split() for line in lines]

    wt_rows = parse_tsv(wt)
    ko_rows = parse_tsv(ko)
    # header then one WT row; KO file is header + 10 rows (gene i knocked out)
    genes = ko_rows[0]
    out = []
    if len(wt_rows) > 1:
        vals = wt_rows[1]
        out.append(["wt0", "no", "no", vals[2], vals[3], vals[4]])
    for index, vals in enumerate(ko_rows[1:]):
        if len(vals) < 5:
            continue
        g1 = "yes" if index == 0 else "no"
        g2 = "yes" if index == 1 else "no"
        out.append([f"ko{index}", g1, g2, vals[2], vals[3], vals[4]])
    table = TABLES / "dream4_size10_ko.csv"
    write_csv(table, ["cluster_id", "g1_ko", "g2_ko", "G3", "G4", "G5"], out)
    survey_and_receipt(
        "dream4_size10",
        table,
        "cluster_id",
        {
            "source_url": "https://gnw.sourceforge.net/resources/DREAM4%20in%20silico%20challenge.zip",
            "license": "DREAM4 in silico challenge public archive (GeneNetWeaver)",
            "cluster_unit": "simulated experiment (wildtype or single knockout)",
            "ground_truth_authority": "simulator graph; single-gene KOs only so AB is missing",
            "note": "GNW mirror. Dual-knockout files exist separately. This table is WT+single KO so corner 11 is absent (H-003).",
            "first_falsifier": "a genome-wide DAG from 10 single KOs, or imputing dual KO",
            "alt_of": "Bioconductor DREAM4 ExperimentHub (not installed)",
        },
    )


def process_still_blocked() -> None:
    write_receipt(
        "nutnet_nxp_alt",
        {
            "status": "blocked",
            "source_url": "https://datadryad.org/stash/dataset/doi:10.5061/dryad.qp25093",
            "note": "Dryad download API 401 without token. Figshare 4037022 is a species list xlsx, not the N×P plot table. EDI pasta still 403.",
            "landing_sha256": sha256_file(ALT / "nutnet/plant_species.xlsx")
            if (ALT / "nutnet/plant_species.xlsx").exists()
            else None,
            "alt_of": "nutnet_nxp",
        },
    )
    write_receipt(
        "drugcomb_alt",
        {
            "status": "blocked",
            "source_url": "https://zenodo.org/records/11102665",
            "note": "Zenodo summary_table_v1.4.csv is 193 MB; comboFM zip is 150 MB. API drugcomb.org timed out. Not fetched this session. Still scalar synergy only.",
            "alt_of": "drugcomb",
        },
    )
    write_receipt(
        "oregon_ohie_alt",
        {
            "status": "blocked",
            "source_url": "https://dataverse.harvard.edu/api/search?q=Oregon%20Health%20Insurance%20Experiment",
            "note": "Harvard Dataverse search JSON fetched earlier. Microdata still account-gated. No public CSV without NBER/ICPSR terms.",
            "alt_of": "oregon_ohie",
        },
    )
    write_receipt(
        "scperturb_alt",
        {
            "status": "blocked",
            "source_url": "https://zenodo.org/records/10044268",
            "note": "Zenodo API lists large h5ad files. No small CSV extract. Per-study scoring still required.",
            "alt_of": "scperturb",
        },
    )
    write_receipt(
        "sciplex_alt",
        {
            "status": "blocked",
            "source_url": "https://maayanlab.cloud/Harmonizome/dataset/Sci-Plex+Drug+Perturbation+Signatures",
            "note": "Harmonizome HTML landing fetched. Signature matrices not downloaded (still large). Plate remains the unit.",
            "alt_of": "sciplex_gse139944",
        },
    )
    write_receipt(
        "replogle_alt",
        {
            "status": "blocked",
            "source_url": "https://maayanlab.cloud/Harmonizome/dataset/Replogle+et+al.,+Cell,+2022+K562+Genome-wide+Perturb-seq+Gene+Perturbation+Signatures",
            "note": "Harmonizome landing fetched. No small processed slice downloaded.",
            "alt_of": "replogle_2022_gwps",
        },
    )


def main() -> None:
    process_heartsteps()
    process_jump()
    process_bdg2()
    process_ausgrid()
    process_dream4()
    process_still_blocked()


if __name__ == "__main__":
    main()
