#!/usr/bin/env python3
"""Build survey tables and receipts from whatever raw files landed."""

from __future__ import annotations

import csv
import json
import statistics
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import (
    RAW,
    REPORTS,
    TABLES,
    md5_file,
    run_survey,
    sha256_file,
    summarize_survey,
    token_hi_lo,
    write_receipt,
)

SEED = 20260813


def median(values: list[float]) -> float:
    return statistics.median(values)


def write_csv(path: Path, header: list[str], rows: list[list[object]]) -> None:
    with path.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(header)
        writer.writerows(rows)


def survey_and_receipt(name: str, table: Path, cluster: str, extra: dict) -> None:
    report = REPORTS / f"{name}.json"
    result = run_survey(table, cluster, report)
    extra = dict(extra)
    extra["table"] = str(table)
    extra["table_sha256"] = sha256_file(table)
    extra["n_table_rows"] = sum(1 for _ in table.open()) - 1
    if result.get("ok"):
        extra["status"] = extra.get("status", "surveyed")
        extra["survey"] = summarize_survey(report)
    else:
        extra["status"] = "blocked"
        extra["survey_error"] = result
    write_receipt(name, extra)
    print(f"{name}: {extra['status']} rows={extra['n_table_rows']}")


def process_s1s3() -> None:
    from build_s1s3 import main as s1s3

    s1s3()


def process_chambers() -> None:
    zpath = RAW / "chambers/lt_interventions_standard_v1.zip"
    if not zpath.exists() or zpath.stat().st_size < 1000:
        write_receipt("chambers_lt_standard_v1", {"status": "blocked", "note": "zip missing"})
        return
    md5 = md5_file(zpath)
    sha = sha256_file(zpath)
    pin_ok = (
        md5 == "476664d024f88e8b7640998bb5e9ee33"
        and sha == "8781960b5fff5d752f57566393d4b0f8706cac9b2cd2a3d3226442f17c7bab60"
    )
    base = RAW / "chambers/unpacked/lt_interventions_standard_v1"
    ref = base / "uniform_reference.csv"
    if not ref.exists():
        write_receipt(
            "chambers_lt_standard_v1",
            {
                "status": "blocked",
                "note": "zip present but not unpacked",
                "md5": md5,
                "byte_sha256": sha,
                "pin_ok": pin_ok,
            },
        )
        return
    with ref.open() as handle:
        reader = csv.DictReader(handle)
        rows = list(reader)
    reds = [float(row["red"]) for row in rows]
    greens = [float(row["green"]) for row in rows]
    rcut, gcut = median(reds), median(greens)
    out_rows = []
    for index, row in enumerate(rows):
        out_rows.append(
            [
                f"ref{index}",
                token_hi_lo(float(row["red"]), rcut),
                token_hi_lo(float(row["green"]), gcut),
                row["vis_1"],
                row["ir_1"],
                row["vis_2"],
                row["ir_2"],
            ]
        )
    table = TABLES / "chambers_reference_rg.csv"
    write_csv(
        table,
        ["cluster_id", "red_bin", "green_bin", "vis_1", "ir_1", "vis_2", "ir_2"],
        out_rows,
    )
    survey_and_receipt(
        "chambers_reference_rg",
        table,
        "cluster_id",
        {
            "source_url": "https://causalchamber.s3.eu-central-1.amazonaws.com/downloadables/lt_interventions_standard_v1.zip",
            "license": "CC BY 4.0",
            "byte_sha256": sha,
            "md5": md5,
            "pin_ok": pin_ok,
            "cluster_unit": "measurement row in uniform_reference",
            "ground_truth_authority": "physical actuation; independently sampled LED setpoints in the reference experiment",
            "note": "R and G are independently sampled in uniform_reference, so this 2x2 is a product-ish actuator square inside the reference law. Intervention files remain single-target; no joint-shift corners exist in the release.",
            "first_falsifier": "claiming curvature/composition from this release; claiming an arrow from the atlas",
            "missing_joint_corners": True,
        },
    )
    hidden = TABLES / "chambers_hidden_sensors.csv"
    write_csv(
        hidden,
        ["cluster_id", "vis_1", "ir_1", "vis_2", "ir_2"],
        [[f"h{index}", row["vis_1"], row["ir_1"], row["vis_2"], row["ir_2"]] for index, row in enumerate(rows)],
    )
    survey_and_receipt(
        "chambers_hidden_sensors",
        hidden,
        "cluster_id",
        {
            "source_url": "same zip; actuator columns withheld",
            "license": "CC BY 4.0",
            "byte_sha256": sha,
            "cluster_unit": "measurement row",
            "ground_truth_authority": "physical actuation withheld",
            "note": "Hide-metadata twin. Atlas must not invent context bits from sensors.",
            "first_falsifier": "recovering red/green arms from sensors and calling it certified",
        },
    )
    stacked = []
    for label, fname, red_s, green_s in [
        ("reference", "uniform_reference.csv", "no", "no"),
        ("red_strong", "uniform_red_strong.csv", "yes", "no"),
        ("green_strong", "uniform_green_strong.csv", "no", "yes"),
    ]:
        path = base / fname
        with path.open() as handle:
            for index, row in enumerate(csv.DictReader(handle)):
                if index >= 400:
                    break
                stacked.append(
                    [
                        f"{label}{index}",
                        red_s,
                        green_s,
                        row["vis_1"],
                        row["ir_1"],
                    ]
                )
    stack_table = TABLES / "chambers_single_target_stack.csv"
    write_csv(
        stack_table,
        ["cluster_id", "red_shift", "green_shift", "vis_1", "ir_1"],
        stacked,
    )
    survey_and_receipt(
        "chambers_single_target_stack",
        stack_table,
        "cluster_id",
        {
            "source_url": "same zip; reference+red_strong+green_strong only",
            "license": "CC BY 4.0",
            "cluster_unit": "measurement row",
            "ground_truth_authority": "single-target interventions; AB corner never collected",
            "note": "H-003 demonstration: the yes+yes corner is missing by construction.",
            "first_falsifier": "imputing the joint red+green strong corner",
        },
    )


