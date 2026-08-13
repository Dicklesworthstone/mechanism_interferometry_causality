#!/usr/bin/env python3
"""Build survey tables from *replacements* for still-blocked extracts.

These are different public datasets that fill the same contract holes,
not more mirrors of NutNet / DrugComb / Oregon / EarthScope / FLUXNET /
sci-Plex / UVA-Padova. Authority remains proposal_only. A complete square
is not an arrow.
"""

from __future__ import annotations

import csv
import gzip
import sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import RAW, TABLES, sha256_file, write_receipt
from process_extracts import median, survey_and_receipt, token_hi_lo, write_csv

REP = RAW / "rep"


def _slug(text: object) -> str:
    cleaned = str(text).replace("\r", " ").replace("\n", " ").strip()
    return "".join(ch if ch.isalnum() or ch in "-_." else "_" for ch in cleaned) or "NA"


def _finite(text: str) -> float | None:
    if text is None:
        return None
    text = str(text).strip()
    if text in {"", "NA", "NaN", "nan", "-9999", "-9999.0", "?", "."}:
        return None
    try:
        value = float(text)
    except ValueError:
        return None
    if value != value:
        return None
    return value


def process_npk() -> None:
    """MASS::npk fractional N×P×K — NutNet hole (factorial ecology, missing corners)."""
    path = REP / "ecology/npk.csv"
    if not path.exists() or path.stat().st_size < 50:
        write_receipt("npk_factorial", {"status": "blocked", "note": "npk.csv missing"})
        return
    rows = []
    with path.open() as handle:
        for row in csv.DictReader(handle):
            yld = _finite(row.get("yield") or "")
            if yld is None:
                continue
            n_tok = "yes" if str(row.get("N")).strip() == "1" else "no"
            p_tok = "yes" if str(row.get("P")).strip() == "1" else "no"
            k_tok = "yes" if str(row.get("K")).strip() == "1" else "no"
            block = row.get("block") or "NA"
            rname = row.get("rownames") or str(len(rows))
            rows.append([f"b{block}r{rname}", n_tok, p_tok, k_tok, f"{yld:.3f}"])
    table = TABLES / "npk_factorial.csv"
    write_csv(table, ["cluster_id", "n_on", "p_on", "k_on", "yield"], rows)
    survey_and_receipt(
        "npk_factorial",
        table,
        "cluster_id",
        {
            "source_url": "https://raw.githubusercontent.com/vincentarelbundock/Rdatasets/master/csv/MASS/npk.csv",
            "license": "MASS (Venables & Ripley) via Rdatasets; original Yates pea factorial",
            "cluster_unit": "plot (block × row)",
            "ground_truth_authority": "assigned N/P/K half-replicate; NPK interaction confounded",
            "note": "Replacement for NutNet N×P. Pairwise N×P / N×K / P×K are complete; the 2^(3-1) hole is the three-way NPK cell. Not a grassland biodiversity network.",
            "first_falsifier": "imputing the confounded NPK cell or calling yield→N",
            "replaces": "nutnet_nxp",
        },
    )


def process_gomez() -> None:
    """Gomez rice split-split: nitrogen × management — second ecology square."""
    path = REP / "ecology/gomez.splitsplit.csv"
    if not path.exists() or path.stat().st_size < 50:
        write_receipt("gomez_splitsplit", {"status": "blocked"})
        return
    rows = []
    with path.open() as handle:
        for row in csv.DictReader(handle):
            yld = _finite(row.get("yield") or "")
            if yld is None:
                continue
            nitro = _finite(row.get("nitro") or "")
            if nitro is None:
                continue
            n_tok = "n0" if nitro == 0 else "n_pos"
            mgmt = (row.get("management") or "NA").strip()
            mgmt_tok = "minimum" if mgmt.lower() == "minimum" else "not_minimum"
            cluster = f"{row.get('rep')}_{row.get('row')}_{row.get('col')}_{row.get('rownames')}"
            rows.append([cluster, n_tok, mgmt_tok, f"{yld:.4f}", row.get("gen") or ""])
    table = TABLES / "gomez_splitsplit.csv"
    write_csv(table, ["cluster_id", "nitro_bin", "mgmt_bin", "yield", "genotype"], rows)
    survey_and_receipt(
        "gomez_splitsplit",
        table,
        "cluster_id",
        {
            "source_url": "https://raw.githubusercontent.com/vincentarelbundock/Rdatasets/master/csv/agridat/gomez.splitsplit.csv",
            "license": "agridat (Kwiatkowski) via Rdatasets; Gomez & Gomez rice agronomy",
            "cluster_unit": "plot (rep × row × col)",
            "ground_truth_authority": "assigned nitrogen and management; genotype is a third factor",
            "note": "Second NutNet-hole replacement. Split-split, not a crossed grassland N×P. Do not freeze nitro→yield as MIC orientation.",
            "first_falsifier": "treating management as a discovered nutrient DAG",
            "replaces": "nutnet_nxp",
        },
    )


