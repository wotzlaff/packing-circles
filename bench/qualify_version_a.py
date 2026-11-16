#!/usr/bin/env python3
"""Run and qualify the seeded Rust version A against the frozen legacy archive."""

import argparse
from collections import defaultdict
from datetime import datetime, timezone
import json
import math
from pathlib import Path
import platform
import statistics
import subprocess
import time

from check_solution import parse_instance
from legacy import git_revision, sha256
from run_baseline import check_output, cpu_model

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target/release/packing-cli"
LEGACY = ROOT / "bench/results/contest-development.jsonl"
DEFAULT_BUDGETS = [0.1, 0.5, 1.95]
SEEDS = [5, 6, 7, 8, 9]


def _json_safe(value):
    """Replace non-finite parsed JSON numbers so a corrupt summary stays archivable."""
    if isinstance(value, float) and not math.isfinite(value):
        return None
    if isinstance(value, list):
        return [_json_safe(item) for item in value]
    if isinstance(value, dict):
        return {key: _json_safe(item) for key, item in value.items()}
    return value


def read_jsonl(path):
    rows = []
    with Path(path).open(encoding="utf-8") as stream:
        for number, line in enumerate(stream, 1):
            try:
                value = json.loads(line, parse_constant=lambda token: (_ for _ in ()).throw(ValueError(token)))
                if not isinstance(value, dict):
                    raise ValueError("row is not an object")
                rows.append(value)
            except (json.JSONDecodeError, ValueError) as exc:
                rows.append({"_row_error": f"line {number}: {exc}"})
    return rows


def run_solver(binary, text, budget, seed):
    command = [str(binary), "solve", "-", "--algorithm", "a", "--seed", str(seed),
               "--time-limit", str(budget)]
    started = time.perf_counter()
    try:
        result = subprocess.run(command, input=text, text=True, capture_output=True,
                                timeout=max(2.0, budget * 3 + 1))
        return {"command": command, "elapsed_seconds": time.perf_counter() - started,
                "returncode": result.returncode, "timeout": False,
                "stdout": result.stdout, "stderr": result.stderr}
    except subprocess.TimeoutExpired as exc:
        def decoded(value):
            return value.decode(errors="replace") if isinstance(value, bytes) else (value or "")
        return {"command": command, "elapsed_seconds": time.perf_counter() - started,
                "returncode": None, "timeout": True,
                "stdout": decoded(exc.stdout), "stderr": decoded(exc.stderr)}


def _summary_from_stderr(stderr):
    try:
        data = json.loads(stderr.strip(), parse_constant=lambda token: (_ for _ in ()).throw(ValueError(token)))
        error = _summary_error(data)
        if error:
            return None, error
        data = _json_safe(data)
        data["radius_internal"] = data["radius"]
        return data, None
    except (json.JSONDecodeError, ValueError) as exc:
        return None, f"invalid summary JSON: {exc}"


def _summary_error(data):
    required = ("schema_version", "algorithm", "seed", "evaluations", "iterations", "restarts",
                "elapsed_seconds", "termination", "radius", "output_radius", "feasible", "output_feasible",
                "time_limit_seconds")
    if not isinstance(data, dict) or any(key not in data for key in required):
        return "summary JSON missing required fields"
    if type(data["schema_version"]) is not int or data["schema_version"] != 1 or data["algorithm"] != "a":
        return "summary JSON has unsupported schema or algorithm"
    for key in ("seed", "evaluations", "iterations", "restarts"):
        if isinstance(data[key], bool) or not isinstance(data[key], int) or data[key] < 0:
            return f"summary {key} must be a nonnegative integer"
    for key in ("elapsed_seconds", "radius", "output_radius"):
        value = data[key]
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
            return f"summary {key} must be finite"
        if value < 0 or (key != "elapsed_seconds" and value == 0):
            return f"summary {key} is out of range"
    limit = data["time_limit_seconds"]
    if isinstance(limit, bool) or not isinstance(limit, (int, float)) or not math.isfinite(limit) or limit <= 0:
        return "summary time_limit_seconds must be positive and finite"
    if data["termination"] not in {"time_limit", "evaluation_limit", "iteration_limit",
                                    "restart_limit", "cancelled", "trivial"}:
        return "summary termination is unknown"
    if not isinstance(data["feasible"], bool) or not isinstance(data["output_feasible"], bool):
        return "summary feasibility flags must be booleans"
    return None