def process_lalonde() -> None:
    treated = RAW / "lalonde/nswre74_treated.txt"
    control = RAW / "lalonde/nswre74_control.txt"
    cps = RAW / "lalonde/cps_controls.txt"
    if not treated.exists():
        write_receipt("lalonde_nsw", {"status": "blocked", "note": "NBER files missing"})
        return
    header = [
        "cluster_id",
        "sample",
        "treated",
        "age",
        "educ",
        "re74",
        "re75",
        "re78",
    ]

    def load(path: Path, sample: str, start: int) -> list[list[object]]:
        rows = []
        for index, line in enumerate(path.read_text().splitlines()):
            parts = line.split()
            if len(parts) < 10:
                continue
            treat, age, educ = parts[0], parts[1], parts[2]
            re74, re75, re78 = parts[7], parts[8], parts[9]
            rows.append(
                [
                    f"{sample}{start + index}",
                    sample,
                    "yes" if float(treat) >= 0.5 else "no",
                    age,
                    educ,
                    re74,
                    re75,
                    re78,
                ]
            )
        return rows

    rct = load(treated, "nswt", 0) + load(control, "nswc", 0)
    table = TABLES / "lalonde_nsw_rct.csv"
    write_csv(table, header, rct)
    survey_and_receipt(
        "lalonde_nsw_rct",
        table,
        "cluster_id",
        {
            "source_url": "https://users.nber.org/~rdehejia/data/",
            "license": "public research files posted by Dehejia with Lalonde permission",
            "cluster_unit": "person (NSW experimental sample)",
            "ground_truth_authority": "randomized job-training assignment in NSW",
            "note": "RCT table only. Atlas may find treated×other tokens; it must not certify an earnings DAG.",
            "first_falsifier": "inferring state-independent inclusion from selected rows",
        },
    )
    if cps.exists():
        twin = rct + load(cps, "cps", 0)[:800]
        twin_table = TABLES / "lalonde_nsw_cps_twin.csv"
        write_csv(twin_table, header, twin)
        survey_and_receipt(
            "lalonde_nsw_cps_twin",
            twin_table,
            "cluster_id",
            {
                "source_url": "https://users.nber.org/~rdehejia/data/cps_controls.txt",
                "license": "public research files",
                "cluster_unit": "person",
                "ground_truth_authority": "H-002 observational twin: CPS lookalikes are not randomized",
                "note": "Selected-looking mixture of RCT and observational controls. Rows alone must not recover the NSW assignment law.",
                "first_falsifier": "declaring state_independent inclusion from the twin table",
            },
        )