def process_langli_combo() -> None:
    """Phase-I combo dose table — DrugComb hole (combo corners, not synergy scores)."""
    path = REP / "combo/dose_level.csv"
    dlt_path = REP / "combo/observed_dlt.csv"
    if not path.exists():
        write_receipt("langli_dose_combo", {"status": "blocked", "note": "dose_level.csv missing"})
        return
    drugs_by_combo: dict[tuple[str, str], list[str]] = defaultdict(list)
    rows_by_key: dict[tuple[str, str, str], list[dict[str, str]]] = defaultdict(list)
    with path.open() as handle:
        for row in csv.DictReader(handle):
            nct = row.get("NCTID") or ""
            cid = row.get("CombnID") or ""
            level = row.get("Dose_Level") or ""
            drug = (row.get("Drug_Name") or "").strip().lower()
            if not nct or not drug:
                continue
            drugs_by_combo[(nct, cid)].append(drug)
            rows_by_key[(nct, cid, level)].append(row)
    dlt_by_key: dict[tuple[str, str, str], float] = {}
    if dlt_path.exists():
        with dlt_path.open() as handle:
            for row in csv.DictReader(handle):
                freq = _finite(row.get("Observed_Frequency") or "")
                total = _finite(row.get("Total") or "")
                if freq is None or total is None or total <= 0:
                    continue
                key = (row.get("NCTID") or "", row.get("CombnID") or "", row.get("At_Level") or "")
                dlt_by_key[key] = max(dlt_by_key.get(key, 0.0), freq / total)
    out = []
    for (nct, cid), names in drugs_by_combo.items():
        uniq = []
        seen = set()
        for name in names:
            if name not in seen:
                seen.add(name)
                uniq.append(name)
        if len(uniq) < 2:
            continue
        drug_a, drug_b = uniq[0], uniq[1]
        for (nct2, cid2, level), items in rows_by_key.items():
            if (nct2, cid2) != (nct, cid):
                continue
            present = {(item.get("Drug_Name") or "").strip().lower() for item in items}
            a_on = "yes" if drug_a in present else "no"
            b_on = "yes" if drug_b in present else "no"
            rate = dlt_by_key.get((nct, cid, level))
            rate_s = f"{rate:.4f}" if rate is not None else "NA"
            out.append([_slug(f"{nct}_{cid}_{level}"), a_on, b_on, rate_s, f"{len(present)}"])
    if len(out) < 8:
        write_receipt(
            "langli_dose_combo",
            {"status": "blocked", "note": f"too few paired dose rows ({len(out)})"},
        )
        return
    table = TABLES / "langli_dose_combo.csv"
    write_csv(table, ["cluster_id", "drug_a_on", "drug_b_on", "dlt_rate", "n_drugs"], out)
    survey_and_receipt(
        "langli_dose_combo",
        table,
        "cluster_id",
        {
            "source_url": "https://github.com/langli-lab/drugcombo-data",
            "license": "public GitHub release (langli-lab Phase I combo / DLT tables)",
            "cluster_unit": "trial × combination × dose level",
            "ground_truth_authority": "protocol dose-escalation, not a synergy screen",
            "note": "Replacement for DrugComb scalar-synergy dump. Phase I DLT/MTD, not high-throughput Bliss/Loewe. Missing AB corners are escalation paths (H-003), not biology.",
            "first_falsifier": "a synergy DAG from presence bits, or treating DLT rate as a modular response",
            "replaces": "drugcomb",
        },
    )