def _load_expected(legacy_path):
    rows = read_jsonl(legacy_path)
    metadata_path = Path(legacy_path).with_suffix(".metadata.json")
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    protocol = metadata.get("protocol", {})
    if metadata.get("manifest_sha256") != sha256(ROOT / "bench/data/manifest.json"):
        raise ValueError("legacy archive manifest hash does not match the current frozen manifest")
    if protocol.get("input_count") != 50 or protocol.get("budgets_seconds") != DEFAULT_BUDGETS or protocol.get("repetitions") != 5:
        raise ValueError("legacy metadata does not describe the full frozen 50×3×5 protocol")
    expected_hashes = {}
    manifest_path = ROOT / "bench/data/manifest.json"
    manifest = json.loads(manifest_path.read_text())
    for entry in manifest["instances"]:
        if entry["path"].startswith("development/"):
            expected_hashes[entry["path"]] = entry["sha256"]
    expected = {(name, budget, rep) for name in expected_hashes
                for budget in DEFAULT_BUDGETS for rep in range(5)}
    seen = set()
    for row in rows:
        if row.get("variant") != "contest":
            raise ValueError("legacy archive contains a non-contest variant")
        key = (row.get("instance"), row.get("budget_seconds"), row.get("repetition"))
        if key in seen:
            raise ValueError(f"duplicate legacy combination: {key}")
        seen.add(key)
        if key not in expected or row.get("input_sha256") != expected_hashes.get(key[0]):
            raise ValueError(f"unexpected legacy combination or input hash: {key}")
        if not row.get("successful") or row.get("timeout") or row.get("returncode") != 0 or not row.get("validation", {}).get("feasible"):
            raise ValueError("legacy archive has bad status; refusing quality comparison")
        radius = row.get("validation", {}).get("enclosing_radius")
        if not isinstance(radius, (int, float)) or not math.isfinite(radius) or radius <= 0:
            raise ValueError("legacy archive contains invalid radius")
    if seen != expected:
        raise ValueError(f"legacy archive incomplete: {len(seen)} of {len(expected)} expected rows")
    return rows, metadata, sha256(Path(legacy_path))


def _valid_rust(row, legacy_row=None):
    summary = row.get("summary")
    validation = row.get("validation", {})
    if not isinstance(validation, dict) or not isinstance(summary, dict):
        return False
    if _summary_error(summary):
        return False
    if (type(row.get("schema_version")) is not int or row.get("schema_version") != 1
            or row.get("variant") != "rust-a"
            or row.get("returncode") != 0 or row.get("timeout") is not False
            or row.get("overrun") is not False or summary.get("feasible") is not True
            or summary.get("output_feasible") is not True or validation.get("feasible") is not True):
        return False
    budget = row.get("budget_seconds")
    repetition = row.get("repetition")
    elapsed = row.get("elapsed_seconds")
    if (not isinstance(row.get("instance"), str)
            or isinstance(budget, bool) or not isinstance(budget, (int, float))
            or not math.isfinite(budget) or budget <= 0
            or isinstance(elapsed, bool) or not isinstance(elapsed, (int, float))
            or not math.isfinite(elapsed) or elapsed < 0
            or elapsed > budget + max(0.02, 0.1 * budget)
            or summary.get("time_limit_seconds") != budget
            or isinstance(repetition, bool) or not isinstance(repetition, int)
            or repetition < 0 or type(row.get("seed")) is not int or row.get("seed") != 5 + repetition
            or summary.get("seed") != row.get("seed")):
        return False
    if legacy_row is not None and row.get("input_sha256") != legacy_row.get("input_sha256"):
        return False
    output_radius = validation.get("enclosing_radius")
    if (isinstance(output_radius, bool) or not isinstance(output_radius, (int, float))
            or not math.isfinite(output_radius) or output_radius <= 0):
        return False
    return math.isclose(output_radius, summary["output_radius"], rel_tol=1e-12, abs_tol=1e-12)