def process_surfrad() -> None:
    files = sorted((RAW / "surfrad").glob("tbl23*.dat"))
    if not files:
        write_receipt("surfrad_tbl_2023jan", {"status": "blocked", "note": "no daily files"})
        return
    rows = []
    for path in files:
        lines = path.read_text(errors="replace").splitlines()[2:]
        for line in lines:
            parts = line.split()
            if len(parts) < 40:
                continue
            year, jday, month, day, hour, minute = parts[0:6]
            zenith = float(parts[7])
            dw_solar = float(parts[8])
            temp = float(parts[38])
            rh = float(parts[40])
            if dw_solar < -90 or temp < -90:
                continue
            rows.append(
                (
                    int(hour),
                    zenith,
                    dw_solar,
                    temp,
                    rh,
                    f"{int(year):04d}{int(jday):03d}{int(hour):02d}{int(minute):02d}",
                )
            )
    if not rows:
        write_receipt("surfrad_tbl_2023jan", {"status": "blocked", "note": "no parsed rows"})
        return
    rad_cut = median([row[2] for row in rows])
    zen_cut = median([row[1] for row in rows])
    out = []
    for hour, zenith, dw_solar, temp, rh, cid in rows:
        out.append(
            [
                cid,
                token_hi_lo(dw_solar, rad_cut),
                token_hi_lo(zenith, zen_cut),
                f"{temp:.3f}",
                f"{rh:.3f}",
                f"{dw_solar:.3f}",
            ]
        )
    table = TABLES / "surfrad_tbl_jan2023.csv"
    write_csv(
        table,
        ["cluster_id", "rad_bin", "zenith_bin", "air_temp", "rh", "dw_solar"],
        out,
    )
    survey_and_receipt(
        "surfrad_tbl_jan2023",
        table,
        "cluster_id",
        {
            "source_url": "https://gml.noaa.gov/aftp/data/radiation/surfrad/Boulder_CO/2023/",
            "license": "NOAA public",
            "cluster_unit": "one-minute station record (Table Mountain)",
            "ground_truth_authority": "solar geometry precedes surface response; not a randomized assignment",
            "note": "10 days in January 2023. Diurnal cycle is a common driver. Nighttime/negative-lag still required before any radiation→temp claim.",
            "first_falsifier": "a season-as-tilt-of-elevation story, or an arrow without nighttime placebo",
        },
    )


def process_omni() -> None:
    path = RAW / "omni/omni2_2023.dat"
    if not path.exists():
        write_receipt("omni_2023", {"status": "blocked"})
        return
    rows = []
    for line in path.read_text().splitlines():
        parts = line.split()
        if len(parts) < 36:
            continue
        year, doy, hour = parts[0], parts[1], parts[2]
        bz = float(parts[16])  # Bz GSM
        speed = float(parts[24])  # flow speed
        dens = float(parts[23])
        dst = float(parts[34])
        if abs(bz) > 900 or speed > 9000 or abs(dst) > 9000:
            continue
        rows.append((f"{int(year):04d}{int(float(doy)):03d}{int(float(hour)):02d}", bz, speed, dens, dst))
    bz_cut = median([row[1] for row in rows])
    v_cut = median([row[2] for row in rows])
    out = [
        [cid, token_hi_lo(bz, bz_cut), token_hi_lo(speed, v_cut), f"{dens:.3f}", f"{dst:.3f}"]
        for cid, bz, speed, dens, dst in rows
    ]
    table = TABLES / "omni_2023_hourly.csv"
    write_csv(table, ["cluster_id", "bz_bin", "speed_bin", "density", "dst"], out)
    survey_and_receipt(
        "omni_2023_hourly",
        table,
        "cluster_id",
        {
            "source_url": "https://spdf.gsfc.nasa.gov/pub/data/omni/low_res_omni/omni2_2023.dat",
            "license": "NASA public",
            "cluster_unit": "hour",
            "ground_truth_authority": "solar-wind front precedes Dst; not a product assignment",
            "note": "Bz GSM and flow speed tokenized. Earth→Sun is forbidden. Time reversal is the kill.",
            "first_falsifier": "any Earth→Sun arrow or a contemporaneous undirected edge sold as causation",
        },
    )


