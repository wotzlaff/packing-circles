#!/usr/bin/env python3
"""Run serial, config-driven solver comparisons and summarize their raw JSONL."""

import argparse
from collections import defaultdict
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import time
import uuid

from check_solution import parse_instance
from legacy import ROOT, command_for, sha256
from run_baseline import check_output, cpu_model


def _digest(path):
    return sha256(Path(path)) if Path(path).is_file() else None


def _json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"), parse_constant=_bad_number,
                      parse_float=_finite_float)


def _bad_number(token):
    raise ValueError(f"nonfinite JSON number: {token}")


def _finite_float(token):
    value = float(token)
    if not math.isfinite(value):
        _bad_number(token)
    return value


def validate_config(config, config_path):
    if (not isinstance(config, dict) or type(config.get("schema_version")) is not int
            or config.get("schema_version") != 1):
        raise ValueError("config schema_version must be 1")
    inputs = config.get("instance_paths")
    budgets, seeds, reps, solvers = (config.get(k) for k in ("budgets_seconds", "seeds", "repetitions", "solvers"))
    if not isinstance(inputs, list) or not inputs or any(not isinstance(p, str) or not p for p in inputs):
        raise ValueError("instance_paths must be a nonempty list of paths")
    if len(set(inputs)) != len(inputs):
        raise ValueError("instance_paths must be unique")
    canonical_inputs = set()
    for p in inputs:
        path = (ROOT / p).resolve() if not Path(p).is_absolute() else Path(p).resolve()
        if path in canonical_inputs:
            raise ValueError("instance_paths must resolve to unique canonical paths")
        canonical_inputs.add(path)
        if not path.is_file():
            raise ValueError(f"instance path is not a file: {p}")
        parse_instance(path.read_text(encoding="utf-8"))
    if not isinstance(budgets, list) or not budgets or any(isinstance(x, bool) or not isinstance(x, (int, float)) or not math.isfinite(x) or x <= 0 for x in budgets):
        raise ValueError("budgets_seconds must contain positive finite numbers")
    if len(set(budgets)) != len(budgets):
        raise ValueError("budgets_seconds must be unique")
    if not isinstance(seeds, list) or not seeds or any(type(x) is not int or x < 0 or x > 2**64 - 1 for x in seeds):
        raise ValueError("seeds must contain u64 integers")
    if type(reps) is not int or reps <= 0:
        raise ValueError("repetitions must be a positive integer")
    if not isinstance(solvers, list) or not solvers:
        raise ValueError("solvers must be a nonempty list")
    names = set()
    for solver in solvers:
        if not isinstance(solver, dict) or not isinstance(solver.get("name"), str) or not solver["name"]:
            raise ValueError("each solver needs a name")
        if solver["name"] in names:
            raise ValueError("solver names must be unique")
        names.add(solver["name"])
        if solver.get("kind") not in ("legacy", "rust"):
            raise ValueError(f"unsupported solver kind: {solver.get('kind')}")
        exe = solver.get("executable")
        if not isinstance(exe, str) or not exe:
            raise ValueError(f"solver {solver['name']} needs executable")
        exe_path = (ROOT / exe).resolve() if not Path(exe).is_absolute() else Path(exe).resolve()
        if not exe_path.is_file():
            raise ValueError(f"solver executable is not a file: {exe}")
        if solver["kind"] == "legacy" and solver.get("legacy_variant", "contest") not in ("contest", "main"):
            raise ValueError("legacy_variant must be contest or main")
        if solver["kind"] == "rust" and solver.get("algorithm") not in ("a", "b"):
            raise ValueError("Rust solver algorithm must be a or b")
        variant = solver.get("variant")
        if solver["kind"] == "rust" and solver.get("algorithm") == "b":
            if not isinstance(variant, str) or not variant or variant.startswith("-"):
                raise ValueError("Rust B solver needs an explicit variant")
        elif variant is not None:
            raise ValueError("variant is only supported for Rust B")
        sources = solver.get("source_paths", [])
        if not isinstance(sources, list) or any(not isinstance(p, str) or not Path((ROOT / p) if not Path(p).is_absolute() else p).is_file() for p in sources):
            raise ValueError(f"invalid source_paths for {solver['name']}")
    if "traces" in config and type(config["traces"]) is not bool:
        raise ValueError("traces must be a boolean")
    return config


