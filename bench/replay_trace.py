#!/usr/bin/env python3
"""Validate and summarize a solver trace without trusting its recorded geometry."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import sys
from typing import Any

from check_solution import parse_solution, validate_solution


def _bad_constant(token: str) -> None:
    raise ValueError(f"invalid JSON number {token}")


def _finite(value: Any, name: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{name} must be a finite number")
    try:
        number = float(value)
    except OverflowError as exc:
        raise ValueError(f"{name} must be finite") from exc
    if not math.isfinite(number) or (positive and number <= 0):
        raise ValueError(f"{name} must be {'positive and ' if positive else ''}finite")
    return number


def _integer(value: Any, name: str) -> int:
    if type(value) is not int or value < 0:
        raise ValueError(f"{name} must be a nonnegative integer")
    return value


def _snapshot_solution(row: dict[str, Any], radii: list[float]) -> dict[str, Any]:
    points = row.get("points")
    if not isinstance(points, list) or len(points) != len(radii):
        raise ValueError("snapshot point count does not match instance")
    circles = []
    for point in points:
        if not isinstance(point, list) or len(point) != 2:
            raise ValueError("each snapshot point must be [x, y]")
        circles.append((radii[len(circles)], _finite(point[0], "point x"), _finite(point[1], "point y")))
    return {"radius": _finite(row.get("radius"), "snapshot radius", positive=True), "circles": circles}


def _check_export(row: dict[str, Any], radii: list[float], internal: dict[str, Any]) -> dict[str, Any]:
    output = row.get("output")
    if not isinstance(output, str):
        raise ValueError("snapshot output must be contest-format text")
    parsed = parse_solution(output, radii)
    exported = validate_solution(radii, parsed)
    if not exported["feasible"]:
        raise ValueError("snapshot output is not strictly feasible")
    scaled_radius = internal["radius"] * 1e4
    if not math.isfinite(scaled_radius):
        raise ValueError("snapshot radius overflows four-decimal formatting")
    expected_radius = math.ceil(scaled_radius) / 1e4
    if abs(parsed["radius"] - expected_radius) > 1e-12:
        raise ValueError("snapshot output radius is not the internal radius rounded upward to four decimals")
    for i, (_, x, y) in enumerate(internal["circles"]):
        _, ox, oy = parsed["circles"][i]
        expected_x, expected_y = float(format(x, ".4f")), float(format(y, ".4f"))
        if abs(ox - x) > 5e-5 + 1e-12 or abs(oy - y) > 5e-5 + 1e-12 or ox != expected_x or oy != expected_y:
            raise ValueError(f"snapshot output coordinates do not match point {i} to four decimals")
    return parsed


def replay_trace(path: str | Path, expected_output: str | None = None) -> dict[str, Any]:
    """Validate all trace events and return a compact replay summary."""
    rows = []
    try:
        with Path(path).open(encoding="utf-8") as stream:
            for number, line in enumerate(stream, 1):
                try:
                    row = json.loads(line, parse_constant=_bad_constant)
                except (json.JSONDecodeError, ValueError) as exc:
                    raise ValueError(f"line {number}: invalid JSON: {exc}") from exc
                if not isinstance(row, dict):
                    raise ValueError(f"line {number}: row must be an object")
                rows.append(row)
    except OSError as exc:
        raise ValueError(str(exc)) from exc
    if len(rows) < 3:
        raise ValueError("trace must contain header, initial, and final rows")
    header = rows[0]
    if header.get("type") != "header" or type(header.get("schema_version")) is not int or header["schema_version"] != 1:
        raise ValueError("first row must be a schema version 1 header")
    if header.get("algorithm") not in ("a", "b") or not isinstance(header.get("run_id"), str) or not header["run_id"]:
        raise ValueError("header algorithm or run_id is invalid")
    if header["algorithm"] == "b" and (not isinstance(header.get("variant"), str) or not header["variant"]):
        raise ValueError("B trace header variant is required")
    seed = _integer(header.get("seed"), "header seed")
    if seed > 2**64 - 1:
        raise ValueError("header seed exceeds u64")
    options = header.get("options")
    if not isinstance(options, dict):
        raise ValueError("header options must be an object")
    if options.get("time_limit_seconds") is not None:
        if _finite(options["time_limit_seconds"], "header time_limit_seconds") < 0:
            raise ValueError("header time_limit_seconds must be nonnegative")
    for option_name in ("evaluation_limit", "iteration_limit", "restart_limit"):
        if _integer(options.get(option_name), f"header {option_name}") > 2**64 - 1:
            raise ValueError(f"header {option_name} exceeds u64")
    instance = header.get("instance")
    if not isinstance(instance, dict) or not isinstance(instance.get("radii"), list) or not instance["radii"]:
        raise ValueError("header instance.radii must be a nonempty list")
    radii = [_finite(r, f"instance radius {i}", positive=True) for i, r in enumerate(instance["radii"])]
    last_time, last_eval, last_iterations, last_restarts, previous_incumbent = -1.0, -1, -1, -1, math.inf
    kinds = []
    last_output = None
    last_iterations = last_restarts = 0
    exported_radius = None
    for index, row in enumerate(rows[1:]):
        label = f"snapshot {index}"
        if row.get("type") != "snapshot" or type(row.get("schema_version")) is not int or row["schema_version"] != 1:
            raise ValueError(f"{label} has unsupported type or schema")
        if row.get("run_id") != header["run_id"]:
            raise ValueError(f"{label} run_id differs from header")
        kind = row.get("kind")
        if kind not in {"initial", "incumbent", "final"}:
            raise ValueError(f"{label} has invalid kind")
        if row.get("sequence") != index or type(row.get("sequence")) is not int:
            raise ValueError(f"{label} sequence must be contiguous from zero")
        if index == 0 and kind != "initial":
            raise ValueError("first snapshot must be initial")
        if index > 0 and kind == "initial":
            raise ValueError("initial snapshot may appear only once")
        if kind == "final" and index != len(rows) - 2:
            raise ValueError("final snapshot must be last; extra rows follow final")
        if index == len(rows) - 2 and kind != "final":
            raise ValueError("last snapshot must be final")
        elapsed = _finite(row.get("elapsed_seconds"), f"{label} elapsed_seconds")
        if elapsed < 0:
            raise ValueError(f"{label} elapsed_seconds must be nonnegative")
        evaluations = _integer(row.get("evaluations"), f"{label} evaluations")
        iterations = _integer(row.get("iterations"), f"{label} iterations")
        restarts = _integer(row.get("restarts"), f"{label} restarts")
        if (elapsed < last_time or evaluations < last_eval or iterations < last_iterations
                or restarts < last_restarts):
            raise ValueError(f"{label} time and work counters must be monotone")
        if index and kind == "incumbent" and elapsed - last_time < 0.1 - 1e-12:
            raise ValueError(f"{label} is less than 0.1 seconds after previous snapshot")
        internal = _snapshot_solution(row, radii)
        metrics = validate_solution(radii, internal)
        if not metrics["feasible"]:
            raise ValueError(f"{label} geometry is infeasible")
        recorded = _finite(row.get("max_violation"), f"{label} max_violation")
        if recorded < 0 or abs(recorded - metrics["max_violation"]) > max(1e-10, metrics["internal_tolerance"]):
            raise ValueError(f"{label} max_violation does not match recomputed geometry")
        if row.get("feasible") is not True:
            raise ValueError(f"{label} must record feasible=true")
        exported = _check_export(row, radii, internal)
        if internal["radius"] > previous_incumbent + 1e-12:
            raise ValueError(f"{label} incumbent radius increased")
        previous_incumbent = internal["radius"]
        last_time, last_eval, last_iterations, last_restarts = elapsed, evaluations, iterations, restarts
        exported_radius = exported["radius"]
        kinds.append(kind)
        last_output = row["output"]
    if expected_output is not None and last_output != expected_output:
        raise ValueError("final trace output does not exactly match expected stdout")
    return {"valid": True, "run_id": header["run_id"], "algorithm": header["algorithm"],
            "variant": header.get("variant"),
            "seed": seed, "options": options,
            "snapshots": len(kinds), "incumbents": kinds.count("incumbent"),
            "final_radius": _finite(rows[-1].get("radius"), "final radius", positive=True),
            "exported_radius": exported_radius, "evaluations": last_eval,
            "iterations": last_iterations, "restarts": last_restarts,
            "elapsed_seconds": last_time}


def _close(left: Any, right: Any) -> bool:
    return (not isinstance(left, bool) and isinstance(left, (int, float)) and math.isfinite(left)
            and not isinstance(right, bool) and isinstance(right, (int, float)) and math.isfinite(right)
            and math.isclose(left, right, rel_tol=1e-12, abs_tol=1e-12))


def validate_run_summary(replay_result: dict, summary: dict) -> None:
    """Link a validated trace to a timed or fixed-work solver invocation."""
    if not isinstance(replay_result, dict) or replay_result.get("valid") is not True:
        raise ValueError("trace replay result is invalid")
    if not isinstance(summary, dict) or type(summary.get("schema_version")) is not int or summary["schema_version"] != 1:
        raise ValueError("solver summary has unsupported schema")
    if replay_result.get("algorithm") != summary.get("algorithm"):
        raise ValueError("trace algorithm differs from solver summary")
    if replay_result.get("variant") != summary.get("variant"):
        raise ValueError("trace variant differs from solver summary")
    if type(summary.get("seed")) is not int or replay_result.get("seed") != summary["seed"]:
        raise ValueError("trace seed differs from solver summary")
    options = replay_result.get("options")
    if not isinstance(options, dict):
        raise ValueError("trace options are missing")
    for name in ("time_limit_seconds", "evaluation_limit", "iteration_limit", "restart_limit"):
        if name == "time_limit_seconds":
            if summary.get(name) is not None and _finite(summary[name],name) < 0:
                raise ValueError("time limit cannot be negative")
        elif type(summary.get(name)) is not int or summary[name] < 0:
            raise ValueError(f"solver summary {name} must be a nonnegative integer")
        if options.get(name) != summary.get(name):
            raise ValueError(f"trace option {name} differs from solver summary")
    for name in ("evaluations", "iterations", "restarts"):
        if type(summary.get(name)) is not int or summary[name] < 0 or replay_result.get(name) != summary[name]:
            raise ValueError(f"trace final {name} differs from solver summary")
    if not _close(replay_result.get("final_radius"), summary.get("radius")):
        raise ValueError("trace final radius differs from solver summary")
    if not _close(replay_result.get("exported_radius"), summary.get("output_radius")):
        raise ValueError("trace exported radius differs from solver summary")
    elapsed = _finite(replay_result.get("elapsed_seconds"),"trace elapsed")
    total = _finite(summary.get("elapsed_seconds"),"solver elapsed")
    if elapsed < 0 or total < 0 or elapsed > total + 1e-9:
        raise ValueError("trace elapsed time exceeds solver summary elapsed time")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path)
    parser.add_argument("--solution", type=Path, help="expected final solution stdout")
    args = parser.parse_args(argv)
    try:
        expected = args.solution.read_text(encoding="utf-8") if args.solution else None
        report = replay_trace(args.trace, expected)
    except (OSError, ValueError) as exc:
        print(json.dumps({"valid": False, "error": str(exc)}))
        return 2
    print(json.dumps(report, sort_keys=True, allow_nan=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