def process_airfoil() -> None:
    path = RAW / "airfoil/airfoil_self_noise.dat"
    if not path.exists():
        write_receipt("uci_airfoil", {"status": "blocked"})
        return
    rows = []
    freqs, aoas = [], []
    parsed = []
    for index, line in enumerate(path.read_text().splitlines()):
        parts = line.replace(",", " ").split()
        if len(parts) < 6:
            parts = line.split("\t")
        if len(parts) < 6:
            continue
        freq, aoa, chord, vel, thick, spl = map(float, parts[:6])
        parsed.append((index, freq, aoa, chord, vel, thick, spl))
        freqs.append(freq)
        aoas.append(aoa)
    fcut, acut = median(freqs), median(aoas)
    out = [
        [
            f"af{index}",
            token_hi_lo(freq, fcut),
            token_hi_lo(aoa, acut),
            f"{vel:.5f}",
            f"{spl:.5f}",
        ]
        for index, freq, aoa, chord, vel, thick, spl in parsed
    ]
    table = TABLES / "uci_airfoil.csv"
    write_csv(table, ["cluster_id", "freq_bin", "aoa_bin", "velocity", "spl"], out)
    survey_and_receipt(
        "uci_airfoil",
        table,
        "cluster_id",
        {
            "source_url": "https://archive.ics.uci.edu/dataset/291/airfoil+self+noise",
            "license": "UCI / NASA airfoil self-noise, research reuse",
            "cluster_unit": "experimental configuration row (no run-id in the public file)",
            "ground_truth_authority": "passive aeroacoustic measurements; setup map is incomplete",
            "note": "CI small-physics adapter. Constitutive/passive: must not mint a unique actuator direction from SPL vs geometry.",
            "first_falsifier": "a calibrated causal-inference claim without a run-to-setup receipt",
        },
    )


def process_owid() -> None:
    life = RAW / "owid/life-expectancy.csv"
    gdp = RAW / "owid/gdp-per-capita.csv"
    if not life.exists() or not gdp.exists():
        write_receipt("owid_gdp_le", {"status": "blocked"})
        return
    lives = {}
    with life.open() as handle:
        for row in csv.DictReader(handle):
            key = (row.get("Code") or row.get("Entity"), row.get("Year"))
            val = row.get("Life expectancy") or row.get("Period life expectancy at birth - Sex: all - Age: 0")
            if key[0] and val:
                try:
                    lives[key] = float(val)
                except ValueError:
                    pass
    gdps = {}
    with gdp.open() as handle:
        reader = csv.DictReader(handle)
        value_key = [k for k in reader.fieldnames or [] if "gdp" in k.lower() or "income" in k.lower()]
        value_key = value_key[0] if value_key else reader.fieldnames[-1]
        for row in reader:
            key = (row.get("Code") or row.get("Entity"), row.get("Year"))
            val = row.get(value_key)
            if key[0] and val:
                try:
                    gdps[key] = float(val)
                except ValueError:
                    pass
    joined = []
    for key, le in lives.items():
        if key in gdps and key[1] and 2000 <= int(key[1]) <= 2019:
            joined.append((f"{key[0]}{key[1]}", key[0], int(key[1]), gdps[key], le))
    if not joined:
        write_receipt("owid_gdp_le", {"status": "blocked", "note": "join empty"})
        return
    gcut = median([row[3] for row in joined])
    ycut = 2010
    out = [
        [cid, token_hi_lo(gdp_v, gcut), "late" if year >= ycut else "early", f"{le:.4f}", f"{gdp_v:.4f}"]
        for cid, code, year, gdp_v, le in joined
    ]
    table = TABLES / "owid_gdp_le.csv"
    write_csv(table, ["cluster_id", "gdp_bin", "period", "life_expectancy", "gdp"], out)
    survey_and_receipt(
        "owid_gdp_le",
        table,
        "cluster_id",
        {
            "source_url": "https://ourworldindata.org/",
            "license": "OWID CC BY",
            "cluster_unit": "country-year",
            "ground_truth_authority": "none; registered abstain",
            "status": "abstain_required",
            "note": "Coordinated development. An arrow is method failure.",
            "first_falsifier": "emitting GDP→LE or LE→GDP as a certified direction",
        },
    )