def analyze(rows, legacy_rows):
    """Return raw completeness and paired per-instance median-radius gate results."""
    expected = {(r["instance"], r["budget_seconds"], r["repetition"]) for r in legacy_rows}
    keys = [(r.get("instance"), r.get("budget_seconds"), r.get("repetition")) for r in rows]
    complete = len(rows) == len(expected) and len(set(keys)) == len(expected) and set(keys) == expected
    legacy_by = defaultdict(list)
    for row in legacy_rows:
        legacy_by[(row["instance"], row["budget_seconds"])].append(row["validation"]["enclosing_radius"])
    legacy_index = {(r["instance"], r["budget_seconds"], r["repetition"]): r for r in legacy_rows}
    rust_by = defaultdict(list)
    for row in rows:
        key = (row.get("instance"), row.get("budget_seconds"), row.get("repetition"))
        if _valid_rust(row, legacy_index.get(key)):
            rust_by[(row["instance"], row["budget_seconds"])].append(row["validation"]["enclosing_radius"])
    budgets = {}
    all_valid = complete and all(_valid_rust(row, legacy_index.get((row.get("instance"),
                                                                  row.get("budget_seconds"),
                                                                  row.get("repetition")))) for row in rows)
    instances = sorted({r["instance"] for r in legacy_rows})
    for budget in sorted({r["budget_seconds"] for r in legacy_rows}):
        ratios = {}
        for instance in instances:
            rust_values = rust_by.get((instance, budget), [])
            legacy_values = legacy_by[(instance, budget)]
            if len(rust_values) != 5 or len(legacy_values) != 5:
                continue
            ratios[instance] = statistics.median(rust_values) / statistics.median(legacy_values)
        gm = math.exp(statistics.mean(math.log(value) for value in ratios.values())) if ratios else None
        worst = max(ratios.values()) if ratios else None
        budgets[str(budget)] = {"instances": len(ratios), "ratios": ratios,
                                "geometric_mean": gm, "worst": worst,
                                "qualified": bool(complete and all_valid and len(ratios) == len(instances)
                                                  and (budget != 1.95 or (gm <= 1.01 and worst <= 1.05)))}
    return {"complete": complete, "all_outputs_valid": all_valid,
            "status": "QUALIFIED" if budgets.get("1.95", {}).get("qualified") else "INCOMPLETE_OR_FAILED",
            "budgets": budgets, "runs": len(rows), "expected_runs": len(expected)}