def process_star() -> None:
    """Project STAR kindergarten class-size RCT — Oregon hole."""
    path = REP / "rct/STAR.csv"
    if not path.exists():
        write_receipt("star_kindergarten", {"status": "blocked"})
        return
    rows = []
    with path.open() as handle:
        for row in csv.DictReader(handle):
            assigned = (row.get("stark") or "").strip()
            mathk = _finite(row.get("mathk") or "")
            readk = _finite(row.get("readk") or "")
            school = (row.get("schoolidk") or "").strip()
            if assigned not in {"small", "regular", "regular+aide"} or mathk is None or not school:
                continue
            class_tok = "small" if assigned == "small" else "not_small"
            lunch = (row.get("lunchk") or "").strip().lower()
            lunch_tok = "free" if lunch == "free" else "not_free"
            student = row.get("rownames") or str(len(rows))
            rows.append(
                [
                    f"s{school.zfill(4)}_{student}",
                    class_tok,
                    lunch_tok,
                    f"{mathk:.1f}",
                    f"{readk:.1f}" if readk is not None else "NA",
                ]
            )
    table = TABLES / "star_kindergarten.csv"
    write_csv(table, ["cluster_id", "class_small", "lunch_free", "mathk", "readk"], rows)
    survey_and_receipt(
        "star_kindergarten",
        table,
        "cluster_id",
        {
            "source_url": "https://github.com/vincentarelbundock/Rdatasets (AER/STAR) / Tennessee STAR public extract",
            "license": "public STAR extract via Rdatasets/AER",
            "cluster_unit": "student in kindergarten; assignment was within school — do not treat students as the randomization unit for school shocks",
            "ground_truth_authority": "class-size assignment (small vs regular / regular+aide). lunch is a covariate, not assigned.",
            "note": "Replacement for Oregon OHIE microdata. This is a class-size RCT, not a lottery-insurance ITT/LATE table. A complete class×lunch square is design+covariate, not a discovered DAG.",
            "first_falsifier": "lunch→score as an audited arrow, or clustering at the student when the question is school-level",
            "replaces": "oregon_ohie",
        },
    )


def process_ihdp() -> None:
    """Hill NPCI IHDP replicate 1 — semi-synthetic RCT twin."""
    path = REP / "rct/ihdp_npci_1.csv"
    if not path.exists():
        write_receipt("ihdp_npci_1", {"status": "blocked"})
        return
    rows = []
    with path.open() as handle:
        reader = csv.reader(handle)
        for index, cols in enumerate(reader):
            if len(cols) < 12:
                continue
            treat = cols[0].strip()
            y = _finite(cols[1])
            if treat not in {"0", "1"} or y is None:
                continue
            cov = None
            for col in cols[11:]:
                if col.strip() in {"0", "1"}:
                    cov = col.strip()
                    break
            if cov is None:
                cov = "0"
            rows.append([f"ihdp{index:04d}", "yes" if treat == "1" else "no", "yes" if cov == "1" else "no", f"{y:.6f}"])
    table = TABLES / "ihdp_npci_1.csv"
    write_csv(table, ["cluster_id", "treated", "cov_bin", "outcome"], rows)
    survey_and_receipt(
        "ihdp_npci_1",
        table,
        "cluster_id",
        {
            "source_url": "https://raw.githubusercontent.com/AMLab-Amsterdam/CEVAE/master/datasets/IHDP/csv/ihdp_npci_1.csv",
            "license": "public Hill NPCI / CEVAE IHDP replicate (Infant Health and Development Program covariates + simulated outcomes)",
            "cluster_unit": "child (simulated replicate 1)",
            "ground_truth_authority": "semi-synthetic: treatment and covariates from IHDP, outcomes from Hill NPCI simulator",
            "note": "Replacement for Oregon. Not real later-life outcomes. Simulator authority. Do not certify treat→y from this table.",
            "first_falsifier": "treating y as measured IHDP IQ, or a real-world LATE",
            "replaces": "oregon_ohie",
        },
    )


