#!/usr/bin/env python3
from __future__ import annotations

import copy
import csv
import hashlib
import json
import math
import os
import re
import struct
import subprocess
import sys
import tomllib
from pathlib import Path

from jsonschema import Draft202012Validator
from lxml import html
from referencing import Registry, Resource

sys.path.insert(0, str(Path(__file__).resolve().parent))
# The checker and the generator must agree exactly on which paths belong in the manifest.
# Importing the one implementation makes disagreement impossible; two hand-synced copies
# drifted three separate times and each drift failed every clone but the author's.
from generate_repository_manifest import manifest_paths  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
ERRORS: list[str] = []
CHECKS = 0


def fail(message: str) -> None:
    ERRORS.append(message)


def check(condition: bool, message: str) -> None:
    global CHECKS
    CHECKS += 1
    if not condition:
        fail(message)


def load_json(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        fail(f"invalid JSON {path.relative_to(ROOT)}: {exc}")
        return {}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def unique_field(items: list[dict], field: str) -> bool:
    values = [item.get(field) for item in items]
    return len(values) == len(set(values))


def query_columns_resolve(query: dict, column_ids: set[object]) -> bool:
    exposure = query.get("exposure_column")
    outcome = query.get("outcome_column")
    return exposure in column_ids and outcome in column_ids and exposure != outcome


def route_is_allowed(
    assignment_kind: object,
    authorization: dict,
    allowed_routes: dict[tuple[object, object, object], set[str]],
) -> bool:
    route = (
        assignment_kind,
        authorization.get("strategy"),
        authorization.get("estimand"),
    )
    premise_names = {
        premise.get("name")
        for premise in authorization.get("required_premises", [])
        if isinstance(premise, dict)
    }
    return route in allowed_routes and premise_names == allowed_routes[route]


def authorization_matches_oracle(authorization: dict, oracle_route: dict) -> bool:
    return (
        authorization.get("strategy") == oracle_route.get("authorized_strategy")
        and authorization.get("estimand") == oracle_route.get("authorized_estimand")
    )


def authorization_evidence_resolves(authorization: dict, evidence_ids: set[object]) -> bool:
    return all(
        premise.get("evidence_ref") in evidence_ids
        for premise in authorization.get("required_premises", [])
        if isinstance(premise, dict)
    )


def authority_template_semantic_errors(
    routing_view: dict,
    authorized: dict,
    blind: dict,
    oracle: dict,
    benchmark_dir: Path,
) -> list[str]:
    """Validate cross-document authority semantics that JSON Schema cannot express."""
    errors: list[str] = []

    def require(condition: bool, message: str) -> None:
        if not condition:
            errors.append(message)

    source_table_path = benchmark_dir / "source_table.csv"
    routing_data_path = benchmark_dir / "routing_data.csv"
    confirmation_data_path = benchmark_dir / "confirmation_data.csv"
    diagnostic_path = benchmark_dir / "design_diagnostic_receipt.json"
    transformation_path = benchmark_dir / "transformation.json"
    discovery_path = benchmark_dir / "discovery_units.txt"
    confirmation_path = benchmark_dir / "confirmation_units.txt"
    required_paths = [
        source_table_path,
        routing_data_path,
        confirmation_data_path,
        diagnostic_path,
        transformation_path,
        discovery_path,
        confirmation_path,
    ]
    if not all(path.is_file() for path in required_paths):
        return ["authority-template content files are missing"]

    documents = [routing_view, authorized, blind, oracle]
    expected_hashes = {
        "source_table_sha256": sha256(source_table_path),
        "routing_data_sha256": sha256(routing_data_path),
        "confirmation_data_sha256": sha256(confirmation_data_path),
        "transformation_sha256": sha256(transformation_path),
        "discovery_unit_sha256": sha256(discovery_path),
        "confirmation_unit_sha256": sha256(confirmation_path),
    }
    for field in ["execution_status", "benchmark_id", "routing_view_id", *expected_hashes]:
        require(len({document.get(field) for document in documents}) == 1, f"binding drift: {field}")
    for document in documents:
        for field, expected in expected_hashes.items():
            require(document.get(field) == expected, f"stale content binding: {field}")

    neutral_columns = [
        item for item in routing_view.get("neutral_columns", []) if isinstance(item, dict)
    ]
    column_ids = [item.get("column_id") for item in neutral_columns]
    column_id_set = set(column_ids)
    require(unique_field(neutral_columns, "column_id"), "duplicate neutral column ID")
    with routing_data_path.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        rows = list(reader)
        table_columns = reader.fieldnames or []
    with source_table_path.open(newline="", encoding="utf-8") as handle:
        source_rows = list(csv.DictReader(handle))
    with confirmation_data_path.open(newline="", encoding="utf-8") as handle:
        confirmation_rows = list(csv.DictReader(handle))
    source_serialized = [json.dumps(row, sort_keys=True) for row in source_rows]
    partition_serialized = [
        json.dumps(row, sort_keys=True) for row in [*rows, *confirmation_rows]
    ]
    require(
        sorted(source_serialized) == sorted(partition_serialized),
        "discovery and confirmation tables do not reconstruct the source table",
    )
    require(column_ids == table_columns, "routing column order mismatch")
    transformation = load_json(transformation_path)
    transformation_columns = (
        transformation.get("ordered_columns", []) if isinstance(transformation, dict) else []
    )
    require(transformation_columns == table_columns, "transformation column order mismatch")

    queries = [item for item in routing_view.get("queries", []) if isinstance(item, dict)]
    require(unique_field(queries, "query_id"), "duplicate routing query ID")
    for query in queries:
        require(query_columns_resolve(query, column_id_set), "invalid query column reference")
    queries_by_id = {item.get("query_id"): item for item in queries}

    authorization_items = [
        item
        for item in authorized.get("estimand_authorizations", [])
        if isinstance(item, dict)
    ]
    oracle_items = [
        item for item in oracle.get("expected_routes", []) if isinstance(item, dict)
    ]
    require(unique_field(authorization_items, "query_id"), "duplicate authorization query ID")
    require(unique_field(oracle_items, "query_id"), "duplicate oracle query ID")
    authorizations = {item.get("query_id"): item for item in authorization_items}
    oracle_routes = {item.get("query_id"): item for item in oracle_items}
    require(
        set(queries_by_id) == set(authorizations) == set(oracle_routes),
        "query sets disagree",
    )

    assignment = authorized.get("assignment", {})
    if not isinstance(assignment, dict):
        assignment = {}
    assignment_column = assignment.get("assignment_column")
    assignment_unit = assignment.get("assignment_unit_column")
    require(assignment_column in column_id_set, "unknown assignment column")
    require(assignment_unit in column_id_set, "unknown assignment unit")
    unit_columns = {
        item.get("column_id")
        for item in neutral_columns
        if "unit" in item.get("candidate_roles", [])
    }
    require(assignment_unit in unit_columns, "assignment unit lacks unit role")
    if assignment.get("kind") == "randomized_encouragement":
        require(
            assignment.get("probability_contract")
            in {"externally_documented_constant", "externally_documented_by_stratum"},
            "randomized encouragement lacks a probability contract",
        )
        require(
            assignment.get("timing_contract") == "pre_exposure_and_outcome",
            "randomized encouragement has an invalid timing contract",
        )

    discovery_units = {
        line.strip()
        for line in discovery_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    }
    confirmation_units = {
        line.strip()
        for line in confirmation_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    }
    source_units = {row.get(str(assignment_unit)) for row in source_rows}
    routing_units = {row.get(str(assignment_unit)) for row in rows}
    sealed_confirmation_units = {
        row.get(str(assignment_unit)) for row in confirmation_rows
    }
    require(not (discovery_units & confirmation_units), "unit partitions overlap")
    require(discovery_units | confirmation_units == source_units, "unit partitions do not cover source")
    require(routing_units == discovery_units, "routing data is not discovery-only")
    require(
        sealed_confirmation_units == confirmation_units,
        "confirmation data does not match its sealed partition",
    )

    diagnostic = load_json(diagnostic_path)
    if not isinstance(diagnostic, dict):
        diagnostic = {}
    require(
        diagnostic.get("routing_data_sha256") == sha256(routing_data_path),
        "diagnostic is not bound to routing data",
    )
    require(
        diagnostic.get("discovery_unit_sha256") == sha256(discovery_path),
        "diagnostic is not bound to discovery units",
    )
    require(diagnostic.get("unit_column") == assignment_unit, "diagnostic unit disagrees")

    unit_values: dict[str, tuple[float, float]] = {}
    for unit in sorted(discovery_units):
        unit_rows = [row for row in rows if row.get(str(assignment_unit)) == unit]
        require(bool(unit_rows), f"diagnostic unit {unit} has no rows")
        if not unit_rows:
            continue
        assignments = {float(row.get(str(assignment_column), "nan")) for row in unit_rows}
        exposure_column = next(
            (
                item.get("exposure_column")
                for item in authorization_items
                if item.get("strategy") == "instrumental_variable"
            ),
            None,
        )
        exposures = [float(row.get(str(exposure_column), "nan")) for row in unit_rows]
        require(len(assignments) == 1 and assignments <= {0.0, 1.0}, "assignment varies within unit")
        require(all(value in {0.0, 1.0} for value in exposures), "exposure is not binary")
        if len(assignments) == 1 and exposures:
            unit_values[unit] = (next(iter(assignments)), sum(exposures) / len(exposures))
    zero_values = [exposure for assignment_value, exposure in unit_values.values() if assignment_value == 0]
    one_values = [exposure for assignment_value, exposure in unit_values.values() if assignment_value == 1]
    require(bool(zero_values) and bool(one_values), "diagnostic lacks both assignment arms")
    if zero_values and one_values:
        zero_mean = sum(zero_values) / len(zero_values)
        one_mean = sum(one_values) / len(one_values)
        first_stage = abs(one_mean - zero_mean)
        relevance = diagnostic.get("relevance", {})
        positivity = diagnostic.get("positivity", {})
        if not isinstance(relevance, dict):
            relevance = {}
        if not isinstance(positivity, dict):
            positivity = {}
        require(relevance.get("assignment_column") == assignment_column, "relevance assignment differs")
        require(relevance.get("exposure_column") == exposure_column, "relevance exposure differs")
        require(relevance.get("assignment_zero_units") == len(zero_values), "zero-unit count differs")
        require(relevance.get("assignment_one_units") == len(one_values), "one-unit count differs")
        require(math.isclose(relevance.get("mean_exposure_given_zero", math.nan), zero_mean), "zero mean differs")
        require(math.isclose(relevance.get("mean_exposure_given_one", math.nan), one_mean), "one mean differs")
        require(math.isclose(relevance.get("absolute_first_stage", math.nan), first_stage), "first stage differs")
        relevance_floor = relevance.get("minimum_absolute_first_stage", math.inf)
        require(first_stage >= relevance_floor, "first stage is below its frozen floor")
        total_units = len(zero_values) + len(one_values)
        zero_fraction = len(zero_values) / total_units
        one_fraction = len(one_values) / total_units
        positivity_floor = positivity.get("minimum_arm_fraction", math.inf)
        require(
            math.isclose(positivity.get("assignment_zero_fraction", math.nan), zero_fraction),
            "zero-arm fraction differs",
        )
        require(
            math.isclose(positivity.get("assignment_one_fraction", math.nan), one_fraction),
            "one-arm fraction differs",
        )
        require(min(zero_fraction, one_fraction) >= positivity_floor, "positivity floor is not met")

    evidence_items = [
        item for item in authorized.get("premise_evidence", []) if isinstance(item, dict)
    ]
    require(unique_field(evidence_items, "evidence_id"), "duplicate evidence ID")
    evidence_by_id = {item.get("evidence_id"): item for item in evidence_items}
    benchmark_root = benchmark_dir.resolve()
    for evidence in evidence_items:
        evidence_path = (benchmark_dir / str(evidence.get("relative_path"))).resolve()
        try:
            evidence_path.relative_to(benchmark_root)
            inside = True
        except ValueError:
            inside = False
        require(inside, "evidence path escapes fixture")
        require(evidence_path.is_file(), "evidence path is missing")
        if evidence_path.is_file():
            require(evidence.get("content_sha256") == sha256(evidence_path), "stale evidence hash")
    diagnostic_evidence = [
        item
        for item in evidence_items
        if set(item.get("covers_premises", [])) & {"relevance", "positivity"}
    ]
    require(
        len(diagnostic_evidence) == 1
        and diagnostic_evidence[0].get("relative_path")
        == "design_diagnostic_receipt.json"
        and diagnostic_evidence[0].get("content_sha256") == sha256(diagnostic_path)
        and diagnostic_evidence[0].get("evidence_class")
        == "design_diagnostic_receipt",
        "relevance and positivity are not bound to the design diagnostic receipt",
    )

    route_contracts: dict[tuple[object, object, object], dict[str, str]] = {
        ("randomized_encouragement", "recorded_randomization", "offer_itt"): {
            "assignment_integrity": "externally_asserted_not_empirically_proved",
            "correct_unit": "externally_asserted_not_empirically_proved",
            "consistency": "externally_asserted_not_empirically_proved",
            "positivity": "empirically_checked_not_design_authority",
        },
        ("randomized_encouragement", "instrumental_variable", "complier_late"): {
            "relevance": "empirically_checked_not_design_authority",
            "exclusion": "externally_asserted_not_empirically_proved",
            "independence": "externally_asserted_not_empirically_proved",
            "monotonicity": "externally_asserted_not_empirically_proved",
            "correct_unit": "externally_asserted_not_empirically_proved",
            "consistency": "externally_asserted_not_empirically_proved",
            "positivity": "empirically_checked_not_design_authority",
        },
    }
    for query_id, authorization in authorizations.items():
        premises = [
            item
            for item in authorization.get("required_premises", [])
            if isinstance(item, dict)
        ]
        require(unique_field(premises, "name"), "duplicate premise name")
        route_key = (
            assignment.get("kind"),
            authorization.get("strategy"),
            authorization.get("estimand"),
        )
        contract = route_contracts.get(route_key)
        require(contract is not None, "invalid assignment/strategy/estimand route")
        premise_statuses = {item.get("name"): item.get("status") for item in premises}
        if contract is not None:
            require(premise_statuses == contract, "premise names or authority statuses disagree")
        require(
            authorization_evidence_resolves(authorization, set(evidence_by_id)),
            "unresolved premise evidence",
        )
        for premise in premises:
            evidence = evidence_by_id.get(premise.get("evidence_ref"), {})
            expected_class = (
                "design_diagnostic_receipt"
                if premise.get("status") == "empirically_checked_not_design_authority"
                else "external_design_assertion"
            )
            require(evidence.get("evidence_class") == expected_class, "wrong evidence class")
            require(
                premise.get("name") in evidence.get("covers_premises", []),
                "evidence does not cover referenced premise",
            )
        query = queries_by_id.get(query_id, {})
        exposure = authorization.get("exposure_column")
        assignment_reference = authorization.get("assignment_reference_column")
        require(exposure == query.get("exposure_column"), "authorization exposure disagrees with query")
        require(assignment_reference == assignment_column, "authorization loses assignment reference")
        if authorization.get("strategy") == "recorded_randomization":
            require(exposure == assignment_column, "ITT exposure is not the randomized assignment")
        if authorization.get("strategy") == "instrumental_variable":
            require(exposure != assignment_column, "IV exposure incorrectly equals its instrument")
        require(
            authorization_matches_oracle(authorization, oracle_routes.get(query_id, {})),
            "authorization disagrees with oracle",
        )

    authority_hash = assignment.get("authority_source_sha256")
    assignment_evidence = [
        item
        for item in evidence_items
        if "assignment_integrity" in item.get("covers_premises", [])
    ]
    require(
        len(assignment_evidence) == 1
        and assignment_evidence[0].get("content_sha256") == authority_hash,
        "assignment authority source is not the assignment-integrity evidence",
    )
    require(authority_hash == oracle.get("source_document_sha256"), "oracle source disagrees")
    require(authorized.get("use_scope") == oracle.get("use_scope"), "use scope disagrees")
    require(authorized.get("status") == "illustrative_supplied", "authorized status is wrong")
    require(blind.get("status") == "withheld", "blind status is wrong")
    require(
        not {"assignment", "estimand_authorizations", "premise_evidence", "use_scope"}.intersection(blind),
        "blind receipt leaks authority fields",
    )
    return errors


def active_tilt_candidate_fingerprint(candidates: list[object]) -> str:
    """Mirror mic-proposal's exact, order-sensitive candidate-library framing."""
    digest = hashlib.sha256()
    digest.update(b"mic-active-tilt-candidates-v1\0")

    def add_length(value: int) -> None:
        digest.update(value.to_bytes(8, "big"))

    def add_string(value: object) -> None:
        encoded = str(value).encode("utf-8")
        add_length(len(encoded))
        digest.update(encoded)

    add_length(len(candidates))
    for item in candidates:
        if not isinstance(item, dict):
            continue
        add_string(item.get("candidate_id", ""))
        add_string(item.get("primitive_id", ""))
        digest.update(bytes([bool(item.get("measurable_delivery"))]))
        digest.update(bytes([bool(item.get("common_support"))]))
        eligibility = item.get("design_eligibility", {})
        status = eligibility.get("status") if isinstance(eligibility, dict) else None
        if status == "not_required_for_four_law":
            digest.update(b"\x00")
        elif status == "product_odds_verified":
            digest.update(b"\x01")
            add_string(eligibility.get("audit_id", ""))
        elif status == "reweighted_to_product":
            digest.update(b"\x02")
            add_string(eligibility.get("plan_id", ""))
        else:
            digest.update(b"\x03")
        predictions = item.get("predicted_pairwise_separations", [])
        add_length(len(predictions))
        for prediction in predictions:
            add_string(prediction.get("first", ""))
            add_string(prediction.get("second", ""))
            digest.update(struct.pack(">d", float(prediction.get("separation", math.nan))))
        cost = item.get("cost")
        if cost is None:
            digest.update(b"\x00")
        else:
            digest.update(b"\x01")
            digest.update(struct.pack(">d", float(cost)))
    return f"sha256:{digest.hexdigest()}"


def scout_json_fingerprint(domain: bytes, value: object) -> str:
    """Mirror the scout's domain-separated, length-framed Serde JSON fingerprint."""
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    digest = hashlib.sha256()
    digest.update(domain)
    digest.update(len(encoded).to_bytes(8, "big"))
    digest.update(encoded)
    return digest.hexdigest()


def scout_bundle_semantic_errors(request: dict, draft: dict, proposal: dict) -> list[str]:
    """Cross-check the frozen scout artifact against the two exact inputs."""
    errors: list[str] = []

    def require(condition: bool, message: str) -> None:
        if not condition:
            errors.append(message)

    def sorted_unique_strings(value: object) -> bool:
        return (
            isinstance(value, list)
            and bool(value)
            and all(isinstance(item, str) and item.strip() for item in value)
            and value == sorted(set(value))
        )

    partition = request.get("partition_claim", {})
    if isinstance(partition, dict):
        require(
            partition.get("discovery_units", 0) + partition.get("confirmation_units", 0)
            == partition.get("total_units", -1),
            "scout unit counts do not sum to the declared total",
        )
    require(
        sorted_unique_strings(request.get("learner_families")),
        "scout learner families are not canonical",
    )

    environments = draft.get("environments", [])
    environment_ids = [
        item.get("environment_id") for item in environments if isinstance(item, dict)
    ]
    require(
        len(environment_ids) == len(environments) == len(set(environment_ids)),
        "scout environment identifiers are incomplete or duplicated",
    )
    require(environment_ids == sorted(environment_ids), "scout environments are not canonical")
    for environment in environments:
        if isinstance(environment, dict):
            require(
                sorted_unique_strings(environment.get("defining_columns")),
                f"scout environment {environment.get('environment_id')} columns are not canonical",
            )

    supports = draft.get("supports", [])
    support_items = [item for item in supports if isinstance(item, dict)]
    support_ids = [item.get("support_id") for item in support_items]
    require(
        len(support_ids) == len(supports) == len(set(support_ids)),
        "scout support identifiers are incomplete or duplicated",
    )
    require(support_ids == sorted(support_ids), "scout supports are not canonical")
    require(
        len(environments) + len(supports) <= request.get("candidate_budget", -1),
        "scout exceeds the frozen candidate budget",
    )
    support_by_id = {item.get("support_id"): item for item in support_items}
    for support in support_items:
        require(
            support.get("environment_id") in environment_ids,
            f"scout support {support.get('support_id')} has an unknown environment",
        )
        require(
            sorted_unique_strings(support.get("variables")),
            f"scout support {support.get('support_id')} variables are not canonical",
        )
        require(
            support.get("learner_family") in request.get("learner_families", []),
            f"scout support {support.get('support_id')} is outside the learner battery",
        )

    def relation(left: list[str], right: list[str]) -> str:
        left_set, right_set = set(left), set(right)
        if left_set == right_set:
            return "equal"
        if left_set < right_set:
            return "left_proper_subset"
        if right_set < left_set:
            return "right_proper_subset"
        if left_set.isdisjoint(right_set):
            return "disjoint"
        return "overlap"

    relation_pairs: list[tuple[object, object]] = []
    for item in draft.get("support_relations", []):
        if not isinstance(item, dict):
            continue
        pair = (item.get("left_support_id"), item.get("right_support_id"))
        relation_pairs.append(pair)
        left = support_by_id.get(pair[0])
        right = support_by_id.get(pair[1])
        require(left is not None and right is not None, "scout relation has an unknown support")
        if left is None or right is None:
            continue
        require(pair[0] != pair[1], "scout relation compares a support to itself")
        require(
            left.get("semantics") == right.get("semantics") == item.get("semantics"),
            "scout relation crosses support semantics",
        )
        require(
            item.get("relation")
            == relation(left.get("variables", []), right.get("variables", [])),
            "scout relation does not match the frozen variable sets",
        )
    require(
        len(relation_pairs) == len(set(relation_pairs)),
        "scout repeats an ordered support relation",
    )
    require(relation_pairs == sorted(relation_pairs), "scout support relations are not canonical")

    for field, identifier in [
        ("contract_requests", "request_id"),
        ("next_queries", "query_id"),
    ]:
        items = draft.get(field, [])
        identifiers = [item.get(identifier) for item in items if isinstance(item, dict)]
        require(
            len(identifiers) == len(items) == len(set(identifiers)),
            f"scout {field} identifiers are incomplete or duplicated",
        )
        require(identifiers == sorted(identifiers), f"scout {field} are not canonical")
    request_items = [
        item for item in draft.get("contract_requests", []) if isinstance(item, dict)
    ]
    request_ids = {item.get("request_id") for item in request_items}
    strategy_items = draft.get("strategy_eligibility", {})
    strategy_ids = set(strategy_items) if isinstance(strategy_items, dict) else set()
    for item in request_items:
        require(
            item.get("required_for") in strategy_ids,
            f"scout contract {item.get('request_id')} references an unknown strategy",
        )
    if isinstance(strategy_items, dict):
        for strategy_id, eligibility in strategy_items.items():
            if isinstance(eligibility, dict) and eligibility.get("status") == "missing_contract":
                require(
                    eligibility.get("contract_request_ref") in request_ids,
                    f"scout strategy {strategy_id} references an unknown contract request",
                )
    for query in draft.get("next_queries", []):
        if isinstance(query, dict):
            require(
                sorted_unique_strings(query.get("separates_hypotheses")),
                f"scout query {query.get('query_id')} hypotheses are not canonical",
            )
            require(
                sorted_unique_strings(query.get("contract_request_ids")),
                f"scout query {query.get('query_id')} contract requests are not canonical",
            )
            require(
                set(query.get("contract_request_ids", [])).issubset(request_ids),
                f"scout query {query.get('query_id')} has an unknown contract request",
            )
    reasons = proposal.get("reasons", [])
    reason_order = [
        "no_environment_candidate",
        "unit_unverified",
        "selection_unestablished",
        "learner_disagreement",
        "support_failure",
        "composition_unavailable",
        "same_target_premise_unestablished",
        "confirmation_sealed",
    ]
    reason_rank = {reason: index for index, reason in enumerate(reason_order)}
    require(
        len(reasons) == len(set(reasons))
        and all(reason in reason_rank for reason in reasons)
        and reasons == sorted(reasons, key=reason_rank.get),
        "scout reason codes are not canonical",
    )
    expected_reasons = {"selection_unestablished", "confirmation_sealed"}
    unit_declaration = request.get("unit_declaration", {})
    if isinstance(unit_declaration, dict) and unit_declaration.get("basis") in {
        "unverified_identifier",
        "row",
    }:
        expected_reasons.add("unit_unverified")
    if not environments:
        expected_reasons.add("no_environment_candidate")
    if any(item.get("kind") == "same_target_grouping" for item in request_items):
        expected_reasons.add("same_target_premise_unestablished")
    require(
        set(reasons) == expected_reasons,
        "scout reason codes are not derived from the typed request and draft",
    )

    next_queries = draft.get("next_queries", [])
    expected_status = (
        "abstained"
        if not environments
        else "inconclusive"
        if not next_queries
        else "recommended"
    )
    require(proposal.get("status") == expected_status, "scout status is not derived from the draft")
    require(proposal.get("authority") == "proposal_only", "scout grants causal authority")
    require(proposal.get("certificate_eligible") is False, "scout is certificate eligible")
    require(
        proposal.get("input_claims_verified") is False,
        "scout falsely verifies caller-supplied partition, unit, or isolation claims",
    )
    require(proposal.get("request_id") == request.get("request_id"), "scout request ID drifted")
    require(proposal.get("proposal_id") == draft.get("proposal_id"), "scout proposal ID drifted")
    require(proposal.get("seed") == request.get("seed"), "scout seed drifted")
    require(
        proposal.get("request_fingerprint")
        == scout_json_fingerprint(b"mic-self-driving-request-v1\0", request),
        "scout request fingerprint drifted",
    )
    require(
        proposal.get("candidate_library_fingerprint")
        == scout_json_fingerprint(b"mic-shift-factorization-library-v1\0", draft),
        "scout candidate-library fingerprint drifted",
    )
    for field in [
        "environments",
        "supports",
        "support_relations",
        "strategy_eligibility",
        "contract_requests",
        "next_queries",
    ]:
        require(proposal.get(field) == draft.get(field), f"scout output drifted from draft field {field}")
    return errors


def required_files() -> None:
    paths = [
        "README.md",
        "REPOSITORY_MANIFEST.json",
        "Cargo.toml",
        "pyproject.toml",
        "paper/main.tex",
        "paper/main.pdf",
        "paper/references.bib",
        "site/index.html",
        "site/styles.css",
        "site/app.js",
        "site/mechanism_interferometry.pdf",
        "schemas/experiment_manifest.schema.json",
        "schemas/evidence_finding.schema.json",
        "schemas/audit_report.schema.json",
        "schemas/active_tilt_input.schema.json",
        "schemas/orientation_input.schema.json",
        "schemas/closure_crossfit_request.schema.json",
        "schemas/proposal_batch.schema.json",
        "schemas/self_driving_request.schema.json",
        "schemas/shift_factorization_draft.schema.json",
        "schemas/shift_factorization_proposal.schema.json",
        "schemas/benchmark_routing_view.schema.json",
        "schemas/design_authority_receipt.schema.json",
        "schemas/benchmark_oracle.schema.json",
        "schemas/design_diagnostic_receipt.schema.json",
        "schemas/scalar_response_contract.schema.json",
        "examples/orientation/parity_demo.json",
        "examples/closure_crossfit_request.json",
        "examples/proposal_inputs/parity_active_tilt.json",
        "examples/proposals/parity_active_tilt.json",
        "examples/scout_inputs/self_driving_request.json",
        "examples/scout_inputs/shift_factorization_draft.json",
        "examples/scout_proposals/shift_factorization.json",
        "examples/benchmarks/authority_ablation_template/README.md",
        "examples/benchmarks/authority_ablation_template/source_table.csv",
        "examples/benchmarks/authority_ablation_template/routing_data.csv",
        "examples/benchmarks/authority_ablation_template/confirmation_data.csv",
        "examples/benchmarks/authority_ablation_template/design_diagnostic_receipt.json",
        "examples/benchmarks/authority_ablation_template/transformation.json",
        "examples/benchmarks/authority_ablation_template/discovery_units.txt",
        "examples/benchmarks/authority_ablation_template/confirmation_units.txt",
        "examples/benchmarks/authority_ablation_template/design_source_excerpt.txt",
        "examples/benchmarks/authority_ablation_template/semantic_contract.txt",
        "examples/benchmarks/authority_ablation_template/routing_view.json",
        "examples/benchmarks/authority_ablation_template/authorized_design_receipt.json",
        "examples/benchmarks/authority_ablation_template/blind_design_receipt.json",
        "examples/benchmarks/authority_ablation_template/oracle.json",
        "examples/datasets/nci_almanac/README.md",
        "examples/datasets/nci_almanac/scalar_response_contract.json",
        "crates/mic-proposal/Cargo.toml",
        "crates/mic-proposal/src/lib.rs",
        "scripts/generate_simulations.py",
        "scripts/generate_example_data.py",
        "scripts/generate_repository_manifest.py",
        "scripts/package_release.sh",
        "scripts/build_all.sh",
        "docs/FORMAL_SPEC.md",
        "docs/IMPLEMENTATION_BLUEPRINT.md",
        "docs/INFERENCE_PROTOCOL.md",
        "docs/PROPOSAL_ADAPTERS.md",
        "docs/FRANKEN_INTEGRATION.md",
    ]
    for relative in paths:
        check((ROOT / relative).is_file(), f"missing required file {relative}")


def validate_schemas_and_manifests() -> None:
    schema_dir = ROOT / "schemas"
    schemas: dict[str, dict] = {}
    for path in sorted(schema_dir.glob("*.schema.json")):
        document = load_json(path)
        if not isinstance(document, dict):
            continue
        try:
            Draft202012Validator.check_schema(document)
        except Exception as exc:
            fail(f"invalid JSON Schema {path.relative_to(ROOT)}: {exc}")
        schemas[path.name] = document

    manifest_schema = schemas.get("experiment_manifest.schema.json")
    orientation_schema = schemas.get("orientation_input.schema.json")
    closure_crossfit_schema = schemas.get("closure_crossfit_request.schema.json")
    proposal_input_schema = schemas.get("active_tilt_input.schema.json")
    proposal_schema = schemas.get("proposal_batch.schema.json")
    scout_request_schema = schemas.get("self_driving_request.schema.json")
    scout_draft_schema = schemas.get("shift_factorization_draft.schema.json")
    scout_proposal_schema = schemas.get("shift_factorization_proposal.schema.json")
    audit_report_schema = schemas.get("audit_report.schema.json")
    four_law_report_schema = schemas.get("four_law_report.schema.json")
    finding_schema = schemas.get("evidence_finding.schema.json")
    routing_view_schema = schemas.get("benchmark_routing_view.schema.json")
    design_receipt_schema = schemas.get("design_authority_receipt.schema.json")
    benchmark_oracle_schema = schemas.get("benchmark_oracle.schema.json")
    design_diagnostic_schema = schemas.get("design_diagnostic_receipt.schema.json")
    scalar_response_schema = schemas.get("scalar_response_contract.schema.json")

    check(closure_crossfit_schema is not None, "closure-crossfit schema was not loaded")
    if closure_crossfit_schema is not None:
        closure_request = load_json(ROOT / "examples" / "closure_crossfit_request.json")
        closure_validator = Draft202012Validator(closure_crossfit_schema)
        errors = sorted(
            closure_validator.iter_errors(closure_request), key=lambda item: list(item.path)
        )
        for error in errors:
            location = ".".join(str(part) for part in error.path) or "<root>"
            fail(f"closure-crossfit request schema violation at {location}: {error.message}")
        if isinstance(closure_request, dict):
            proportions = closure_request.get("sampling_proportions", [])
            check(
                len(proportions) == 4
                and all(isinstance(value, (int, float)) and math.isfinite(value) for value in proportions)
                and math.isclose(sum(proportions), 1.0, abs_tol=1e-10),
                "closure-crossfit sampling proportions must form a finite simplex",
            )
            samples = closure_request.get("samples", [])
            feature_widths = {
                len(sample.get("features", []))
                for sample in samples
                if isinstance(sample, dict)
            }
            check(len(feature_widths) == 1, "closure-crossfit feature dimensions differ")
            check(
                all(
                    math.isfinite(value)
                    for sample in samples
                    if isinstance(sample, dict)
                    for value in sample.get("features", [])
                ),
                "closure-crossfit features must be finite",
            )
            unknown = copy.deepcopy(closure_request)
            unknown["certificate_eligible"] = True
            check(
                bool(list(closure_validator.iter_errors(unknown))),
                "closure-crossfit schema accepts an authority-bearing unknown field",
            )

    check(scalar_response_schema is not None, "scalar-response contract schema was not loaded")
    if scalar_response_schema is not None:
        scalar_contract = load_json(
            ROOT / "examples" / "datasets" / "nci_almanac" / "scalar_response_contract.json"
        )
        scalar_errors = sorted(
            Draft202012Validator(scalar_response_schema).iter_errors(scalar_contract),
            key=lambda item: list(item.path),
        )
        for error in scalar_errors:
            location = ".".join(str(part) for part in error.path) or "<root>"
            fail(f"scalar-response contract schema violation at {location}: {error.message}")
        if isinstance(scalar_contract, dict):
            check(
                scalar_contract.get("authority") == "proposal_only"
                and scalar_contract.get("certificate_eligible") is False,
                "scalar-response contract gained causal certificate authority",
            )

    check(scout_request_schema is not None, "self-driving request schema was not loaded")
    check(scout_draft_schema is not None, "shift-factorization draft schema was not loaded")
    check(scout_proposal_schema is not None, "shift-factorization proposal schema was not loaded")
    if (
        scout_request_schema is not None
        and scout_draft_schema is not None
        and scout_proposal_schema is not None
    ):
        scout_registry = Registry().with_resource(
            str(scout_proposal_schema["$id"]),
            Resource.from_contents(scout_proposal_schema),
        )
        request_validator = Draft202012Validator(scout_request_schema)
        draft_validator = Draft202012Validator(scout_draft_schema, registry=scout_registry)
        scout_validator = Draft202012Validator(scout_proposal_schema)
        request_path = ROOT / "examples" / "scout_inputs" / "self_driving_request.json"
        draft_path = ROOT / "examples" / "scout_inputs" / "shift_factorization_draft.json"
        proposal_path = ROOT / "examples" / "scout_proposals" / "shift_factorization.json"
        request = load_json(request_path)
        draft = load_json(draft_path)
        proposal = load_json(proposal_path)
        for name, validator, value in [
            ("self-driving request", request_validator, request),
            ("shift-factorization draft", draft_validator, draft),
            ("shift-factorization proposal", scout_validator, proposal),
        ]:
            for error in sorted(validator.iter_errors(value), key=lambda error: list(error.path)):
                location = ".".join(str(part) for part in error.path) or "<root>"
                fail(f"{name} schema violation at {location}: {error.message}")
        if isinstance(request, dict) and isinstance(draft, dict) and isinstance(proposal, dict):
            for error in scout_bundle_semantic_errors(request, draft, proposal):
                fail(error)

            def scout_bundle_rejected(
                candidate_request: dict, candidate_draft: dict, candidate_proposal: dict
            ) -> bool:
                schema_errors = [
                    *request_validator.iter_errors(candidate_request),
                    *draft_validator.iter_errors(candidate_draft),
                    *scout_validator.iter_errors(candidate_proposal),
                ]
                semantic_errors = scout_bundle_semantic_errors(
                    candidate_request, candidate_draft, candidate_proposal
                )
                return bool(schema_errors or semantic_errors)

            mutations: list[tuple[str, dict, dict, dict]] = []
            unsorted_learners = copy.deepcopy(request)
            unsorted_learners["learner_families"] = list(
                reversed(unsorted_learners["learner_families"])
            )
            mutations.append(("unsorted learner battery", unsorted_learners, draft, proposal))
            bad_partition = copy.deepcopy(request)
            bad_partition["partition_claim"]["confirmation_units"] -= 1
            mutations.append(("nonexhaustive unit counts", bad_partition, draft, proposal))
            confirmation_leak = copy.deepcopy(request)
            confirmation_leak["confirmation_table_sha256"] = "0" * 64
            mutations.append(("confirmation commitment leak", confirmation_leak, draft, proposal))
            unknown_environment = copy.deepcopy(draft)
            unknown_environment["supports"][0]["environment_id"] = "env_999"
            mutations.append(("unknown support environment", request, unknown_environment, proposal))
            crossed_semantics = copy.deepcopy(draft)
            crossed_semantics["supports"][1]["semantics"] = "marginal_shift_set"
            mutations.append(("cross-semantic support relation", request, crossed_semantics, proposal))
            false_relation = copy.deepcopy(draft)
            false_relation["support_relations"][0]["relation"] = "equal"
            mutations.append(("false support relation", request, false_relation, proposal))
            repeated_support = copy.deepcopy(draft)
            repeated_support["supports"][1]["support_id"] = repeated_support["supports"][0]["support_id"]
            mutations.append(("duplicate support ID", request, repeated_support, proposal))
            permuted_supports = copy.deepcopy(draft)
            permuted_supports["supports"].reverse()
            mutations.append(("permuted support library", request, permuted_supports, proposal))
            outside_battery = copy.deepcopy(draft)
            outside_battery["supports"][0]["learner_family"] = "neural"
            mutations.append(("learner outside battery", request, outside_battery, proposal))
            unknown_contract = copy.deepcopy(draft)
            unknown_contract["next_queries"][0]["contract_request_ids"] = ["contract_999"]
            mutations.append(("unknown query contract", request, unknown_contract, proposal))
            stale_fingerprint = copy.deepcopy(proposal)
            stale_fingerprint["candidate_library_fingerprint"] = "0" * 64
            mutations.append(("stale library fingerprint", request, draft, stale_fingerprint))
            forged_authority = copy.deepcopy(proposal)
            forged_authority["authority"] = "certificate"
            mutations.append(("forged authority", request, draft, forged_authority))
            forged_claim_verification = copy.deepcopy(proposal)
            forged_claim_verification["input_claims_verified"] = True
            mutations.append(("forged claim verification", request, draft, forged_claim_verification))
            for name, candidate_request, candidate_draft, candidate_proposal in mutations:
                check(
                    scout_bundle_rejected(
                        candidate_request, candidate_draft, candidate_proposal
                    ),
                    f"scout validator accepted adversary: {name}",
                )

    check(audit_report_schema is not None, "audit report schema was not loaded")
    check(four_law_report_schema is not None, "four-law report schema was not loaded")
    check(finding_schema is not None, "evidence finding schema was not loaded")
    if audit_report_schema is not None and finding_schema is not None:
        registry = Registry().with_resource(
            str(finding_schema["$id"]), Resource.from_contents(finding_schema)
        )
        audit_validator = Draft202012Validator(audit_report_schema, registry=registry)
        established_gates = {
            "locality": "established",
            "conditional_normalization": "established",
            "square_flatness": "established",
            "orientation": "established",
        }
        base_report = {
            "schema_version": "2.0.0",
            "run_id": "schema-conformance",
            "experiment_id": "schema-conformance",
            "mode": "strict",
            "status": "passed",
            "gates": established_gates,
            "manifest_sha256": "0" * 64,
            "dependency_revisions": {
                "frankenpandas": "0" * 40,
                "franken_numpy": "0" * 40,
                "frankenscipy": "0" * 40,
                "frankentorch": "0" * 40,
            },
            "findings": [],
            "artifacts": {},
        }
        error_finding = {
            "code": "invalid_contract",
            "message": "invalid evidence contract",
            "severity": "error",
            "stage": "preflight",
            "context": {},
        }
        valid_reports = [copy.deepcopy(base_report)]
        failed = copy.deepcopy(base_report)
        failed["status"] = "failed"
        failed["gates"]["locality"] = "refuted"
        valid_reports.append(failed)
        abstained = copy.deepcopy(base_report)
        abstained["status"] = "abstained"
        abstained["gates"]["locality"] = "unresolved"
        valid_reports.append(abstained)
        blocked_refutation = copy.deepcopy(failed)
        blocked_refutation["status"] = "abstained"
        blocked_refutation["findings"] = [error_finding]
        valid_reports.append(blocked_refutation)
        diagnostic = copy.deepcopy(base_report)
        diagnostic["mode"] = "exploratory"
        diagnostic["status"] = "diagnostic_only"
        valid_reports.append(diagnostic)
        for index, report in enumerate(valid_reports):
            check(
                not list(audit_validator.iter_errors(report)),
                f"valid typed audit report fixture {index} was rejected",
            )

        invalid_reports: list[dict] = []
        missing_gate = copy.deepcopy(base_report)
        del missing_gate["gates"]["locality"]
        invalid_reports.append(missing_gate)
        passed_unresolved = copy.deepcopy(base_report)
        passed_unresolved["gates"]["square_flatness"] = "unresolved"
        invalid_reports.append(passed_unresolved)
        passed_with_error = copy.deepcopy(base_report)
        passed_with_error["findings"] = [error_finding]
        invalid_reports.append(passed_with_error)
        exploratory_pass = copy.deepcopy(base_report)
        exploratory_pass["mode"] = "exploratory"
        invalid_reports.append(exploratory_pass)
        failed_without_refutation = copy.deepcopy(base_report)
        failed_without_refutation["status"] = "failed"
        invalid_reports.append(failed_without_refutation)
        failed_with_error = copy.deepcopy(failed)
        failed_with_error["findings"] = [error_finding]
        invalid_reports.append(failed_with_error)
        unexplained_abstention = copy.deepcopy(base_report)
        unexplained_abstention["status"] = "abstained"
        invalid_reports.append(unexplained_abstention)
        clean_refutation_as_abstention = copy.deepcopy(failed)
        clean_refutation_as_abstention["status"] = "abstained"
        clean_refutation_as_abstention["gates"]["orientation"] = "unresolved"
        invalid_reports.append(clean_refutation_as_abstention)
        for index, report in enumerate(invalid_reports):
            check(
                bool(list(audit_validator.iter_errors(report))),
                f"invalid typed audit report fixture {index} was accepted",
            )

    check(routing_view_schema is not None, "benchmark routing-view schema was not loaded")
    check(design_receipt_schema is not None, "design-authority receipt schema was not loaded")
    check(benchmark_oracle_schema is not None, "benchmark oracle schema was not loaded")
    check(design_diagnostic_schema is not None, "design diagnostic schema was not loaded")
    if (
        routing_view_schema is not None
        and design_receipt_schema is not None
        and benchmark_oracle_schema is not None
        and design_diagnostic_schema is not None
    ):
        benchmark_dir = ROOT / "examples" / "benchmarks" / "authority_ablation_template"
        routing_view = load_json(benchmark_dir / "routing_view.json")
        authorized = load_json(benchmark_dir / "authorized_design_receipt.json")
        blind = load_json(benchmark_dir / "blind_design_receipt.json")
        oracle = load_json(benchmark_dir / "oracle.json")
        diagnostic = load_json(benchmark_dir / "design_diagnostic_receipt.json")
        benchmark_documents = [
            (routing_view, Draft202012Validator(routing_view_schema), "routing view"),
            (authorized, Draft202012Validator(design_receipt_schema), "authorized receipt"),
            (blind, Draft202012Validator(design_receipt_schema), "blind receipt"),
            (oracle, Draft202012Validator(benchmark_oracle_schema), "oracle"),
            (diagnostic, Draft202012Validator(design_diagnostic_schema), "design diagnostic"),
        ]
        for document, document_validator, label in benchmark_documents:
            for error in sorted(
                document_validator.iter_errors(document), key=lambda item: list(item.path)
            ):
                location = ".".join(str(part) for part in error.path) or "<root>"
                fail(f"authority-template {label} schema violation at {location}: {error.message}")

        if all(isinstance(document, dict) for document in [routing_view, authorized, blind, oracle]):
            for error in authority_template_semantic_errors(
                routing_view, authorized, blind, oracle, benchmark_dir
            ):
                fail(f"authority-template semantic violation: {error}")
            source_table_path = benchmark_dir / "source_table.csv"
            routing_data_path = benchmark_dir / "routing_data.csv"
            confirmation_data_path = benchmark_dir / "confirmation_data.csv"
            transformation_path = benchmark_dir / "transformation.json"
            discovery_path = benchmark_dir / "discovery_units.txt"
            confirmation_path = benchmark_dir / "confirmation_units.txt"
            transformation = load_json(transformation_path)
            expected_hashes = {
                "source_table_sha256": sha256(source_table_path),
                "routing_data_sha256": sha256(routing_data_path),
                "confirmation_data_sha256": sha256(confirmation_data_path),
                "transformation_sha256": sha256(transformation_path),
                "discovery_unit_sha256": sha256(discovery_path),
                "confirmation_unit_sha256": sha256(confirmation_path),
            }
            binding_fields = [
                "execution_status",
                "benchmark_id",
                "routing_view_id",
                "source_table_sha256",
                "routing_data_sha256",
                "confirmation_data_sha256",
                "transformation_sha256",
                "discovery_unit_sha256",
                "confirmation_unit_sha256",
            ]
            for field in binding_fields:
                values = {document.get(field) for document in [routing_view, authorized, blind, oracle]}
                check(len(values) == 1, f"authority-template binding drift in {field}")
            for document, _, label in benchmark_documents[:4]:
                for field, expected_hash in expected_hashes.items():
                    check(
                        document.get(field) == expected_hash,
                        f"authority-template {label} does not bind actual {field}",
                    )

            check(routing_view.get("authority") == "proposal_only", "routing view grants causal authority")
            sealed_context = routing_view.get("sealed_context", {})
            if isinstance(sealed_context, dict):
                check(
                    sealed_context and not any(sealed_context.values()),
                    "routing view exposes sealed benchmark context",
                )

            neutral_columns = [
                item
                for item in routing_view.get("neutral_columns", [])
                if isinstance(item, dict)
            ]
            column_ids = [item.get("column_id") for item in neutral_columns]
            check(unique_field(neutral_columns, "column_id"), "routing view has duplicate column IDs")
            with routing_data_path.open(newline="", encoding="utf-8") as handle:
                rows = list(csv.DictReader(handle))
                table_columns = list(handle.seek(0) or next(csv.reader(handle)))
            check(
                column_ids == table_columns,
                "routing view neutral-column order does not match routing table",
            )
            transformation_columns = (
                transformation.get("ordered_columns", [])
                if isinstance(transformation, dict)
                else []
            )
            check(
                transformation_columns == table_columns,
                "transformation does not bind the routing-table column order",
            )

            queries = [
                item for item in routing_view.get("queries", []) if isinstance(item, dict)
            ]
            query_id_list = [item.get("query_id") for item in queries]
            check(unique_field(queries, "query_id"), "routing view has duplicate query IDs")
            query_ids = set(query_id_list)
            for query in queries:
                check(
                    query_columns_resolve(query, set(column_ids)),
                    f"query {query.get('query_id')} has invalid column references",
                )

            authorization_items = [
                item
                for item in authorized.get("estimand_authorizations", [])
                if isinstance(item, dict)
            ]
            authorization_id_list = [item.get("query_id") for item in authorization_items]
            check(
                unique_field(authorization_items, "query_id"),
                "authorized receipt has duplicate query IDs",
            )
            authorization_ids = set(authorization_id_list)
            oracle_items = [
                item for item in oracle.get("expected_routes", []) if isinstance(item, dict)
            ]
            oracle_id_list = [item.get("query_id") for item in oracle_items]
            check(
                unique_field(oracle_items, "query_id"),
                "benchmark oracle has duplicate query IDs",
            )
            oracle_ids = set(oracle_id_list)
            check(
                query_ids == authorization_ids == oracle_ids,
                "authority template does not bind every frozen query",
            )
            check(
                authorized.get("status") == "illustrative_supplied",
                "authorized template receipt is not illustrative",
            )
            check(blind.get("status") == "withheld", "blind receipt does not withhold authority")
            check(
                not {
                    "assignment",
                    "estimand_authorizations",
                    "premise_evidence",
                    "use_scope",
                }.intersection(blind),
                "blind receipt leaks design authority",
            )

            assignment = authorized.get("assignment", {})
            if not isinstance(assignment, dict):
                assignment = {}
            assignment_column = assignment.get("assignment_column")
            assignment_unit = assignment.get("assignment_unit_column")
            check(assignment_column in column_ids, "assignment references an unknown column")
            check(assignment_unit in column_ids, "assignment unit references an unknown column")
            unit_role_columns = {
                item.get("column_id")
                for item in neutral_columns
                if "unit" in item.get("candidate_roles", [])
            }
            check(
                assignment_unit in unit_role_columns,
                "assignment unit is not declared as a neutral unit column",
            )

            discovery_units = {
                line.strip()
                for line in discovery_path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            }
            confirmation_units = {
                line.strip()
                for line in confirmation_path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            }
            with source_table_path.open(newline="", encoding="utf-8") as handle:
                source_rows = list(csv.DictReader(handle))
            with confirmation_data_path.open(newline="", encoding="utf-8") as handle:
                confirmation_rows = list(csv.DictReader(handle))
            data_units = {row.get(str(assignment_unit)) for row in source_rows}
            routing_units = {row.get(str(assignment_unit)) for row in rows}
            confirmation_data_units = {
                row.get(str(assignment_unit)) for row in confirmation_rows
            }
            check(not (discovery_units & confirmation_units), "unit partitions overlap")
            check(
                discovery_units | confirmation_units == data_units,
                "unit partitions do not exactly cover the source table",
            )
            check(routing_units == discovery_units, "routing table is not discovery-only")
            check(
                confirmation_data_units == confirmation_units,
                "confirmation table does not match the sealed partition",
            )

            evidence_items = [
                item for item in authorized.get("premise_evidence", []) if isinstance(item, dict)
            ]
            evidence_id_list = [item.get("evidence_id") for item in evidence_items]
            check(
                unique_field(evidence_items, "evidence_id"),
                "authorized receipt has duplicate evidence IDs",
            )
            evidence_by_id = {item.get("evidence_id"): item for item in evidence_items}
            benchmark_root = benchmark_dir.resolve()
            for evidence in evidence_items:
                relative_path = evidence.get("relative_path")
                evidence_path = (benchmark_dir / str(relative_path)).resolve()
                try:
                    evidence_path.relative_to(benchmark_root)
                    inside_benchmark = True
                except ValueError:
                    inside_benchmark = False
                check(inside_benchmark, f"evidence {evidence.get('evidence_id')} escapes fixture")
                check(evidence_path.is_file(), f"evidence {evidence.get('evidence_id')} is missing")
                if evidence_path.is_file():
                    check(
                        evidence.get("content_sha256") == sha256(evidence_path),
                        f"evidence {evidence.get('evidence_id')} has a stale digest",
                    )

            authorizations = {item.get("query_id"): item for item in authorization_items}
            oracle_routes = {item.get("query_id"): item for item in oracle_items}
            expected_routes = {
                ("randomized_encouragement", "recorded_randomization", "offer_itt"): {
                    "assignment_integrity",
                    "correct_unit",
                    "consistency",
                    "positivity",
                },
                ("randomized_encouragement", "instrumental_variable", "complier_late"): {
                    "relevance",
                    "exclusion",
                    "independence",
                    "monotonicity",
                    "correct_unit",
                    "consistency",
                    "positivity",
                },
            }
            for query_id, authorization in authorizations.items():
                premise_items = [
                    item
                    for item in authorization.get("required_premises", [])
                    if isinstance(item, dict)
                ]
                premise_names = [item.get("name") for item in premise_items]
                check(
                    unique_field(premise_items, "name"),
                    f"authorization {query_id} has duplicate premise names",
                )
                check(
                    route_is_allowed(assignment.get("kind"), authorization, expected_routes),
                    f"authorization {query_id} has an invalid route or premise set",
                )
                check(
                    authorization_evidence_resolves(authorization, set(evidence_by_id)),
                    f"authorization {query_id} has unresolved premise evidence",
                )
                for premise in premise_items:
                    evidence_ref = premise.get("evidence_ref")
                    check(
                        evidence_ref in evidence_by_id,
                        f"authorization {query_id} has unresolved evidence {evidence_ref}",
                    )
                    evidence = evidence_by_id.get(evidence_ref, {})
                    expected_class = (
                        "design_diagnostic_receipt"
                        if premise.get("status") == "empirically_checked_not_design_authority"
                        else "external_design_assertion"
                    )
                    check(
                        evidence.get("evidence_class") == expected_class,
                        f"authorization {query_id} misclassifies evidence {evidence_ref}",
                    )
                oracle_route = oracle_routes.get(query_id, {})
                check(
                    authorization_matches_oracle(authorization, oracle_route),
                    f"authorization {query_id} disagrees with the oracle",
                )

            authority_source_hash = assignment.get("authority_source_sha256")
            check(
                authority_source_hash == oracle.get("source_document_sha256"),
                "assignment authority source disagrees with the oracle source",
            )
            check(
                any(
                    item.get("content_sha256") == authority_source_hash
                    and item.get("evidence_class") == "external_design_assertion"
                    for item in evidence_items
                ),
                "assignment authority source has no content-bound evidence",
            )
            check(
                authorized.get("use_scope") == oracle.get("use_scope"),
                "authorized use scope disagrees with the oracle",
            )

            blind_text = json.dumps([routing_view, blind], sort_keys=True).lower()
            for leaked_term in ["oregon", "medicaid", "lottery", "instrument", "coverage"]:
                check(leaked_term not in blind_text, f"blind routing artifacts leak {leaked_term}")

            invalid_blind = copy.deepcopy(blind)
            invalid_blind["estimand_authorizations"] = authorized.get(
                "estimand_authorizations", []
            )
            check(
                bool(
                    list(
                        Draft202012Validator(design_receipt_schema).iter_errors(
                            invalid_blind
                        )
                    )
                ),
                "withheld design receipt can smuggle estimand authority",
            )

            routing_validator = Draft202012Validator(routing_view_schema)
            receipt_validator = Draft202012Validator(design_receipt_schema)
            oracle_validator = Draft202012Validator(benchmark_oracle_schema)

            def template_rejected(
                candidate_routing: dict,
                candidate_authorized: dict,
                candidate_blind: dict,
                candidate_oracle: dict,
            ) -> bool:
                schema_errors = [
                    *routing_validator.iter_errors(candidate_routing),
                    *receipt_validator.iter_errors(candidate_authorized),
                    *receipt_validator.iter_errors(candidate_blind),
                    *oracle_validator.iter_errors(candidate_oracle),
                ]
                semantic_errors = authority_template_semantic_errors(
                    candidate_routing,
                    candidate_authorized,
                    candidate_blind,
                    candidate_oracle,
                    benchmark_dir,
                )
                return bool(schema_errors or semantic_errors)

            duplicate_query = copy.deepcopy(routing_view)
            duplicate_query["queries"].append(copy.deepcopy(duplicate_query["queries"][0]))
            check(
                template_rejected(duplicate_query, authorized, blind, oracle),
                "duplicate-query adversary passed the real template validator",
            )
            nonexistent_assignment = copy.deepcopy(authorized)
            nonexistent_assignment["assignment"]["assignment_column"] = "x_999"
            check(
                template_rejected(routing_view, nonexistent_assignment, blind, oracle),
                "unknown-assignment adversary passed the real template validator",
            )
            wrong_design = copy.deepcopy(authorized)
            wrong_design["assignment"]["kind"] = "regression_discontinuity"
            check(
                template_rejected(routing_view, wrong_design, blind, oracle),
                "strategy-kind adversary passed the real template validator",
            )
            wrong_oracle = copy.deepcopy(oracle)
            wrong_oracle["expected_routes"][0]["authorized_estimand"] = "treatment_ate"
            check(
                template_rejected(routing_view, authorized, blind, wrong_oracle),
                "oracle-estimand adversary passed the real template validator",
            )
            unresolved_evidence = copy.deepcopy(authorized)
            unresolved_evidence["estimand_authorizations"][0]["required_premises"][0][
                "evidence_ref"
            ] = "ev_missing"
            check(
                template_rejected(routing_view, unresolved_evidence, blind, oracle),
                "missing-evidence adversary passed the real template validator",
            )
            downgraded_exclusion = copy.deepcopy(authorized)
            exclusion = downgraded_exclusion["estimand_authorizations"][1][
                "required_premises"
            ][1]
            exclusion["status"] = "empirically_checked_not_design_authority"
            exclusion["evidence_ref"] = "ev_relevance"
            check(
                template_rejected(routing_view, downgraded_exclusion, blind, oracle),
                "empirical-exclusion adversary passed the real template validator",
            )
            swapped_exposures = copy.deepcopy(routing_view)
            swapped_exposures["queries"][0]["exposure_column"] = "x_002"
            swapped_exposures["queries"][1]["exposure_column"] = "x_001"
            check(
                template_rejected(swapped_exposures, authorized, blind, oracle),
                "swapped-exposure adversary passed the real template validator",
            )
            decorative_contract = copy.deepcopy(authorized)
            decorative_contract["assignment"]["probability_contract"] = "not_applicable"
            decorative_contract["assignment"]["timing_contract"] = "cutoff_precedes_outcome"
            check(
                template_rejected(routing_view, decorative_contract, blind, oracle),
                "invalid-randomization-contract adversary passed the real validator",
            )
            wrong_evidence_subject = copy.deepcopy(authorized)
            wrong_evidence_subject["estimand_authorizations"][0]["required_premises"][0][
                "evidence_ref"
            ] = "ev_semantics"
            check(
                template_rejected(routing_view, wrong_evidence_subject, blind, oracle),
                "wrong-evidence-subject adversary passed the real validator",
            )
            raw_csv_as_diagnostic = copy.deepcopy(authorized)
            raw_evidence = raw_csv_as_diagnostic["premise_evidence"][2]
            raw_evidence["relative_path"] = "routing_data.csv"
            raw_evidence["content_sha256"] = sha256(routing_data_path)
            raw_evidence["evidence_class"] = "empirical_diagnostic"
            check(
                template_rejected(routing_view, raw_csv_as_diagnostic, blind, oracle),
                "raw-CSV diagnostic laundering passed the real validator",
            )
            leaky_identifier = copy.deepcopy(routing_view)
            leaky_identifier["benchmark_id"] = "randomized_encouragement_late"
            check(
                template_rejected(leaky_identifier, authorized, blind, oracle),
                "semantic-identifier leakage adversary passed the real validator",
            )

    if four_law_report_schema is not None:
        four_law_validator = Draft202012Validator(four_law_report_schema)
        four_law_report = {
            "schema_version": "2.0.0",
            "experiment_id": "schema-conformance",
            "status": "abstained",
            "gates": {
                "locality": "unresolved",
                "conditional_normalization": "unresolved",
                "square_flatness": "unresolved",
                "orientation": "unresolved",
            },
            "preflight": {"manifest_canonical_sha256": "0" * 64},
            "ingest": {
                "fingerprint": {
                    "content_sha256": "0" * 64,
                    "cluster_fingerprint": "0" * 64,
                    "n_rows": 0,
                    "n_included_clusters": 0,
                },
                "regime_counts": [],
                "clusters_spanning_regimes": [],
                "missing_regimes": [],
            },
            "four_law": [],
            "projection": {},
            "ledger": {
                "schema_version": "1.0.0",
                "mode": "strict",
                "findings": [],
                "provenance": {},
            },
        }
        check(
            not list(four_law_validator.iter_errors(four_law_report)),
            "valid non-certifying four-law v2 report was rejected",
        )
        invalid_four_law_reports: list[dict] = []
        passed_four_law = copy.deepcopy(four_law_report)
        passed_four_law["status"] = "passed"
        invalid_four_law_reports.append(passed_four_law)
        established_four_law = copy.deepcopy(four_law_report)
        established_four_law["gates"]["square_flatness"] = "established"
        invalid_four_law_reports.append(established_four_law)
        mismatched_mode = copy.deepcopy(four_law_report)
        mismatched_mode["ledger"]["mode"] = "exploratory"
        invalid_four_law_reports.append(mismatched_mode)
        stale_version = copy.deepcopy(four_law_report)
        stale_version["schema_version"] = "1.0.0"
        invalid_four_law_reports.append(stale_version)
        missing_manifest_binding = copy.deepcopy(four_law_report)
        del missing_manifest_binding["preflight"]["manifest_canonical_sha256"]
        invalid_four_law_reports.append(missing_manifest_binding)
        malformed_manifest_binding = copy.deepcopy(four_law_report)
        malformed_manifest_binding["preflight"]["manifest_canonical_sha256"] = "not-a-sha256"
        invalid_four_law_reports.append(malformed_manifest_binding)
        for index, report in enumerate(invalid_four_law_reports):
            check(
                bool(list(four_law_validator.iter_errors(report))),
                f"invalid four-law report fixture {index} was accepted",
            )

    check(proposal_input_schema is not None, "active-tilt input schema was not loaded")
    if proposal_input_schema is not None:
        input_validator = Draft202012Validator(proposal_input_schema)
        for path in sorted((ROOT / "examples" / "proposal_inputs").glob("*.json")):
            proposal_input = load_json(path)
            errors = sorted(input_validator.iter_errors(proposal_input), key=lambda error: list(error.path))
            for error in errors:
                location = ".".join(str(part) for part in error.path) or "<root>"
                fail(f"{path.relative_to(ROOT)} proposal-input schema violation at {location}: {error.message}")
            if not isinstance(proposal_input, dict):
                continue
            request = proposal_input.get("request", {})
            candidates = proposal_input.get("candidates", [])
            if not isinstance(request, dict) or not isinstance(candidates, list):
                continue
            feature_flags = request.get("source", {}).get("feature_flags", [])
            check(feature_flags == sorted(set(feature_flags)), f"{path.name} input feature flags are not canonicalized")
            candidate_ids = [candidate.get("candidate_id") for candidate in candidates if isinstance(candidate, dict)]
            check(len(candidate_ids) == len(set(candidate_ids)), f"{path.name} input repeats a candidate identifier")

    check(orientation_schema is not None, "orientation input schema was not loaded")
    if orientation_schema is not None:
        orientation_validator = Draft202012Validator(orientation_schema)
        for path in sorted((ROOT / "examples" / "orientation").glob("*.json")):
            orientation = load_json(path)
            errors = sorted(orientation_validator.iter_errors(orientation), key=lambda error: list(error.path))
            for error in errors:
                location = ".".join(str(part) for part in error.path) or "<root>"
                fail(f"{path.relative_to(ROOT)} orientation schema violation at {location}: {error.message}")
            if not isinstance(orientation, dict):
                continue
            deletions = orientation.get("deletions", [])
            variables = [row.get("variable") for row in deletions if isinstance(row, dict)]
            check(len(variables) == len(set(variables)), f"{path.name} repeats a deletion variable")
            for row in deletions:
                if not isinstance(row, dict):
                    continue
                lower = float(row.get("lower", math.nan))
                point = float(row.get("relative_discrepancy", math.nan))
                upper = float(row.get("upper", math.nan))
                check(lower <= point <= upper, f"{path.name} has misordered deletion bounds for {row.get('variable')}")

    check(proposal_schema is not None, "proposal batch schema was not loaded")
    if proposal_schema is not None:
        proposal_validator = Draft202012Validator(proposal_schema)
        for path in sorted((ROOT / "examples" / "proposals").glob("*.json")):
            proposal = load_json(path)
            errors = sorted(proposal_validator.iter_errors(proposal), key=lambda error: list(error.path))
            for error in errors:
                location = ".".join(str(part) for part in error.path) or "<root>"
                fail(f"{path.relative_to(ROOT)} proposal schema violation at {location}: {error.message}")
            if not isinstance(proposal, dict):
                continue
            check(proposal.get("authority") == "proposal_only", f"{path.name} grants proposal artifact certificate authority")
            fingerprint = str(proposal.get("candidate_library_fingerprint", ""))
            check(bool(re.fullmatch(r"sha256:[0-9a-f]{64}", fingerprint)), f"{path.name} has an invalid candidate-library fingerprint")
            hypotheses = proposal.get("surviving_hypotheses", [])
            check(hypotheses == sorted(hypotheses), f"{path.name} hypotheses are not canonicalized")
            expected_pairs = {
                (hypotheses[left], hypotheses[right])
                for left in range(len(hypotheses))
                for right in range(left + 1, len(hypotheses))
            }
            source = proposal.get("source", {})
            feature_flags = source.get("feature_flags", []) if isinstance(source, dict) else []
            check(feature_flags == sorted(set(feature_flags)), f"{path.name} feature flags are not canonicalized")
            rankings = proposal.get("rankings", [])
            if isinstance(rankings, list):
                ranks = [item.get("rank") for item in rankings if isinstance(item, dict)]
                check(ranks == list(range(1, len(rankings) + 1)), f"{path.name} ranks are not consecutive")
                scores = [item.get("worst_case_predicted_separation") for item in rankings if isinstance(item, dict)]
                check(scores == sorted(scores, reverse=True), f"{path.name} maximin scores are not descending")
                candidate_ids = [item.get("candidate_id") for item in rankings if isinstance(item, dict)]
                check(len(candidate_ids) == len(set(candidate_ids)), f"{path.name} repeats a ranked candidate")
                for item in rankings:
                    if not isinstance(item, dict):
                        continue
                    check(item.get("primitive_id") == proposal.get("primitive_id"), f"{path.name} ranked tilt changes primitive")
                    predictions = item.get("predicted_pairwise_separations", [])
                    pair_keys = [
                        (prediction.get("first"), prediction.get("second"))
                        for prediction in predictions
                        if isinstance(prediction, dict)
                    ]
                    check(pair_keys == sorted(pair_keys), f"{path.name} pairwise predictions are not canonicalized")
                    check(set(pair_keys) == expected_pairs and len(pair_keys) == len(expected_pairs), f"{path.name} lacks a complete unique hypothesis-pair table")
                    values = [
                        float(prediction["separation"])
                        for prediction in predictions
                        if isinstance(prediction, dict) and "separation" in prediction
                    ]
                    score = float(item.get("worst_case_predicted_separation", math.nan))
                    check(bool(values) and math.isclose(score, min(values), abs_tol=1e-14), f"{path.name} maximin score does not match raw predictions")
                    if proposal.get("planned_analysis") == "product_factorial":
                        eligibility = item.get("design_eligibility", {})
                        status = eligibility.get("status") if isinstance(eligibility, dict) else None
                        check(status in {"product_odds_verified", "reweighted_to_product"}, f"{path.name} ranks a product-factorial tilt without design evidence")
                selected = proposal.get("selected_candidate_id")
                expected = rankings[0].get("candidate_id") if rankings and isinstance(rankings[0], dict) else None
                check(selected == expected, f"{path.name} selected candidate does not match rank one")
                expected_status = "recommended" if rankings else "abstained_no_eligible_candidate"
                check(proposal.get("status") == expected_status, f"{path.name} proposal status does not match eligibility")
                rejected = proposal.get("rejected", [])
                rejected_ids = {item.get("candidate_id") for item in rejected if isinstance(item, dict)}
                check(not rejected_ids.intersection(candidate_ids), f"{path.name} both ranks and rejects a candidate")
            semantics = str(proposal.get("score_semantics", "")).lower()
            check("not probability or confidence" in semantics, f"{path.name} does not quarantine proposal score semantics")
            ranking_policy = str(proposal.get("ranking_policy", "")).lower()
            check("candidate_id" in ranking_policy and "cost" in ranking_policy, f"{path.name} does not freeze deterministic tie-breaking")

            input_path = ROOT / "examples" / "proposal_inputs" / path.name
            if input_path.is_file():
                proposal_input = load_json(input_path)
                if isinstance(proposal_input, dict):
                    request = proposal_input.get("request", {})
                    candidates = proposal_input.get("candidates", [])
                    if isinstance(request, dict) and isinstance(candidates, list):
                        for field in ["schema_version", "proposal_id", "primitive_id", "planned_analysis", "source", "seed"]:
                            check(proposal.get(field) == request.get(field), f"{path.name} output drifted from input field {field}")
                        check(proposal.get("surviving_hypotheses") == sorted(request.get("surviving_hypotheses", [])), f"{path.name} output hypotheses drifted from input")
                        input_ids = {item.get("candidate_id") for item in candidates if isinstance(item, dict)}
                        output_ids = {
                            item.get("candidate_id")
                            for collection in [proposal.get("rankings", []), proposal.get("rejected", [])]
                            for item in collection
                            if isinstance(item, dict)
                        }
                        check(input_ids == output_ids, f"{path.name} does not account for every input candidate")
                        expected_fingerprint = active_tilt_candidate_fingerprint(candidates)
                        check(proposal.get("candidate_library_fingerprint") == expected_fingerprint, f"{path.name} candidate-library fingerprint drifted from input")

    check(manifest_schema is not None, "experiment manifest schema was not loaded")
    if manifest_schema is None:
        return
    validator = Draft202012Validator(manifest_schema)
    for path in sorted((ROOT / "examples" / "configs").glob("*.json")):
        manifest = load_json(path)
        errors = sorted(validator.iter_errors(manifest), key=lambda error: list(error.path))
        for error in errors:
            location = ".".join(str(part) for part in error.path) or "<root>"
            fail(f"{path.relative_to(ROOT)} schema violation at {location}: {error.message}")
        if not isinstance(manifest, dict):
            continue
        regimes = manifest.get("regimes", [])
        if isinstance(regimes, list):
            proportions = [float(regime["sampling_proportion"]) for regime in regimes if isinstance(regime, dict)]
            check(math.isclose(sum(proportions), 1.0, abs_tol=1e-10), f"sampling proportions do not sum to one in {path.name}")
            dimensions = {
                len(regime.get("design", {}).get("bits", []))
                for regime in regimes
                if isinstance(regime, dict)
            }
            check(len(dimensions) == 1, f"regime dimensions differ in {path.name}")
            ids = [regime.get("id") for regime in regimes if isinstance(regime, dict)]
            corners = [tuple(regime.get("design", {}).get("bits", [])) for regime in regimes if isinstance(regime, dict)]
            check(len(ids) == len(set(ids)), f"duplicate regime id in {path.name}")
            check(len(corners) == len(set(corners)), f"duplicate design corner in {path.name}")
        data = manifest.get("data", {})
        if isinstance(data, dict) and data.get("format") != "synthetic":
            source = ROOT / str(data.get("path", ""))
            check(source.is_file(), f"manifest data path does not exist: {source.relative_to(ROOT) if source.is_absolute() and ROOT in source.parents else source}")
            if source.suffix.lower() == ".csv" and source.is_file():
                with source.open(newline="", encoding="utf-8") as handle:
                    reader = csv.DictReader(handle)
                    headers = set(reader.fieldnames or [])
                    rows = list(reader)
                required_columns = {
                    manifest.get("cluster_column"),
                    manifest.get("regime_column"),
                    *manifest.get("state_columns", []),
                    *(column for block in manifest.get("candidate_state_blocks", []) for column in block),
                }
                required_columns.discard(None)
                check(required_columns <= headers, f"{source.name} is missing declared columns {sorted(required_columns - headers)}")
                check(len(rows) > 0, f"{source.name} contains no data rows")



def validate_repository_manifest() -> None:
    path = ROOT / "REPOSITORY_MANIFEST.json"
    if not path.is_file():
        return
    document = load_json(path)
    if not isinstance(document, dict):
        return
    check(document.get("schema_version") == "1.0.0", "unexpected repository manifest schema version")
    check(document.get("hash_algorithm") == "sha256", "repository manifest must use sha256")
    files = document.get("files")
    check(isinstance(files, list), "repository manifest files must be a list")
    if not isinstance(files, list):
        return
    entries: dict[str, dict] = {}
    for item in files:
        check(isinstance(item, dict), "repository manifest contains a non-object entry")
        if not isinstance(item, dict):
            continue
        relative = item.get("path")
        check(isinstance(relative, str) and bool(relative), "repository manifest entry has invalid path")
        if not isinstance(relative, str) or not relative:
            continue
        check(relative not in entries, f"repository manifest repeats {relative}")
        entries[relative] = item
        target = ROOT / relative
        check(target.is_file(), f"repository manifest path is missing: {relative}")
        if target.is_file():
            check(item.get("bytes") == target.stat().st_size, f"repository manifest byte count drift: {relative}")
            check(item.get("sha256") == sha256(target), f"repository manifest hash drift: {relative}")
    actual = {path.relative_to(ROOT).as_posix() for path in manifest_paths()}
    declared = set(entries)
    check(actual == declared, f"repository manifest path set drift: missing={sorted(actual - declared)}, extra={sorted(declared - actual)}")
    check(document.get("file_count") == len(files), "repository manifest file_count drift")
    aggregate = hashlib.sha256()
    for relative in sorted(entries):
        aggregate.update(relative.encode())
        aggregate.update(b"\0")
        aggregate.update(str(entries[relative].get("sha256", "")).encode())
        aggregate.update(b"\n")
    check(document.get("aggregate_sha256") == aggregate.hexdigest(), "repository manifest aggregate hash drift")

def validate_site() -> None:
    index = ROOT / "site" / "index.html"
    if not index.is_file():
        return
    document = html.fromstring(index.read_text(encoding="utf-8"))
    ids = document.xpath("//@id")
    check(len(ids) == len(set(ids)), "site has duplicate HTML ids")
    id_set = set(ids)
    for href in document.xpath("//@href"):
        if href.startswith("#"):
            check(href[1:] in id_set, f"site fragment target is missing: {href}")
        elif re.match(r"^(?:https?:|mailto:|tel:|data:)", href):
            continue
        else:
            target = (ROOT / "site" / href.split("#", 1)[0]).resolve()
            check(target.is_file(), f"site link target is missing: {href}")
    for source in document.xpath("//@src"):
        if re.match(r"^(?:https?:|data:)", source):
            continue
        target = (ROOT / "site" / source).resolve()
        check(target.is_file(), f"site asset is missing: {source}")
    title = "".join(document.xpath("/html/head/title/text()")).strip()
    check(title == "Mechanism Interferometry", "unexpected site title")
    remote = re.findall(r"(?:src|href)=[\"']https?://", index.read_text(encoding="utf-8"))
    check(not remote, "static site contains a remote dependency")
    node = shutil_which("node")
    if node:
        result = subprocess.run([node, "--check", str(ROOT / "site" / "app.js")], capture_output=True, text=True, check=False)
        check(result.returncode == 0, f"site JavaScript syntax failed: {result.stderr.strip()}")


def validate_paper() -> None:
    paper = ROOT / "paper" / "main.pdf"
    site_copy = ROOT / "site" / "mechanism_interferometry.pdf"
    if paper.is_file() and site_copy.is_file():
        check(sha256(paper) == sha256(site_copy), "website paper copy differs from paper/main.pdf")
    pdfinfo = shutil_which("pdfinfo")
    if pdfinfo and paper.is_file():
        result = subprocess.run([pdfinfo, str(paper)], capture_output=True, text=True, check=False)
        check(result.returncode == 0, f"pdfinfo failed: {result.stderr.strip()}")
        match = re.search(r"^Pages:\s+(\d+)$", result.stdout, re.MULTILINE)
        check(match is not None and int(match.group(1)) >= 30, "paper PDF is unexpectedly short")
    source = (ROOT / "paper" / "main.tex").read_text(encoding="utf-8")
    for token in [
        "Finite modular soft-intervention certificate",
        "curvature-balance",
        "Nested state expansion",
        "Generalised Covariance Measure",
        "Partial factorial designs",
        "Representation learning",
    ]:
        check(token.lower() in source.lower(), f"paper source is missing core section/token: {token}")
    log = ROOT / "paper" / "main.log"
    if log.is_file():
        text = log.read_text(encoding="utf-8", errors="replace")
        check("undefined references" not in text.lower(), "paper build has undefined references")
        check("citation" not in "\n".join(line.lower() for line in text.splitlines() if "undefined" in line.lower()), "paper build has undefined citations")


def validate_proposal_boundary() -> None:
    path = ROOT / "docs" / "PROPOSAL_ADAPTERS.md"
    if not path.is_file():
        return
    source = path.read_text(encoding="utf-8")
    for token in [
        "It may decide what to test next. It may not decide what is true.",
        "diagnostic_only",
        "may not break a `MULTIPLE_PASSES`",
        "must preserve the asserted primitive target",
        "product-odds rule",
        "mic_proposal::rank_active_tilts",
        "proposal_only",
    ]:
        check(token in source, f"proposal-adapter contract is missing boundary token: {token}")


def validate_simulations() -> None:
    results = load_json(ROOT / "artifacts" / "simulations" / "exact_results.json")
    if not isinstance(results, dict):
        return
    running = results.get("running_example", {})
    latent = results.get("latent_conservation", {})
    implementation = results.get("implementation_inconsistency", {})
    hidden_sensor = results.get("hidden_sensor_tomography", {})
    parity = results.get("parity_orientation_failure", {})
    check(math.isclose(float(running.get("outcome_synergy", math.nan)), 0.3, abs_tol=1e-14), "running-example synergy drift")
    check(math.isclose(float(running.get("full_state_curvature", math.nan)), 0.0, abs_tol=1e-14), "running-example full curvature drift")
    check(math.isclose(float(latent.get("cov_observed_ratios", math.nan)), -0.09, abs_tol=1e-14), "latent observable covariance drift")
    check(math.isclose(float(latent.get("mean_conditional_latent_covariance", math.nan)), 0.09, abs_tol=1e-14), "latent hidden covariance drift")
    check(math.isclose(float(implementation.get("normalizer", math.nan)), 1.063, abs_tol=1e-14), "implementation normalizer drift")
    check(int(parity.get("pass_count", -1)) == 2, "parity pass-count drift")
    hidden_curvature = hidden_sensor.get("observed_curvature", [math.nan, math.nan])
    check(
        len(hidden_curvature) == 2
        and math.isclose(float(hidden_curvature[0]), math.log(0.8), abs_tol=1e-14)
        and math.isclose(float(hidden_curvature[1]), math.log(1.2), abs_tol=1e-14),
        "hidden-sensor curvature drift",
    )
    check(
        hidden_sensor.get("infinitesimal_missing_rank") == [1, 1],
        "hidden-sensor rank drift",
    )
    for figure in [
        "running_example",
        "latent_conservation",
        "implementation_inconsistency",
        "hidden_sensor_tomography",
    ]:
        check((ROOT / "paper" / "figures" / f"{figure}.pdf").is_file(), f"missing PDF figure {figure}")
        check((ROOT / "paper" / "figures" / f"{figure}.png").is_file(), f"missing PNG figure {figure}")


def validate_cargo_and_sources() -> None:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    members = cargo.get("workspace", {}).get("members", [])
    for member in members:
        member_path = ROOT / member
        check((member_path / "Cargo.toml").is_file(), f"workspace member lacks Cargo.toml: {member}")
        source_files = sorted((member_path / "src").rglob("*.rs"))
        check(bool(source_files), f"workspace member has no Rust source: {member}")
        for source in source_files:
            text = source.read_text(encoding="utf-8")
            check(text.startswith("#![forbid(unsafe_code)]"), f"{source.relative_to(ROOT)} does not forbid unsafe code")
            check(re.search(r"\bunsafe\s*\{", text) is None, f"unsafe block in {source.relative_to(ROOT)}")
    dependency_text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    integration_text = (ROOT / "docs" / "FRANKEN_INTEGRATION.md").read_text(encoding="utf-8")
    revisions = {
        "frankenpandas": "9599d6f4a12306897a9bc19be3d2ba2ac228a97c",
        "franken_numpy": "6964e776528f1e492620ebd627d78d4f958220f4",
        "frankenscipy": "e259ed002eec05a2eca08d38a0763e0e58b0623c",
        "frankentorch": "5a3a0e70a2854c08e42ae02d816a78b8f88d912d",
    }
    for name, revision in revisions.items():
        check(revision in dependency_text, f"{name} revision missing from Cargo.toml")
        check(revision in integration_text, f"{name} revision missing from FRANKEN_INTEGRATION.md")

    stats_text = (ROOT / "crates" / "mic-stats" / "src" / "lib.rs").read_text(encoding="utf-8")
    for token in [
        "ProductDesignEvidence",
        "from_sampling_odds_audit",
        "from_reweighting_audit",
        "source_fingerprint",
        "DiagnosticOnly",
        "design_evidence: &ProductDesignEvidence",
    ]:
        check(token in stats_text, f"mic-stats GCM evidence boundary is missing token: {token}")


def rust_call_arguments(source: str, function_name: str) -> list[list[str]]:
    """Return top-level argument strings for Rust calls to one free function.

    This is deliberately a small lexical scanner rather than a Rust parser. It
    understands nested delimiters and quoted strings, which is enough to keep a
    multiline `finding_with_context` call from evading the repository rule by
    formatting alone.
    """
    calls: list[list[str]] = []
    pattern = re.compile(rf"\b{re.escape(function_name)}\s*\(")
    for match in pattern.finditer(source):
        prefix = source[max(0, match.start() - 24) : match.start()]
        if re.search(r"\bfn\s+$", prefix):
            continue
        arguments: list[str] = []
        start = match.end()
        item_start = start
        depth = 1
        quote: str | None = None
        escaped = False
        index = start
        while index < len(source) and depth > 0:
            character = source[index]
            if quote is not None:
                if escaped:
                    escaped = False
                elif character == "\\":
                    escaped = True
                elif character == quote:
                    quote = None
            elif character in {'"', "'"}:
                quote = character
            elif character in "([{":
                depth += 1
            elif character in ")]}":
                depth -= 1
                if depth == 0:
                    arguments.append(source[item_start:index].strip())
                    break
            elif character == "," and depth == 1:
                arguments.append(source[item_start:index].strip())
                item_start = index + 1
            index += 1
        if depth == 0:
            calls.append(arguments)
    return calls


def validate_finding_code_vocabulary() -> None:
    """Require every in-workspace finding emission to resolve to `mic_audit::code`."""
    audit_source = (ROOT / "crates" / "mic-audit" / "src" / "lib.rs").read_text(
        encoding="utf-8"
    )
    declarations = re.findall(
        r'pub const\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*"([a-z][a-z0-9_]*)"\s*;',
        audit_source,
    )
    declared_names = {name for name, _ in declarations}
    declared_values = {value for _, value in declarations}
    check(bool(declarations), "mic-audit declares no finding-code vocabulary")
    check(
        len(declared_values) == len(declarations),
        "mic-audit finding-code constants contain duplicate wire values",
    )

    scanner_fixture = (
        'ledger.push(finding_with_context(Severity::Error, format!("s,{x}"), '
        '"fixture_code", "message", context));'
    )
    fixture_calls = rust_call_arguments(scanner_fixture, "finding_with_context")
    check(
        len(fixture_calls) == 1
        and len(fixture_calls[0]) == 5
        and fixture_calls[0][2] == '"fixture_code"',
        "finding-code lexical scanner regression",
    )

    for source_path in sorted((ROOT / "crates").glob("*/src/**/*.rs")):
        source = source_path.read_text(encoding="utf-8")
        relative = source_path.relative_to(ROOT)
        for function_name in ["finding_with_context", "finding"]:
            for arguments in rust_call_arguments(source, function_name):
                check(
                    len(arguments) >= 3,
                    f"cannot parse {function_name} arguments in {relative}",
                )
                if len(arguments) < 3:
                    continue
                expression = arguments[2]
                literal = re.fullmatch(r'"([a-z][a-z0-9_]*)"', expression)
                constant = re.fullmatch(
                    r"code::(?:info::)?([A-Z][A-Z0-9_]*)", expression
                )
                if literal:
                    check(
                        literal.group(1) in declared_values,
                        f"undeclared finding code {literal.group(1)!r} emitted in {relative}",
                    )
                elif constant:
                    check(
                        constant.group(1) in declared_names,
                        f"unknown finding-code constant {expression} in {relative}",
                    )
                else:
                    check(
                        False,
                        f"dynamic finding-code expression {expression!r} in {relative}",
                    )

        for finding in re.finditer(r"\bFinding\s*\{(?P<body>.*?)\}", source, re.DOTALL):
            prefix = source[max(0, finding.start() - 24) : finding.start()]
            if re.search(r"\bstruct\s+$", prefix):
                continue
            code_match = re.search(r"\bcode\s*:\s*([^,\n]+)", finding.group("body"))
            if not code_match:
                continue
            expression = code_match.group(1).strip()
            if expression in {"code.into()", "code_value.into()"}:
                continue
            literal = re.fullmatch(r'"([a-z][a-z0-9_]*)"(?:\.into\(\))?', expression)
            constant = re.fullmatch(
                r"code::(?:info::)?([A-Z][A-Z0-9_]*)(?:\.into\(\))?", expression
            )
            if literal:
                check(
                    literal.group(1) in declared_values,
                    f"undeclared direct Finding code {literal.group(1)!r} in {relative}",
                )
            elif constant:
                check(
                    constant.group(1) in declared_names,
                    f"unknown direct Finding constant {expression} in {relative}",
                )
            else:
                check(False, f"dynamic direct Finding code {expression!r} in {relative}")


def validate_shell_scripts() -> None:
    for path in sorted((ROOT / "scripts").glob("*.sh")):
        check(os.access(path, os.X_OK), f"shell script is not executable: {path.relative_to(ROOT)}")
        bash = shutil_which("bash")
        if bash:
            result = subprocess.run([bash, "-n", str(path)], capture_output=True, text=True, check=False)
            check(result.returncode == 0, f"shell syntax failed for {path.name}: {result.stderr.strip()}")


def validate_no_placeholders() -> None:
    pattern = re.compile(r"\b(?:TODO|TBD|FIXME|XXX)\b|Rest of your code|Previous code")
    ignored_suffixes = {".pdf", ".png", ".csv"}
    ignored_names = {"main.log", "main.aux", "main.bbl", "main.bcf", "main.blg", "main.fls", "main.out", "main.run.xml", "main.toc", "main.fdb_latexmk"}
    # Same git-sourced path set as the manifest: the ban applies to content the
    # repository actually ships, not to a contributor's untracked scratch files or to
    # whatever a virtualenv happens to vendor.
    for path in manifest_paths():
        if path == Path(__file__).resolve() or path.suffix in ignored_suffixes or path.name in ignored_names:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        match = pattern.search(text)
        check(match is None, f"placeholder token {match.group(0)!r} in {path.relative_to(ROOT)}" if match else "")


def shutil_which(command: str) -> str | None:
    for directory in os.environ.get("PATH", "").split(os.pathsep):
        candidate = Path(directory) / command
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


def main() -> int:
    required_files()
    validate_schemas_and_manifests()
    validate_repository_manifest()
    validate_site()
    validate_paper()
    validate_proposal_boundary()
    validate_simulations()
    validate_cargo_and_sources()
    validate_finding_code_vocabulary()
    validate_shell_scripts()
    validate_no_placeholders()
    if ERRORS:
        print(f"repository validation failed with {len(ERRORS)} error(s):", file=sys.stderr)
        for error in ERRORS:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print(f"repository validation passed ({CHECKS} checks)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
