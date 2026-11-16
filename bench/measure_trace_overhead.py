#!/usr/bin/env python3
"""Measure opt-in trace overhead with paired fixed-work version A runs."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import math
import json
from pathlib import Path
from typing import Any
import platform
import statistics
import subprocess
import sys
import time

from check_solution import parse_instance, parse_solution, validate_solution
from legacy import sha256
from legacy import git_revision
from replay_trace import replay_trace, validate_run_summary
from run_baseline import cpu_model

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target/release/packing-cli"
COUNTERS = ("evaluations", "iterations", "restarts", "initialization_fallbacks", "numerical_failures", "termination")


def _mode_status_valid(mode: dict, seed: Any, evaluations: int | None) -> bool:
    summary = mode.get("summary") if isinstance(mode.get("summary"), dict) else {}
    elapsed = mode.get("elapsed_seconds")
    requested = evaluations if evaluations is not None else summary.get("evaluations")
    radius, output_radius, solver_elapsed = (summary.get("radius"), summary.get("output_radius"),
                                             summary.get("elapsed_seconds"))
    finite_positive = lambda x: (not isinstance(x, bool) and isinstance(x, (int, float))
                                 and math.isfinite(x) and x > 0)
    return (type(mode.get("returncode")) is int and mode["returncode"] == 0
            and isinstance(mode.get("stdout"), str)
            and not isinstance(elapsed, bool) and isinstance(elapsed, (int, float))
            and math.isfinite(elapsed) and elapsed > 0
            and all(type(summary.get(counter)) is int and summary[counter] >= 0 for counter in COUNTERS[:-1])
            and type(summary.get("schema_version")) is int and summary["schema_version"] == 1
            and summary.get("termination") == "evaluation_limit"
            and type(seed) is int and seed >= 0 and type(summary.get("seed")) is int
            and summary["seed"] == seed and summary.get("algorithm") == "a"
            and summary.get("time_limit_seconds") is None
            and type(summary.get("evaluation_limit")) is int and summary["evaluation_limit"] == requested
            and type(summary.get("evaluations")) is int and summary["evaluations"] == requested
            and type(summary.get("iteration_limit")) is int and summary["iteration_limit"] >= 0
            and type(summary.get("restart_limit")) is int and summary["restart_limit"] >= 0
            and finite_positive(radius) and finite_positive(output_radius)
            and not isinstance(solver_elapsed, bool) and isinstance(solver_elapsed, (int, float))
            and math.isfinite(solver_elapsed) and solver_elapsed >= 0)


def analyze_pairs(rows: list[dict], expected_instances: set[str], repetitions: int,
                  evaluations: int | None = None) -> dict:
    """Audit paired rows; useful for checking archived and synthetic cohorts."""
    expected = {(name, rep) for name in expected_instances for rep in range(repetitions)}
    seen = [(row.get("instance"), row.get("repetition")) if isinstance(row, dict) else (None, None)
            for row in rows]
    complete = len(seen) == len(expected) and len(set(seen)) == len(expected) and set(seen) == expected
    changed_outputs, changed_counts, invalid, bad_status = [], [], [], []
    for row in rows:
        if not isinstance(row, dict):
            bad_status.append(((None, None), "malformed pair"))
            continue
        key = (row.get("instance"), row.get("repetition"))
        plain = row.get("plain") if isinstance(row.get("plain"), dict) else {}
        traced = row.get("traced") if isinstance(row.get("traced"), dict) else {}
        if plain.get("stdout") != traced.get("stdout"):
            changed_outputs.append(key)
        plain_summary = plain.get("summary") if isinstance(plain.get("summary"), dict) else {}
        traced_summary = traced.get("summary") if isinstance(traced.get("summary"), dict) else {}
        if any(plain_summary.get(counter) != traced_summary.get(counter) for counter in COUNTERS):
            changed_counts.append(key)
        pv = row.get("plain_validation") if isinstance(row.get("plain_validation"), dict) else {}
        tv = row.get("traced_validation") if isinstance(row.get("traced_validation"), dict) else {}
        replay = row.get("trace_replay") if isinstance(row.get("trace_replay"), dict) else {}
        if (pv.get("feasible") is not True or tv.get("feasible") is not True or replay.get("valid") is not True):
            invalid.append(key)
        expected_seed = 5 + row["repetition"] if type(row.get("repetition")) is int else None
        row_seed = row.get("seed")
        for mode in (plain, traced):
            replay = row.get("trace_replay") if isinstance(row.get("trace_replay"), dict) else {}
            traced_summary = traced.get("summary") if isinstance(traced.get("summary"), dict) else {}
            if (not _mode_status_valid(mode, expected_seed, evaluations)
                    or type(row_seed) is not int or row_seed != expected_seed
                    or (mode is traced and _summary_link_error(replay, traced_summary))):
                bad_status.append((key, mode.get("mode", "run")))
    return {"complete": complete, "changed_outputs": changed_outputs,
            "changed_counters": changed_counts, "invalid_pairs": invalid,
            "bad_status": bad_status,
            "valid": complete and not changed_outputs and not changed_counts and not invalid and not bad_status}


def _summary_link_error(replay: dict, summary: dict) -> bool:
    try:
        validate_run_summary(replay, summary)
    except ValueError:
        return True
    return False


def _summary(stderr: str) -> dict:
    try:
        value = json.loads(stderr.strip(), parse_constant=lambda x: (_ for _ in ()).throw(ValueError(x)))
    except (json.JSONDecodeError, ValueError) as exc:
        raise ValueError(f"solver summary is not valid JSON: {exc}") from exc
    if (not isinstance(value, dict) or any(key not in value for key in COUNTERS)
            or type(value.get("schema_version")) is not int or value.get("schema_version") != 1):
        raise ValueError("solver summary is missing required fields")
    return value


def _run(binary: Path, input_text: str, seed: int, evaluations: int, trace: Path | None) -> dict:
    command = [str(binary), "solve", "-", "--algorithm", "a", "--seed", str(seed),
               "--fixed-work", "--evaluations", str(evaluations)]
    if trace is not None:
        command.extend(("--trace", str(trace)))
    started = time.perf_counter()
    try:
        proc = subprocess.run(command, input=input_text, text=True, capture_output=True,
                              timeout=max(30.0, evaluations * 0.001))
        returncode, stdout, stderr = proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired as exc:
        def decode(value: str | bytes | None) -> str:
            return value.decode(errors="replace") if isinstance(value, bytes) else (value or "")
        stdout, stderr = decode(exc.stdout), decode(exc.stderr)
        elapsed = time.perf_counter() - started
        try:
            summary = _summary(stderr)
            summary_error = None
        except ValueError as summary_exc:
            summary, summary_error = {}, str(summary_exc)
        return {"mode": "traced" if trace else "plain", "command": command,
                "elapsed_seconds": elapsed, "returncode": None, "stdout": stdout,
                "stderr": stderr, "summary": summary, "summary_error": summary_error,
                "timeout": True}
    except OSError as exc:
        return {"mode": "traced" if trace else "plain", "command": command,
                "elapsed_seconds": time.perf_counter() - started, "returncode": None,
                "stdout": "", "stderr": str(exc), "summary": {}, "summary_error": str(exc)}
    elapsed = time.perf_counter() - started
    try:
        summary = _summary(stderr)
        summary_error = None
    except ValueError as exc:
        summary, summary_error = {}, str(exc)
    return {"mode": "traced" if trace else "plain", "command": command,
            "elapsed_seconds": elapsed, "returncode": returncode, "stdout": stdout,
            "stderr": stderr, "summary": summary, "summary_error": summary_error, "timeout": False}


def _validated(run: dict, radii: list[float]) -> dict:
    try:
        solution = parse_solution(run["stdout"], radii)
        report = validate_solution(radii, solution)
        return report
    except (KeyError, ValueError) as exc:
        return {"feasible": False, "error": str(exc)}


def _source_hashes() -> dict[str, str]:
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", *sorted((ROOT / "crates").rglob("*.rs")),
             *sorted((ROOT / "crates").rglob("Cargo.toml"))]
    return {str(path.relative_to(ROOT)): sha256(path) for path in paths}


def _save_report(path: Path, report: dict) -> None:
    with path.open("w", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2, allow_nan=False)
        stream.write("\n")


def _refresh_report(report: dict, entries: list[dict], repetitions: int, evaluations: int) -> None:
    rows = report["runs"]
    audit = analyze_pairs(rows, {e["path"] for e in entries}, repetitions, evaluations)
    grouped: dict[str, list[float]] = {}
    for row in rows:
        ratio = row.get("elapsed_ratio")
        if (row.get("failure") is None and isinstance(ratio, (int, float))
                and _mode_status_valid(row.get("plain", {}), row.get("seed"), evaluations)
                and _mode_status_valid(row.get("traced", {}), row.get("seed"), evaluations)
                and row.get("plain_validation", {}).get("feasible") is True
                and row.get("traced_validation", {}).get("feasible") is True
                and row.get("trace_replay", {}).get("valid") is True
                and math.isfinite(ratio) and ratio > 0):
            grouped.setdefault(row["instance"], []).append(ratio)
    medians = {name: statistics.median(values) for name, values in grouped.items()}
    increase = statistics.median(medians.values()) - 1.0 if medians else None
    full = len(entries) == 50 and repetitions == 3 and len(rows) == 150
    report["instance_median_ratios"] = medians
    report["pair_audit"] = audit
    report["protocol"].update({"instances": len(entries), "repetitions": repetitions,
                               "evaluations": evaluations, "complete": full,
            "qualifies": bool(full and audit["valid"] and all(r.get("failure") is None for r in rows)
                              and not report["warmup_failures"]
                              and increase is not None and increase < 0.05),
                               "median_runtime_increase": increase})


def collect(output: Path, binary: Path = DEFAULT_BINARY, evaluations: int = 50000,
            repetitions: int = 3, limit: int | None = None) -> dict:
    output = output.resolve()
    binary = binary.resolve()
    trace_dir = output.with_name(output.stem + ".traces")
    if output.exists() or trace_dir.exists():
        raise ValueError("refusing to overwrite report or trace directory")
    if not binary.is_file() or evaluations <= 0 or repetitions <= 0 or (limit is not None and limit <= 0):
        raise ValueError("binary must exist; evaluations, repetitions, and limit must be positive")
    manifest_path = ROOT / "bench/data/manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    entries = [e for e in manifest["instances"] if e["path"].startswith("development/")]
    if limit is not None:
        entries = entries[:limit]
    if not entries:
        raise ValueError("development instance cohort is empty")
    for entry in entries:
        if sha256(manifest_path.parent / entry["path"]) != entry["sha256"]:
            raise ValueError(f"frozen input hash mismatch: {entry['path']}")
    output.parent.mkdir(parents=True, exist_ok=True)
    trace_dir.mkdir()
    report = {"schema_version": 1, "started_utc": datetime.now(timezone.utc).isoformat(),
              "protocol": {"expected_instances": 50, "expected_repetitions": 3,
                           "seed_rule": "5 + repetition",
                           "pair_order": "alternating by instance index plus repetition",
                           "target_runtime_increase": 0.05, "complete": False, "qualifies": False,
                           "median_runtime_increase": None},
              "environment": {"platform": platform.platform(), "cpu": cpu_model(),
                              "python": platform.python_version(), "rustc": subprocess.run(
                                  ["rustc", "--version"], text=True, capture_output=True, check=True).stdout.strip(),
                              "cargo": subprocess.run(["cargo", "--version"], text=True,
                                                      capture_output=True, check=True).stdout.strip()},
              "binary": {"path": str(binary), "sha256": sha256(binary)},
              "root_commit": git_revision(ROOT), "source_sha256": _source_hashes(),
              "harness_sha256": sha256(Path(__file__)),
              "checker_sha256": sha256(ROOT / "bench/check_solution.py"),
              "replay_sha256": sha256(ROOT / "bench/replay_trace.py"),
              "manifest_sha256": sha256(manifest_path), "trace_directory": str(trace_dir),
              "instance_median_ratios": {}, "runs": [], "warmup_failures": []}
    _save_report(output, report)
    # One unmeasured warm-up per mode on the first frozen instance.
    first_text = (manifest_path.parent / entries[0]["path"]).read_text(encoding="utf-8")
    first_radii = parse_instance(first_text)
    warm_plain = _run(binary, first_text, 5, evaluations, None)
    warm_trace = trace_dir / "warmup.jsonl"
    warm_traced = _run(binary, first_text, 5, evaluations, warm_trace)
    warm_errors = []
    if not _validated(warm_plain, first_radii).get("feasible"):
        warm_errors.append("plain warm-up output invalid")
    if not _validated(warm_traced, first_radii).get("feasible"):
        warm_errors.append("traced warm-up output invalid")
    try:
        warm_replay = replay_trace(warm_trace, warm_traced["stdout"])
        validate_run_summary(warm_replay, warm_traced["summary"])
    except (OSError, ValueError) as exc:
        warm_errors.append(f"traced warm-up trace invalid: {exc}")
    if not _mode_status_valid(warm_plain, 5, evaluations) or not _mode_status_valid(warm_traced, 5, evaluations):
        warm_errors.append("warm-up solver summary or status is invalid")
    if warm_plain["stdout"] != warm_traced["stdout"] or any(
            warm_plain["summary"].get(c) != warm_traced["summary"].get(c) for c in COUNTERS):
        warm_errors.append("warm-up output or work counters changed under tracing")
    report["warmup_failures"] = warm_errors
    warm_trace.unlink(missing_ok=True)
    _save_report(output, report)

    for entry_index, entry in enumerate(entries):
        name = entry["path"]
        text = (manifest_path.parent / name).read_text(encoding="utf-8")
        radii = parse_instance(text)
        for repetition in range(repetitions):
            seed = 5 + repetition
            trace_path = trace_dir / f"{Path(name).stem}-rep{repetition}.jsonl"
            order = ("traced", "plain") if (entry_index + repetition) % 2 else ("plain", "traced")
            pair = {}
            for mode in order:
                pair[mode] = _run(binary, text, seed, evaluations, trace_path if mode == "traced" else None)
            plain, traced = pair["plain"], pair["traced"]
            plain_validation, traced_validation = _validated(plain, radii), _validated(traced, radii)
            try:
                trace_summary = replay_trace(trace_path, traced["stdout"])
            except (OSError, ValueError) as exc:
                trace_summary = {"valid": False, "error": str(exc)}
            ratio = (traced["elapsed_seconds"] / plain["elapsed_seconds"]
                     if plain["elapsed_seconds"] > 0 else None)
            failures = []
            if plain["returncode"] != 0 or traced["returncode"] != 0:
                failures.append("nonzero or missing solver return code")
            if not plain_validation.get("feasible") or not traced_validation.get("feasible"):
                failures.append("invalid solver output")
            if plain["stdout"] != traced["stdout"]:
                failures.append("fixed-work stdout changed under tracing")
            if any(plain["summary"].get(c) != traced["summary"].get(c) for c in COUNTERS):
                failures.append("work counters changed under tracing")
            if not trace_summary.get("valid"):
                failures.append("trace replay failed")
            report["runs"].append({"instance": name, "input_sha256": entry["sha256"],
                                   "repetition": repetition, "seed": seed, "order": order,
                                   "plain": plain, "traced": traced,
                                   "trace_path": str(trace_path.relative_to(output.parent)),
                                   "trace_replay": trace_summary, "plain_validation": plain_validation,
                                   "traced_validation": traced_validation, "elapsed_ratio": ratio,
                                   "failure": failures or None})
            _refresh_report(report, entries, repetitions, evaluations)
            _save_report(output, report)
            if len(report["runs"]) % 25 == 0:
                print(f"{len(report['runs'])} paired runs archived", flush=True)
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--evaluations", type=int, default=50000)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--limit", type=int, help="smoke only; limits development inputs")
    args = parser.parse_args(argv)
    try:
        report = collect(args.output, args.binary, args.evaluations, args.repetitions, args.limit)
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        print(json.dumps({"error": str(exc)}), file=sys.stderr)
        return 2
    print(json.dumps(report["protocol"], sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