def process_nhefs() -> None:
    """NHEFS qsmk observational table — must abstain."""
    path = REP / "rct/nhefs.csv"
    if not path.exists():
        write_receipt("nhefs_qsmk", {"status": "blocked"})
        return
    rows = []
    with path.open() as handle:
        for row in csv.DictReader(handle):
            qsmk = (row.get("qsmk") or "").strip()
            sex = (row.get("sex") or "").strip()
            death = (row.get("death") or "").strip()
            seqn = (row.get("seqn") or row.get("rownames") or "").strip()
            wt = _finite(row.get("wt82_71") or "")
            if qsmk not in {"0", "1"} or sex not in {"0", "1"} or not seqn:
                continue
            rows.append(
                [
                    f"n{seqn}",
                    "yes" if qsmk == "1" else "no",
                    "female" if sex == "1" else "male",
                    death if death in {"0", "1"} else "NA",
                    f"{wt:.4f}" if wt is not None else "NA",
                ]
            )
    table = TABLES / "nhefs_qsmk.csv"
    write_csv(table, ["cluster_id", "quit_smoke", "sex", "death", "wt_change"], rows)
    survey_and_receipt(
        "nhefs_qsmk",
        table,
        "cluster_id",
        {
            "source_url": "https://github.com/jlistman/nhefs (Hernán / Causal Inference What If teaching extract)",
            "license": "public NHEFS teaching extract",
            "cluster_unit": "person (seqn)",
            "ground_truth_authority": "observational smoking cessation; no assignment",
            "note": "Not an Oregon replacement for ITT/LATE. Teaching observational table. Complete quit×sex square would be a trap. Abstain required.",
            "first_falsifier": "qsmk→death inferred from a UNIQUE_PASS_PATTERN",
            "replaces": "oregon_ohie (observational contrast only)",
        },
    )


def process_iris_anmo() -> None:
    """IRIS IU.ANMO BHZ hour around 2010 Chile M8.8 — EarthScope hole."""
    path = REP / "seismic/anmo_chile.csv"
    if not path.exists() or path.stat().st_size < 1000:
        write_receipt("iris_anmo_chile", {"status": "blocked"})
        return
    origin = datetime(2010, 2, 27, 6, 34, 14, tzinfo=timezone.utc)
    parsed = []
    with path.open() as handle:
        for line in handle:
            if line.startswith("#") or line.lower().startswith("time"):
                continue
            parts = [p.strip() for p in line.split(",")]
            if len(parts) < 2:
                continue
            try:
                stamp = datetime.fromisoformat(parts[0].replace("Z", "+00:00"))
                sample = float(parts[1])
            except ValueError:
                continue
            parsed.append((stamp, sample))
    if len(parsed) < 100:
        write_receipt("iris_anmo_chile", {"status": "blocked", "note": "too few samples"})
        return
    abs_med = median([abs(s) for _, s in parsed])
    buckets: dict[str, list[float]] = defaultdict(list)
    flags: dict[str, str] = {}
    for stamp, sample in parsed:
        key = stamp.strftime("%Y%m%dT%H%M%S")
        buckets[key].append(sample)
        flags[key] = "event" if stamp >= origin else "pre"
    out = []
    for key, samples in buckets.items():
        rms = (sum(v * v for v in samples) / len(samples)) ** 0.5
        amp = "high" if rms >= abs_med else "low"
        out.append([key, flags[key], amp, f"{rms:.3f}", f"{len(samples)}"])
    table = TABLES / "iris_anmo_chile.csv"
    write_csv(table, ["cluster_id", "time_bin", "amp_bin", "rms", "n_samples"], out)
    survey_and_receipt(
        "iris_anmo_chile",
        table,
        "cluster_id",
        {
            "source_url": "https://service.iris.edu/fdsnws/dataselect/1/query?net=IU&sta=ANMO&loc=00&cha=BHZ&start=2010-02-27T06:30:00&end=2010-02-27T07:30:00&format=geocsv",
            "license": "IRIS DMC / FDSN (NSF SAGE); IU.ANMO public waveform",
            "cluster_unit": "UTC second at one station (not a shot gather, not a network)",
            "ground_truth_authority": "catalog origin time of 2010-02-27 Chile M8.8; receiver amplitude is not source location",
            "note": "Replacement for EarthScope PH5. Single-station hour. time_bin×amp_bin will look complete because the wavefield is the amplitude. Ancestry is not recoverable from one receiver. Do not invert source from this table.",
            "first_falsifier": "ANMO→Chile or treating rms bins as a discovered source-action",
            "replaces": "earthscope_ph5",
        },
    )