def process_ice() -> None:
    co2p = RAW / "ice/edc-co2.txt"
    ddp = RAW / "ice/edc-dd.txt"
    if not co2p.exists() or not ddp.exists():
        write_receipt("epica_co2_dd", {"status": "blocked"})
        return
    co2_pts = []
    for line in co2p.read_text(errors="replace").splitlines():
        parts = line.split()
        if len(parts) < 3:
            continue
        try:
            age, co2 = float(parts[1]), float(parts[2])
        except ValueError:
            continue
        if 0 < age < 800000 and 150 < co2 < 400:
            co2_pts.append((age, co2))
    dd_pts = []
    for line in ddp.read_text(errors="replace").splitlines():
        parts = line.split()
        if len(parts) < 4:
            continue
        try:
            if len(parts) >= 5:
                age, dd, temp = float(parts[2]), float(parts[3]), float(parts[4])
            elif len(parts) >= 4:
                age, dd, temp = float(parts[2]), float(parts[3]), float("nan")
            else:
                continue
        except ValueError:
            continue
        if age == age and 0 < age < 800000 and temp == temp:
            dd_pts.append((age, dd, temp))
    # nearest-neighbor join on age, subsample every 20 CO2 points
    dd_pts.sort()
    out = []
    for index, (age, co2) in enumerate(co2_pts[::5]):
        nearest = min(dd_pts, key=lambda item: abs(item[0] - age))
        out.append((f"ice{index}", age, co2, nearest[1], nearest[2]))
    ccut = median([row[2] for row in out])
    tcut = median([row[4] for row in out])
    rows = [
        [cid, token_hi_lo(co2, ccut), token_hi_lo(temp, tcut), f"{age:.1f}", f"{co2:.2f}", f"{temp:.3f}"]
        for cid, age, co2, dd, temp in out
    ]
    table = TABLES / "epica_co2_temp.csv"
    write_csv(table, ["cluster_id", "co2_bin", "temp_bin", "age_yrbp", "co2", "temp"], rows)
    survey_and_receipt(
        "epica_co2_temp",
        table,
        "cluster_id",
        {
            "source_url": "https://www.ncei.noaa.gov/pub/data/paleo/icecore/antarctica/epica_domec/",
            "license": "NOAA Paleo / original Luthi and Jouzel citations required",
            "cluster_unit": "ice-core sample (age)",
            "ground_truth_authority": "none; registered abstain",
            "status": "abstain_required",
            "note": "Lead-lag is not identification. Both series respond to orbital forcing.",
            "first_falsifier": "a CO2→temp or temp→CO2 arrow from the ice core alone",
        },
    )


def process_wage() -> None:
    wage_p = RAW / "wage/FEDMINNFRWG.csv"
    un_p = RAW / "wage/UNRATE.csv"
    if not wage_p.exists() or not un_p.exists():
        write_receipt("minwage_unrate", {"status": "blocked"})
        return

    def load_fred(path: Path, name: str) -> dict[str, float]:
        out = {}
        with path.open() as handle:
            for row in csv.DictReader(handle):
                date = row.get("observation_date") or row.get("DATE")
                val = row.get(name) or row.get(path.stem)
                if not date or not val or val == ".":
                    continue
                try:
                    out[date[:7]] = float(val)
                except ValueError:
                    pass
        return out

    wages = load_fred(wage_p, "FEDMINNFRWG")
    uns = load_fred(un_p, "UNRATE")
    keys = sorted(set(wages) & set(uns))
    if not keys:
        write_receipt("minwage_unrate", {"status": "blocked", "note": "no overlapping months"})
        return
    wcut = median([wages[k] for k in keys])
    ycut = "1990-01"
    rows = [
        [k, token_hi_lo(wages[k], wcut), "late" if k >= ycut else "early", f"{uns[k]:.3f}", f"{wages[k]:.3f}"]
        for k in keys
    ]
    table = TABLES / "minwage_unrate.csv"
    write_csv(table, ["cluster_id", "wage_bin", "period", "unrate", "min_wage"], rows)
    survey_and_receipt(
        "minwage_unrate",
        table,
        "cluster_id",
        {
            "source_url": "https://fred.stlouisfed.org/series/FEDMINNFRWG and UNRATE",
            "license": "FRED public",
            "cluster_unit": "month (national aggregate — not the policy unit)",
            "ground_truth_authority": "none; registered abstain",
            "status": "abstain_required",
            "note": "National min-wage series vs unemployment. Not a Card-Krueger panel. An arrow is method failure.",
            "first_falsifier": "minwage→unemployment as a certified direction from this aggregate",
        },
    )