def summarize(path, legacy_path=LEGACY):
    legacy_rows, metadata, legacy_digest = _load_expected(legacy_path)
    rows = read_jsonl(path)
    analysis = analyze(rows, legacy_rows)
    lines = ["# Version A qualification", "", f"Status: **{analysis['status']}**.",
             "", f"Runs: {analysis['runs']} / {analysis['expected_runs']}; all valid: {analysis['all_outputs_valid']}.",
             f"Legacy archive SHA-256: `{legacy_digest}`. This is a recorded baseline, not a contemporary rerun.",
             f"Baseline hardware: {metadata['cpu']} ({metadata['platform']}).",
             "Legacy contest implementation has sanitizer-confirmed undefined behavior; results are historical evidence.",
             "External machine load was uncontrolled during baseline collection.", "",
             "| Budget (s) | Instances | Geometric mean of instance median radius ratios | Worst instance ratio | Gate |",
             "| ---: | ---: | ---: | ---: | --- |"]
    for budget, data in analysis["budgets"].items():
        gm = "—" if data["geometric_mean"] is None else f"{data['geometric_mean']:.6f}"
        worst = "—" if data["worst"] is None else f"{data['worst']:.6f}"
        lines.append(f"| {budget} | {data['instances']} | {gm} | {worst} | {'PASS' if data['qualified'] else 'FAIL'} |")
    lines.extend(["", "## Radius ratios by size", "",
                  "Each size cell reports the geometric mean and worst per-instance median ratio.", "",
                  "| Circles | Budget (s) | Instances | Geometric mean | Worst instance |",
                  "| ---: | ---: | ---: | ---: | ---: |"])
    for budget, data in analysis["budgets"].items():
        for size in (10, 20, 30, 40, 50):
            sized = [ratio for instance, ratio in data["ratios"].items()
                     if int(Path(instance).stem.split("_")[0]) == size]
            size_gm = math.exp(statistics.mean(math.log(value) for value in sized)) if sized else None
            size_worst = max(sized) if sized else None
            lines.append(f"| {size} | {budget} | {len(sized)} | "
                         f"{'—' if size_gm is None else f'{size_gm:.6f}'} | "
                         f"{'—' if size_worst is None else f'{size_worst:.6f}'} |")
    lines.extend(["", "Ratios use enclosing radius and are computed by taking each instance's median across five repetitions first, then the geometric mean across instances.",
                  "The 1.95 s gate requires geometric mean ≤ 1.01, worst instance ≤ 1.05, complete exact coverage, 100% feasible outputs, and no deadline overruns.",
                  "Deadline allowance: budget + max(20 ms, 10% of budget).", ""])
    return "\n".join(lines), analysis


def _rust_sources():
    files = sorted([*(ROOT / "crates").rglob("*.rs"),
                    *(ROOT / "crates").rglob("Cargo.toml"), ROOT / "Cargo.toml", ROOT / "Cargo.lock"])
    return {str(path.relative_to(ROOT)): sha256(path) for path in files}