def process_reddyproc() -> None:
    """Tharandt 1998 half-hourly eddy covariance — FLUXNET hole."""
    path = REP / "flux/Example_DETha98.txt"
    if not path.exists() or path.stat().st_size < 1000:
        write_receipt("reddyproc_tha98", {"status": "blocked"})
        return
    text = path.read_text(errors="replace").replace("\r\n", "\n").replace("\r", "\n")
    lines = [line for line in text.splitlines() if line.strip()]
    if len(lines) < 10:
        write_receipt("reddyproc_tha98", {"status": "blocked", "note": "unparsed"})
        return
    parsed = []
    for line in lines[2:]:
        cols = line.split("\t") if "\t" in line else line.split()
        if len(cols) < 8:
            continue
        year, doy, hour = cols[0], cols[1], cols[2]
        nee, rg, tair = _finite(cols[3]), _finite(cols[6]), _finite(cols[7])
        if nee is None or rg is None or tair is None:
            continue
        parsed.append((year, doy, hour, nee, rg, tair))
    if len(parsed) < 50:
        write_receipt("reddyproc_tha98", {"status": "blocked", "note": f"too few valid hours ({len(parsed)})"})
        return
    rg_cut = median([p[4] for p in parsed])
    ta_cut = median([p[5] for p in parsed])
    out = []
    for year, doy, hour, nee, rg, tair in parsed:
        try:
            hour_key = f"{float(hour):04.1f}"
        except ValueError:
            hour_key = str(hour)
        cluster = f"{year}d{str(doy).zfill(3)}h{hour_key}"
        out.append(
            [
                cluster,
                token_hi_lo(rg, rg_cut),
                token_hi_lo(tair, ta_cut),
                f"{nee:.4f}",
                f"{rg:.3f}",
            ]
        )
    table = TABLES / "reddyproc_tha98.csv"
    write_csv(table, ["cluster_id", "rg_bin", "tair_bin", "nee", "rg"], out)
    survey_and_receipt(
        "reddyproc_tha98",
        table,
        "cluster_id",
        {
            "source_url": "https://raw.githubusercontent.com/bgctw/REddyProc/master/examples/Example_DETha98.txt",
            "license": "REddyProc example (Tharandt DE-Tha 1998); used as teaching eddy-covariance file",
            "cluster_unit": "half-hour at one site-year",
            "ground_truth_authority": "radiative/meteorological drivers are exogenous to NEE at a single tower on hourly scales; still a common diurnal driver",
            "note": "Replacement for FLUXNET2015 login. One site, 1998. rg×tair complete square is diurnal, not a GPP DAG. Not a multi-site FLUXNET extract.",
            "first_falsifier": "NEE→Rg or treating calendar night as a randomized shade treatment",
            "replaces": "fluxnet2015",
        },
    )


def process_hf004() -> None:
    """Harvard Forest EMS filled half-hours — second flux replacement."""
    path = REP / "flux/hf004-02-filled.csv"
    if not path.exists() or path.stat().st_size < 1000:
        write_receipt("hf004_ems", {"status": "blocked"})
        return
    parsed = []
    kept = 0
    with path.open() as handle:
        reader = csv.DictReader(handle)
        for index, row in enumerate(reader):
            nee = _finite(row.get("nee.e.6mol.m2.s") or "")
            par = _finite(row.get("par.28m.filled.e.6mol.m2.s") or row.get("par.28m.e.6mol.m2.s") or "")
            ta = _finite(row.get("ta.27m.filled.c") or row.get("obs_ta_.27m.c") or "")
            stamp = row.get("datetime") or ""
            if nee is None or par is None or ta is None or not stamp:
                continue
            # First nights are PAR=0; take every 20th valid hour so day/night both appear.
            if kept % 20 != 0:
                kept += 1
                continue
            kept += 1
            parsed.append((stamp, nee, par, ta))
            if len(parsed) >= 4000:
                break
    if len(parsed) < 50:
        write_receipt("hf004_ems", {"status": "blocked", "note": "too few valid filled hours"})
        return
    par_cut = median([p[2] for p in parsed])
    ta_cut = median([p[3] for p in parsed])
    out = [
        [stamp.replace(" ", "T"), token_hi_lo(par, par_cut), token_hi_lo(ta, ta_cut), f"{nee:.4f}", f"{par:.3f}"]
        for stamp, nee, par, ta in parsed
    ]
    table = TABLES / "hf004_ems.csv"
    write_csv(table, ["cluster_id", "par_bin", "tair_bin", "nee", "par"], out)
    survey_and_receipt(
        "hf004_ems",
        table,
        "cluster_id",
        {
            "source_url": "https://harvardforest.fas.harvard.edu/data/p00/hf004/hf004-02-filled.csv",
            "license": "Harvard Forest Data Archive HF004 (public LTER EMS tower)",
            "cluster_unit": "half-hour (first 4000 valid filled rows)",
            "ground_truth_authority": "PAR is the photon driver of GEE; temperature still confounds. Truncated slice.",
            "note": "Second FLUXNET replacement. Public, no AmeriFlux login. Diurnal common driver remains. Do not treat this as a multi-site FLUXNET2015 extract.",
            "first_falsifier": "NEE→PAR or using the filled product as if it were raw",
            "replaces": "fluxnet2015",
        },
    )