def process_glodap() -> None:
    path = RAW / "glodap/arctic_head.csv"
    if not path.exists():
        write_receipt("glodap_arctic_head", {"status": "blocked"})
        return
    rows = []
    with path.open(errors="replace") as handle:
        reader = csv.DictReader(handle)
        for index, row in enumerate(reader):
            try:
                dic = float(row.get("G2tco2") or "nan")
                talk = float(row.get("G2talk") or "nan")
                ph = float(row.get("G2phtsinsitutp") or "nan")
                temp = float(row.get("G2temperature") or "nan")
            except (TypeError, ValueError):
                continue
            if any(value != value for value in (dic, talk, ph, temp)):
                continue
            if min(dic, talk, ph) <= -900:
                continue
            rows.append((f"g{index}", dic, talk, ph, temp))
            if len(rows) >= 4000:
                break
    if len(rows) < 20:
        write_receipt("glodap_arctic_head", {"status": "blocked", "note": "too few valid bottles"})
        return
    fcut = median([row[1] for row in rows])
    acut = median([row[2] for row in rows])
    out = [
        [cid, token_hi_lo(dic, fcut), token_hi_lo(talk, acut), f"{ph:.4f}", f"{temp:.3f}"]
        for cid, dic, talk, ph, temp in rows
    ]
    table = TABLES / "glodap_arctic_head.csv"
    write_csv(table, ["cluster_id", "dic_bin", "talk_bin", "ph", "temperature"], out)
    survey_and_receipt(
        "glodap_arctic_head",
        table,
        "cluster_id",
        {
            "source_url": "https://www.ncei.noaa.gov/data/oceans/ncei/ocads/data/0283442/GLODAPv2.2023_Arctic_Ocean.csv",
            "license": "GLODAPv2.2023 public / cite Lauvset et al.",
            "cluster_unit": "bottle (first ~4k valid Arctic bottles of a truncated download)",
            "ground_truth_authority": "constitutive carbonate chemistry; no actuator receipt",
            "note": "S3: pH is constrained by DIC (tCO2) and alkalinity. Reverse direction must abstain. Truncated file is not the full Arctic product.",
            "first_falsifier": "discovering an actuator from the passive chemistry table",
        },
    )


def process_sachs() -> None:
    data_dir = RAW / "sachs/unpacked/Data Files"
    if not data_dir.exists():
        write_receipt("sachs_2005", {"status": "blocked"})
        return
    labeled = []
    hidden = []
    for path in sorted(data_dir.glob("*.csv")):
        cond = path.stem.replace("cd3cd28", "stim")
        stim = "yes" if "cd3cd28" in path.stem or path.stem.startswith("stim") or "cd3" in path.stem else "no"
        inhib = "yes" if any(tag in path.stem for tag in ("akt", "g0076", "psit", "u0126", "ly")) else "no"
        with path.open() as handle:
            reader = csv.DictReader(handle)
            for index, row in enumerate(reader):
                if index >= 80:
                    break
                cid = f"{path.stem}{index}"
                labeled.append(
                    [
                        cid,
                        stim,
                        inhib,
                        row.get("Raf") or row.get('"Raf"') or list(row.values())[0],
                        row.get("Erk") or list(row.values())[5],
                        row.get("Akt") or list(row.values())[6],
                        row.get("PKC") or list(row.values())[8],
                    ]
                )
                hidden.append(
                    [
                        cid,
                        row.get("Raf") or list(row.values())[0],
                        row.get("Erk") or list(row.values())[5],
                        row.get("Akt") or list(row.values())[6],
                        row.get("PKC") or list(row.values())[8],
                    ]
                )
    lab = TABLES / "sachs_labeled.csv"
    hid = TABLES / "sachs_hidden.csv"
    write_csv(lab, ["cluster_id", "stim", "inhibitor", "Raf", "Erk", "Akt", "PKC"], labeled)
    write_csv(hid, ["cluster_id", "Raf", "Erk", "Akt", "PKC"], hidden)
    survey_and_receipt(
        "sachs_labeled",
        lab,
        "cluster_id",
        {
            "source_url": "https://doi.org/10.5281/zenodo.7681811",
            "license": "CC BY 4.0",
            "cluster_unit": "cell (flow-cytometry event); reagent is the intervention label",
            "ground_truth_authority": "declared reagents; community graph is expert consensus not construction",
            "note": "Tier A labels. Atlas may find stim×inhibitor. Do not treat the community DAG as a certificate.",
            "first_falsifier": "peeling a UNIQUE_TARGET from unlabeled cells",
        },
    )
    survey_and_receipt(
        "sachs_hidden",
        hid,
        "cluster_id",
        {
            "source_url": "same Zenodo; reagent labels withheld",
            "license": "CC BY 4.0",
            "cluster_unit": "cell",
            "ground_truth_authority": "withheld",
            "note": "Hide-labels twin. Must abstain from orientation.",
            "first_falsifier": "recovering reagent arms from protein levels and calling it certified",
        },
    )