def execute(args):
    binary = args.binary.resolve()
    if not binary.is_file():
        raise ValueError(f"Rust binary not found: {binary}")
    out = args.output
    meta_path = out.with_suffix(".metadata.json")
    if out.exists() or meta_path.exists():
        raise ValueError("refusing to overwrite existing evidence; choose a new output path")
    _, legacy_metadata, legacy_digest = _load_expected(LEGACY)
    if legacy_metadata.get("cpu") != cpu_model():
        raise ValueError(f"legacy CPU {legacy_metadata.get('cpu')!r} differs from current CPU {cpu_model()!r}")
    if legacy_metadata.get("platform") != platform.platform():
        raise ValueError("legacy platform differs from current platform")
    manifest_path = ROOT / "bench/data/manifest.json"
    manifest = json.loads(manifest_path.read_text())
    entries = [entry for entry in manifest["instances"] if entry["path"].startswith("development/")]
    for entry in entries:
        if sha256(manifest_path.parent / entry["path"]) != entry["sha256"]:
            raise ValueError(f"input hash mismatch: {entry['path']}")
    if args.limit:
        entries = entries[:args.limit]
    out.parent.mkdir(parents=True, exist_ok=True)
    metadata = {"schema_version": 1, "started_utc": datetime.now(timezone.utc).isoformat(),
                "root_commit": git_revision(ROOT),
                "binary": str(binary), "binary_sha256": sha256(binary), "rust_sources": _rust_sources(),
                "runner_sha256": sha256(Path(__file__)),
                "checker_sha256": sha256(ROOT / "bench/check_solution.py"),
                "build_profile": "release" if binary.parent.name == "release" else "custom",
                "rustc": subprocess.run(["rustc", "--version"], text=True, capture_output=True).stdout.strip(),
                "cargo": subprocess.run(["cargo", "--version"], text=True, capture_output=True).stdout.strip(),
                "platform": platform.platform(), "cpu": cpu_model(), "architecture": platform.machine(),
                "legacy_archive_sha256": legacy_digest,
                "legacy_baseline": {"variant": legacy_metadata["build"]["variant"],
                                    "binary_sha256": legacy_metadata["build"]["binary_sha256"],
                                    "source_sha256": legacy_metadata["build"]["source_sha256"],
                                    "cpu": legacy_metadata["cpu"], "platform": legacy_metadata["platform"],
                                    "contemporary_rerun": False},
                "manifest_sha256": sha256(manifest_path),
                "protocol": {"split": "development", "input_count": len(entries), "budgets_seconds": args.budgets,
                             "repetitions": args.repetitions, "seeds": args.seeds,
                             "serial_execution": True, "external_machine_load_controlled": False,
                             "smoke_limit": args.limit}}
    with meta_path.open("x", encoding="utf-8") as stream:
        json.dump(metadata, stream, indent=2, allow_nan=False); stream.write("\n")
    total = len(entries) * len(args.budgets) * args.repetitions
    with out.open("x", encoding="utf-8") as stream:
        count = 0
        for budget in args.budgets:
            for entry in entries:
                text = (manifest_path.parent / entry["path"]).read_text()
                radii = parse_instance(text)
                for repetition in range(args.repetitions):
                    seed = args.seeds[repetition % len(args.seeds)]
                    result = run_solver(binary, text, budget, seed)
                    summary, summary_error = _summary_from_stderr(result["stderr"])
                    validation = check_output(result["stdout"], radii)
                    row = {"schema_version": 1, "variant": "rust-a", "instance": entry["path"],
                           "input_sha256": entry["sha256"], "circle_count": len(radii),
                           "budget_seconds": budget, "repetition": repetition, "seed": seed,
                           **result, "summary": summary, "summary_error": summary_error,
                           "evaluations": None if summary is None else summary.get("evaluations"),
                           "iterations": None if summary is None else summary.get("iterations"),
                           "restarts": None if summary is None else summary.get("restarts"),
                           "termination": None if summary is None else summary.get("termination"),
                           "radius_internal": None if summary is None else summary.get("radius_internal"),
                           "output_radius": None if summary is None else summary.get("output_radius"),
                           "internal_feasible": None if summary is None else summary.get("feasible"),
                           "validation": validation,
                           "overrun": result["elapsed_seconds"] > budget + max(0.02, 0.1 * budget)}
                    stream.write(json.dumps(_json_safe(row), allow_nan=False) + "\n")
                    stream.flush(); count += 1
                    if count % 25 == 0 or count == total:
                        print(f"{count}/{total} runs archived", flush=True)
    report, analysis = summarize(out)
    print(report, end="")
    sidecar = out.with_suffix(".summary.json")
    sidecar.write_text(json.dumps(analysis, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    run = sub.add_parser("run")
    run.add_argument("--output", type=Path, required=True)
    run.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    run.add_argument("--budgets", type=float, nargs="+", default=DEFAULT_BUDGETS)
    run.add_argument("--repetitions", type=int, default=5)
    run.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
    run.add_argument("--limit", type=int, help="smoke only; result is always incomplete")
    report = sub.add_parser("summarize")
    report.add_argument("results", type=Path)
    report.add_argument("--legacy", type=Path, default=LEGACY)
    args = parser.parse_args(argv)
    if args.action == "run":
        if (args.repetitions <= 0 or not args.seeds or any(not math.isfinite(b) or b <= 0 for b in args.budgets)
                or any(seed < 0 or seed > 2**64 - 1 for seed in args.seeds)
                or (args.limit is not None and args.limit <= 0)):
            parser.error("budgets, repetitions, seeds (u64), and limit must be positive")
        try:
            execute(args)
        except (OSError, ValueError, KeyError) as exc:
            parser.error(str(exc))
    else:
        try:
            report_text, _ = summarize(args.results, args.legacy)
        except (OSError, ValueError, KeyError, json.JSONDecodeError) as exc:
            parser.error(str(exc))
        print(report_text, end="")


if __name__ == "__main__":
    main()