def process_uci70_glucose() -> None:
    """UCI 70-patient insulin/glucose diaries — UVA/Padova hole (not CGM physics)."""
    base = REP / "glucose/uci70/unpacked/Diabetes-Data"
    if not base.exists():
        write_receipt("uci70_glucose", {"status": "blocked", "note": "Diabetes-Data missing"})
        return
    # 33 regular insulin, 34 NPH, 35 UltraLente; 48/57–64 glucose
    insulin_codes = {"33", "34", "35"}
    glucose_codes = {str(c) for c in list(range(48, 65))}
    days: dict[tuple[str, str], dict[str, list[float] | bool]] = {}
    for path in sorted(base.glob("data-*")):
        patient = path.name.split("-")[-1]
        text = path.read_text(errors="replace")
        for line in text.splitlines():
            parts = line.strip().split()
            if len(parts) < 4:
                continue
            date_s, _time_s, code, value_s = parts[0], parts[1], parts[2], parts[3]
            rec = days.setdefault(
                (patient, date_s),
                {"insulin": False, "glucose": [], "meal_more": False},
            )
            val = _finite(value_s)
            if code in insulin_codes:
                rec["insulin"] = True
            if code in glucose_codes and val is not None:
                rec["glucose"].append(val)
            if code == "67":
                rec["meal_more"] = True
    out = []
    for (patient, date_s), rec in days.items():
        if not rec["glucose"]:
            continue
        mean_g = sum(rec["glucose"]) / len(rec["glucose"])
        out.append(
            [
                f"p{patient}_{date_s}",
                "yes" if rec["insulin"] else "no",
                "more" if rec["meal_more"] else "typical_or_none",
                f"{mean_g:.2f}",
                f"{len(rec['glucose'])}",
            ]
        )
    if len(out) < 20:
        write_receipt("uci70_glucose", {"status": "blocked", "note": f"too few patient-days ({len(out)})"})
        return
    table = TABLES / "uci70_glucose.csv"
    write_csv(table, ["cluster_id", "insulin_day", "meal_more", "mean_glucose", "n_glu"], out)
    survey_and_receipt(
        "uci70_glucose",
        table,
        "cluster_id",
        {
            "source_url": "https://archive.ics.uci.edu/static/public/34/diabetes.zip",
            "license": "UCI ML Repository Diabetes (70 patients, 1994 AIM symposium)",
            "cluster_unit": "patient-day",
            "ground_truth_authority": "self-recorded SMBG + insulin diary, not a pump/CGM simulator",
            "note": "Replacement for UVA/Padova and OhioT1DM. Not closed-loop physiology. insulin_day is exposure recorded that day, not a randomized dose. Abstain on insulin→glucose as a modular law.",
            "first_falsifier": "calling this UVA/Padova, or a unique insulin→glucose target",
            "replaces": "uva_padova_t1d / ohiot1dm",
        },
    )