def process_access_pages() -> None:
    pages = {
        "oregon_ohie": (
            RAW / "oregon/landing.html",
            "https://www.nber.org/research/data/oregon-health-insurance-experiment-data",
            "NBER Oregon public-use files require account/terms. Landing page fetched; microdata not downloaded.",
            "blocked",
        ),
        "earthscope_ph5": (
            RAW / "earthscope/ph5ws.html",
            "https://service.earthscope.org/ph5ws/dataselect/1/",
            "PH5WS endpoint responded. No bounded shot gather downloaded (needs experiment-specific QC receipt).",
            "blocked",
        ),
        "ausgrid_solar": (
            RAW / "ausgrid/2012-2013.zip",
            "https://www.ausgrid.com.au/-/media/Documents/Data-to-share/Solar-home-electricity-data/Solar-home-half-hour-data---1-July-2012-to-30-June-2013.zip",
            "Zip did not land this session (empty directory). Utility page may have moved.",
            "blocked",
        ),
        "bdg2": (
            RAW / "bdg2/zenodo.json",
            "https://zenodo.org/records/3887306",
            "Zenodo archive is 595 MB. Not downloaded. CC BY 4.0. Building is the unit once a slice exists.",
            "blocked",
        ),
        "jump_cp": (
            RAW / "jump_gallery.html",
            "https://registry.opendata.aws/cellpainting-gallery/",
            "Metadata/docs fetched. No images downloaded (CC0 gallery is huge). Compound=regime, morphology=state.",
            "blocked",
        ),
        "sciplex_gse139944": (
            RAW / "sciplex_geo.html",
            "https://www.ncbi.nlm.nih.gov/geo/query/acc.cgi?acc=GSE139944",
            "GEO accession page fetched. Matrices not downloaded (650k cells). Plate is the unit.",
            "blocked",
        ),
        "scperturb": (
            RAW / "scperturb_zenodo.json",
            "https://zenodo.org/records/10044268",
            "Zenodo API fetched. h5ad files not downloaded (per-study, do not pool scores).",
            "blocked",
        ),
        "nutnet_nxp": (
            RAW / "nutnet/datadois.html",
            "https://nutnet.org/datadois",
            "Published-subset DOIs listed. Full N×P plot table requires a request. EDI metadata 403 this session.",
            "blocked",
        ),
        "dream4": (
            RAW / "dream4.html",
            "https://www.bioconductor.org/packages/release/data/experiment/html/DREAM4.html",
            "Bioconductor landing fetched. ExperimentHub package not installed.",
            "blocked",
        ),
        "drugcomb": (
            RAW / "drugcomb_meta.json",
            "https://api.drugcomb.org/summary",
            "API sample may be empty/failed. Scalar synergy only (RoseLark: no MIC κ manifest).",
            "blocked",
        ),
    }
    for name, (path, url, note, status) in pages.items():
        write_receipt(
            name,
            {
                "status": status,
                "source_url": url,
                "landing_present": path.exists() and path.stat().st_size > 0,
                "landing_sha256": sha256_file(path) if path.exists() and path.stat().st_size > 0 else None,
                "note": note,
                "first_falsifier": "treating a landing page as an executed extract",
            },
        )
        print(f"{name}: {status}")


def main() -> None:
    process_s1s3()
    process_chambers()
    process_lalonde()
    process_surfrad()
    process_omni()
    process_airfoil()
    process_owid()
    process_ice()
    process_wage()
    process_glodap()
    process_sachs()
    process_access_pages()
    from write_blocked_receipts import main as blocked

    blocked()


if __name__ == "__main__":
    main()