def _summary(stderr, algorithm, seed, budget, variant=None):
    try:
        data = json.loads(stderr.strip(), parse_constant=_bad_number, parse_float=_finite_float)
        if (not isinstance(data, dict) or type(data.get("schema_version")) is not int
                or data.get("schema_version") != 1 or data.get("algorithm") != algorithm):
            raise ValueError("summary schema or algorithm mismatch")
        if algorithm == "b" and (not variant or data.get("variant") != variant):
            raise ValueError("summary variant mismatch")
        for key in ("seed", "evaluations", "iterations", "restarts"):
            if type(data.get(key)) is not int or data[key] < 0:
                raise ValueError(f"invalid summary {key}")
        for key in ("radius", "output_radius", "elapsed_seconds", "time_limit_seconds"):
            v = data.get(key)
            if isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v):
                raise ValueError(f"invalid summary {key}")
        if data["radius"] <= 0 or data["output_radius"] <= 0 or data["elapsed_seconds"] < 0:
            raise ValueError("summary radii must be positive and elapsed_seconds nonnegative")
        if data["seed"] != seed or data["time_limit_seconds"] != budget:
            raise ValueError("summary seed or budget mismatch")
        if data.get("termination") not in {"time_limit", "evaluation_limit", "iteration_limit", "restart_limit", "cancelled", "trivial"}:
            raise ValueError("summary termination is unknown")
        if not isinstance(data.get("feasible"), bool) or not isinstance(data.get("output_feasible"), bool):
            raise ValueError("invalid summary feasibility flags")
        return data, None
    except (ValueError, json.JSONDecodeError) as exc:
        return None, str(exc)


def _same_radius(summary, validation):
    radius = validation.get("enclosing_radius")
    return (isinstance(radius, (int, float)) and not isinstance(radius, bool)
            and math.isfinite(radius)
            and math.isclose(summary.get("output_radius", math.nan), radius,
                             rel_tol=1e-12, abs_tol=1e-12))


def _run(command, text, budget):
    started = time.perf_counter()
    try:
        result = subprocess.run(command, input=text, text=True, capture_output=True,
                                timeout=max(2.0, budget * 3 + 1))
        return result.stdout, result.stderr, result.returncode, False, time.perf_counter() - started
    except subprocess.TimeoutExpired as exc:
        def decode(value):
            return value.decode(errors="replace") if isinstance(value, bytes) else (value or "")
        return decode(exc.stdout), decode(exc.stderr), None, True, time.perf_counter() - started
    except OSError as exc:
        return "", str(exc), None, False, time.perf_counter() - started


def _command(solver, budget, seed, input_path, trace_path=None):
    exe = str((ROOT / solver["executable"]).resolve() if not Path(solver["executable"]).is_absolute() else Path(solver["executable"]).resolve())
    if solver["kind"] == "legacy":
        return command_for(solver.get("legacy_variant", "contest"), budget, exe)
    cmd = [exe, "solve", "-", "--algorithm", solver["algorithm"], "--seed", str(seed), "--time-limit", str(budget)]
    if solver.get("variant") is not None:
        cmd += ["--variant", solver["variant"]]
    if trace_path is not None:
        cmd += ["--trace", str(trace_path)]
    return cmd