def process_hospital_diabetes() -> None:
    """UCI 130-US hospitals — honest: encounter flags, not T1D physics."""
    path = REP / "glucose/unpacked/dataset_diabetes/diabetic_data.csv"
    if not path.exists():
        write_receipt("uci_hospital_diabetes", {"status": "blocked"})
        return
    rows = []
    with path.open() as handle:
        for index, row in enumerate(csv.DictReader(handle)):
            if index >= 8000:
                break
            insulin = (row.get("insulin") or "No").strip()
            change = (row.get("change") or "No").strip()
            readmit = (row.get("readmitted") or "NO").strip()
            enc = row.get("encounter_id") or str(index)
            a1c = row.get("A1Cresult") or "None"
            insulin_tok = "none" if insulin == "No" else "on"
            change_tok = "changed" if change == "Ch" else "unchanged"
            rows.append([enc, insulin_tok, change_tok, readmit, a1c])
    table = TABLES / "uci_hospital_diabetes.csv"
    write_csv(table, ["cluster_id", "insulin_on", "med_changed", "readmitted", "a1c"], rows)
    survey_and_receipt(
        "uci_hospital_diabetes",
        table,
        "cluster_id",
        {
            "source_url": "https://archive.ics.uci.edu/dataset/296/diabetes+130-us+hospitals+for+years+1999-2008",
            "license": "UCI ML Repository (Strack et al. 2014 hospital encounters)",
            "cluster_unit": "encounter (first 8000 rows); patients repeat",
            "ground_truth_authority": "hospital medication flags, not glucose dynamics",
            "note": "NOT a UVA/Padova replacement. Encounter-level insulin/readmission. Kept so the hospital extract is not silently relabeled as T1D physics.",
            "first_falsifier": "insulin_on→readmitted as a physiological law",
            "replaces": None,
        },
    )


def process_lincs_siginfo() -> None:
    """LINCS L1000 signature metadata — sci-Plex/Replogle hole (labels only)."""
    path = REP / "perturb/GSE92742_Broad_LINCS_sig_info.txt.gz"
    if not path.exists() or path.stat().st_size < 10_000:
        write_receipt(
            "lincs_siginfo",
            {
                "status": "blocked",
                "note": "GSE92742 sig_info missing or too small; expression matrices not fetched",
                "tried": [
                    "NCI GI50 wiki 403",
                    "CANDLE ALMANAC ftp 403",
                    "GDSC API 410",
                    "rcellminerData 404",
                    "GSE70138 MiniML 2.2KB (not a matrix)",
                ],
                "replaces": "sciplex_gse139944 / scperturb / replogle_2022_gwps",
            },
        )
        return
    rows = []
    with gzip.open(path, "rt", errors="replace") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        for index, row in enumerate(reader):
            if index % 20 != 0:
                continue
            if len(rows) >= 8000:
                break
            pert_type = (row.get("pert_type") or "unknown").strip()
            cell = (row.get("cell_id") or "unknown").strip()
            sig = row.get("sig_id") or f"s{index}"
            kind = "trt" if pert_type.startswith("trt") else "ctl_or_other"
            cell_tok = "mcf7" if cell.upper() == "MCF7" else "other_cell"
            rows.append([sig, kind, cell_tok, pert_type, cell])
    if len(rows) < 20:
        write_receipt("lincs_siginfo", {"status": "blocked", "note": "sig_info unparsed"})
        return
    table = TABLES / "lincs_siginfo.csv"
    write_csv(table, ["cluster_id", "trt_bin", "cell_bin", "pert_type", "cell"], rows)
    survey_and_receipt(
        "lincs_siginfo",
        table,
        "cluster_id",
        {
            "source_url": "https://ftp.ncbi.nlm.nih.gov/geo/series/GSE92nnn/GSE92742/suppl/GSE92742_Broad_LINCS_sig_info.txt.gz",
            "license": "GEO GSE92742 LINCS L1000 Phase I (Broad)",
            "cluster_unit": "signature (every 20th row, cap 8000)",
            "ground_truth_authority": "pert_type and cell_id labels only; no landmark expression in this slice",
            "note": "Replacement for sci-Plex/scPerturb/Replogle matrices. Metadata atlas. A trt×cell square is the screening catalog, not a gene DAG.",
            "first_falsifier": "a CRISPR/drug DAG from pert_type bits",
            "replaces": "sciplex / scperturb / replogle matrices",
        },
    )


def main() -> None:
    process_npk()
    process_gomez()
    process_langli_combo()
    process_star()
    process_ihdp()
    process_nhefs()
    process_iris_anmo()
    process_reddyproc()
    process_hf004()
    process_uci70_glucose()
    process_hospital_diabetes()
    process_lincs_siginfo()


if __name__ == "__main__":
    main()