def run(config_path, output_path):
    config_path = Path(config_path).resolve()
    config = validate_config(_json(config_path), config_path)
    output = Path(output_path).resolve()
    meta_path = output.with_suffix(".metadata.json")
    trace_dir = output.with_suffix("").with_name(output.stem + ".traces")
    if output.exists() or meta_path.exists() or trace_dir.exists():
        raise ValueError("refusing to overwrite existing output or trace paths")
    output.parent.mkdir(parents=True, exist_ok=True)
    if config.get("traces", False):
        trace_dir.mkdir()
    config_digest = sha256(config_path)
    metadata = {"schema_version": 1, "started_utc": datetime.now(timezone.utc).isoformat(),
                "config_sha256": config_digest, "config": config, "git_root": str(ROOT),
                "platform": platform.platform(), "cpu": cpu_model(), "python": platform.python_version(),
                "root_commit": subprocess.run(["git", "-C", str(ROOT), "rev-parse", "HEAD"],
                                               text=True, capture_output=True).stdout.strip(),
                "toolchains": {name: _toolchain(name) for name in ("rustc", "g++")}}
    metadata["source_hashes"] = {
        name: _digest(ROOT / f"bench/{name}")
        for name in ("compare.py", "check_solution.py", "run_baseline.py", "legacy.py", "replay_trace.py")
    }
    with meta_path.open("x", encoding="utf-8") as f:
        json.dump(metadata, f, indent=2, allow_nan=False); f.write("\n")
    with output.open("x", encoding="utf-8") as stream:
        for inp in config["instance_paths"]:
            path = (ROOT / inp).resolve() if not Path(inp).is_absolute() else Path(inp).resolve()
            text = path.read_text(encoding="utf-8")
            radii = parse_instance(text)
            for budget in config["budgets_seconds"]:
                for rep in range(config["repetitions"]):
                    seed = config["seeds"][rep % len(config["seeds"])]
                    for solver in config["solvers"]:
                        run_id = str(uuid.uuid4())
                        executable_path = Path(solver["executable"])
                        executable_path = executable_path.resolve() if executable_path.is_absolute() else (ROOT / executable_path).resolve()
                        source_paths = [Path(p).resolve() if Path(p).is_absolute() else (ROOT / p).resolve()
                                        for p in solver.get("source_paths", [])]
                        before_hashes = {"executable": _digest(executable_path),
                                         "sources": {str(p): _digest(p) for p in source_paths}}
                        trace_path = None
                        if config.get("traces") and solver["kind"] == "rust":
                            trace_path = trace_dir / f"{run_id}.jsonl"
                        command = _command(solver, budget, seed, path, trace_path)
                        stdout, stderr, rc, timeout, elapsed = _run(command, text, budget)
                        validation = check_output(stdout, radii)
                        summary, summary_error = (None, None)
                        trace_error = None
                        trace_info = None
                        if solver["kind"] == "rust":
                            summary, summary_error = _summary(stderr, solver["algorithm"], seed, budget, solver.get("variant"))
                            if trace_path is not None:
                                if not trace_path.exists():
                                    trace_error = "trace file was not produced"
                                else:
                                    try:
                                        from replay_trace import replay_trace, validate_run_summary
                                        trace_info = replay_trace(trace_path, expected_output=stdout)
                                        if summary is None:
                                            raise ValueError("trace has no valid run summary")
                                        validate_run_summary(trace_info, summary)
                                    except (ImportError, ValueError, OSError) as exc:
                                        trace_error = str(exc)
                        after_hashes = {"executable": _digest(executable_path),
                                        "sources": {str(p): _digest(p) for p in source_paths}}
                        artifacts_changed = before_hashes != after_hashes
                        overrun = elapsed > budget + max(0.02, 0.1 * budget)
                        success = rc == 0 and not timeout and validation.get("feasible") is True and not artifacts_changed
                        if solver["kind"] == "rust":
                            success = (success and summary is not None
                                       and summary.get("feasible") is True
                                       and summary.get("output_feasible") is True
                                       and _same_radius(summary, validation)
                                       and not summary_error)
                            if trace_path is not None:
                                success = (success and trace_error is None and isinstance(trace_info, dict)
                                           and trace_info.get("valid") is True)
                        row = {"schema_version": 1, "run_id": run_id, "solver": solver["name"],
                               "kind": solver["kind"], "algorithm": solver.get("algorithm"),
                               "variant": solver.get("variant"),
                               "config_sha256": config_digest, "instance": str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
                               "circle_count": len(radii), "input_sha256": hashlib.sha256(text.encode()).hexdigest(),
                               "executable_sha256": before_hashes["executable"],
                               "source_hashes": before_hashes["sources"],
                               "artifacts_changed_during_run": artifacts_changed,
                               "budget_seconds": budget, "repetition": rep, "seed": seed,
                               "expected_repetitions": config["repetitions"],
                               "command": command, "stdout": stdout, "stderr": stderr, "returncode": rc,
                               "output_sha256": hashlib.sha256(stdout.encode()).hexdigest(),
                               "timeout": timeout, "elapsed_seconds": elapsed, "overrun": overrun,
                               "validation": validation, "summary": summary, "summary_error": summary_error,
                               "trace_path": os.path.relpath(trace_path, output.parent) if trace_path else None,
                               "trace_requested": bool(config.get("traces")), "trace": trace_info,
                               "trace_run_id": trace_info.get("run_id") if isinstance(trace_info, dict) else None,
                               "trace_error": trace_error, "trace_unavailable": solver["kind"] == "legacy",
                               "successful": bool(success)}
                        stream.write(json.dumps(row, allow_nan=False) + "\n")
                        stream.flush()
    return output


def _toolchain(name):
    try:
        result = subprocess.run([name,"--version"],text=True,capture_output=True)
        return (result.stdout.splitlines() or [None])[0]
    except OSError:
        return None


def _valid(row):
    if row.get("kind") not in ("legacy","rust"):
        return False
    if row.get("successful") is not True or row.get("timeout") is not False or row.get("returncode") != 0:
        return False
    if row.get("overrun") or row.get("validation", {}).get("feasible") is not True:
        return False
    if row.get("artifacts_changed_during_run"):
        return False
    if row.get("kind") == "rust":
        if row.get("summary_error"):
            return False
        s = row.get("summary")
        try:
            parsed, error = _summary(json.dumps(s, allow_nan=False), row.get("algorithm"),
                                     row.get("seed"), row.get("budget_seconds"), row.get("variant"))
        except (TypeError, ValueError):
            return False
        if (error or parsed is None or parsed.get("feasible") is not True
                or parsed.get("output_feasible") is not True
                or not _same_radius(parsed, row.get("validation", {}))):
            return False
        if row.get("trace_requested"):
            trace = row.get("trace")
            if (row.get("trace_error") or not isinstance(trace, dict)
                    or trace.get("valid") is not True
                    or trace.get("run_id") != row.get("trace_run_id")):
                return False
            try:
                from replay_trace import validate_run_summary
                validate_run_summary(trace, parsed)
            except (ImportError, ValueError, TypeError, KeyError):
                return False
        return True
    return True


def summarize(path):
    rows = [json.loads(line) for line in Path(path).read_text(encoding="utf-8").splitlines() if line.strip()]
    groups = defaultdict(list)
    for row in rows:
        groups[(row.get("solver", "?"), row.get("budget_seconds"), row.get("circle_count"))].append(row)
    lines = ["# Solver comparison", "", f"Generated from `{Path(path).name}`. Invalid outputs and deadline overruns remain visible and do not enter quality statistics.", "",
             "| Solver | Circles | Budget (s) | Runs | Valid | Overruns | Median elapsed (s) | Median radius |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for (solver, budget, count), batch in sorted(groups.items(), key=lambda x: (x[0][0], x[0][1], x[0][2] or 0)):
        good = [r for r in batch if _valid(r)]
        elapsed = statistics.median(r["elapsed_seconds"] for r in good) if good else None
        radius = statistics.median(r["validation"]["enclosing_radius"] for r in good) if good else None
        lines.append(f"| {solver} | {count if count is not None else '—'} | {budget:g} | {len(batch)} | {len(good)} | {sum(bool(r.get('overrun')) for r in batch)} | {'—' if elapsed is None else f'{elapsed:.6f}'} | {'—' if radius is None else f'{radius:.8g}'} |")
    solvers = list(dict.fromkeys(r.get("solver", "?") for r in rows))
    budgets = sorted({r["budget_seconds"] for r in rows})
    for budget in budgets:
        by_solver_instance = defaultdict(lambda: defaultdict(list))
        attempts = defaultdict(lambda: defaultdict(list))
        for row in rows:
            if row.get("budget_seconds") == budget:
                attempts[row.get("solver")][row.get("instance")].append(row)
        for solver, instances in attempts.items():
            for instance, batch in instances.items():
                expected=max(row.get("expected_repetitions",1) for row in batch)
                repetitions={row.get("repetition",0) for row in batch}
                if (len(batch)==expected and repetitions==set(range(expected))
                        and all(_valid(row) for row in batch)):
                    by_solver_instance[solver][instance]=[row["validation"]["enclosing_radius"] for row in batch]
        lines.extend(["", f"## Per-instance ratios at {budget:g} s", "", "Ratios use per-instance medians against the first configured solver. Only common instances with every expected repetition valid and on time enter comparisons.", "",
                      "| Solver | Common instances | Geometric mean radius ratio |", "| --- | ---: | ---: |"])
        base = by_solver_instance.get(solvers[0], {}) if solvers else {}
        for solver in solvers:
            cohort = sorted(set(base) & set(by_solver_instance.get(solver, {})))
            ratios = [statistics.median(by_solver_instance[solver][i]) / statistics.median(base[i]) for i in cohort]
            gm = math.exp(statistics.mean(math.log(x) for x in ratios)) if ratios else None
            lines.append(f"| {solver} | {len(cohort)} | {'—' if gm is None else f'{gm:.8f}'} |")
    lines.append("")
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    run_parser = sub.add_parser("run")
    run_parser.add_argument("--config", required=True, type=Path)
    run_parser.add_argument("--output", required=True, type=Path)
    sum_parser = sub.add_parser("summarize")
    sum_parser.add_argument("results", type=Path)
    sum_parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.action == "run":
            print(run(args.config, args.output))
        else:
            report = summarize(args.results)
            if args.output:
                args.output.write_text(report, encoding="utf-8")
            else:
                sys.stdout.write(report)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        parser.error(str(exc))


if __name__ == "__main__":
    main()
