#!/usr/bin/env python3
"""Archive serial runs of the frozen legacy executable on development inputs."""

import argparse
from collections import defaultdict
from datetime import datetime, timezone
import json
import math
from pathlib import Path
import platform
import statistics
import subprocess

from check_solution import parse_instance, parse_solution, validate_solution
from legacy import BUILD, ROOT, git_revision, run, sha256


def cpu_model():
    path = Path("/proc/cpuinfo")
    if path.exists():
        for line in path.read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    return platform.processor()


def check_output(text, radii):
    try:
        report = validate_solution(radii, parse_solution(text, radii))
        radius = report["enclosing_radius"]
        report["area_bound_ratio"] = radius / math.hypot(*radii)
        return report
    except ValueError as exc:
        return {"feasible": False, "parse_error": str(exc)}


def summarize(path):
    """Derive summaries only from raw rows; invalid results never enter quality stats."""
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    groups = defaultdict(list)
    for row in rows:
        groups[(row["variant"], row["budget_seconds"], row["circle_count"])].append(row)
    lines = ["# Legacy baseline results", "", "Generated from `" + path.name + "`.", "",
             "The unmodified contest candidate has a sanitizer-confirmed out-of-bounds read.",
             "These results describe observed behavior, not reproducibility or memory safety.",
             "See [the source map](../../docs/legacy-baseline.md).", "",
             "Validation covers serialized output only; internal states are unavailable.",
             "Each cell groups ten instances × five repetitions in the full protocol.",
             "Quality ratios include successful, strictly feasible runs only.", "",
             "| Variant | Budget (s) | N | Runs | Valid | Crashes | Timeouts | Overruns | Median elapsed (s) | Median area ratio |",
             "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for (variant, budget, size), batch in sorted(groups.items()):
        valid = [r for r in batch if r["successful"]]
        ratios = [r["validation"]["area_bound_ratio"] for r in valid]
        ratio = f"{statistics.median(ratios):.6f}" if ratios else "—"
        lines.append(f"| {variant} | {budget} | {size} | {len(batch)} | {len(valid)} | "
                     f"{sum(r['returncode'] not in (0, None) for r in batch)} | "
                     f"{sum(r['timeout'] for r in batch)} | {sum(r['overrun'] for r in batch)} | "
                     f"{statistics.median(r['elapsed_seconds'] for r in batch):.6f} | {ratio} |")
    lines.extend(["", "Overrun threshold: budget + max(20 ms, 10% of budget).",
                  "Crashes, invalid output, and timeouts are retained in the raw JSONL.",
                  "Area ratios are lower-bound ratios, not proof of optimality.", ""])
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    execute = sub.add_parser("run")
    execute.add_argument("--variant", choices=("contest", "main"), default="contest")
    execute.add_argument("--output", type=Path, required=True)
    execute.add_argument("--budgets", type=float, nargs="+", default=[0.1, 0.5, 1.95])
    execute.add_argument("--repetitions", type=int, default=5)
    execute.add_argument("--limit", type=int, help="smoke only: first N inputs; never a full baseline")
    report = sub.add_parser("summarize")
    report.add_argument("results", type=Path)
    args = parser.parse_args()
    if args.action == "summarize":
        print(summarize(args.results), end="")
        return
    if (args.repetitions <= 0 or any(not math.isfinite(b) or b <= 0 for b in args.budgets)
            or (args.limit is not None and args.limit <= 0)):
        parser.error("budgets, repetitions and limit must be positive and finite")

    manifest_path = ROOT / "bench/data/manifest.json"
    manifest = json.loads(manifest_path.read_text())
    entries = [entry for entry in manifest["instances"] if entry["path"].startswith("development/")]
    for entry in entries:
        if sha256(manifest_path.parent / entry["path"]) != entry["sha256"]:
            raise SystemExit(f"input hash mismatch: {entry['path']}")
    entries = entries[:args.limit]
    binary = BUILD / args.variant
    build_info = json.loads(binary.with_suffix(".json").read_text())
    if build_info["sanitizers"] or sha256(binary) != build_info["binary_sha256"]:
        raise SystemExit("baseline requires the recorded release binary")
    for name, digest in build_info["included_sources"].items():
        if sha256(ROOT / "legacy" / name) != digest:
            raise SystemExit(f"stale build: {name}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    meta_path = args.output.with_suffix(".metadata.json")
    if args.output.exists() or meta_path.exists():
        raise SystemExit("refusing to overwrite existing evidence; choose a new output path")
    metadata = {"schema_version": 1, "started_utc": datetime.now(timezone.utc).isoformat(),
                "root_commit": git_revision(ROOT), "build": build_info,
                "runner_sha256": sha256(Path(__file__)),
                "checker_sha256": sha256(ROOT / "bench/check_solution.py"),
                "wrapper_sha256": sha256(ROOT / "bench/legacy.py"),
                "platform": platform.platform(), "cpu": cpu_model(),
                "python": platform.python_version(), "manifest_sha256": sha256(manifest_path),
                "protocol": {"split": "development", "input_count": len(entries),
                             "budgets_seconds": args.budgets, "repetitions": args.repetitions,
                             "serial_execution": True, "external_machine_load_controlled": False},
                "rng": "fixed seed_seq{5}; standard-library-dependent shuffle"
                       if args.variant == "contest" else "random_device per initialization",
                "limitations": ["unmodified legacy; no internal feasibility or evaluation counts",
                                "contest variant has sanitizer-confirmed undefined behavior"]}
    with meta_path.open("x") as stream:
        json.dump(metadata, stream, indent=2)
        stream.write("\n")
    total = len(entries) * len(args.budgets) * args.repetitions
    with args.output.open("x") as stream:
        count = 0
        for budget in args.budgets:
            for entry in entries:
                text = (manifest_path.parent / entry["path"]).read_text()
                radii = parse_instance(text)
                for repetition in range(args.repetitions):
                    result = run(args.variant, text, budget)
                    validation = check_output(result["stdout"], radii)
                    row = {"schema_version": 1, "variant": args.variant,
                           "instance": entry["path"], "input_sha256": entry["sha256"],
                           "circle_count": len(radii), "budget_seconds": budget,
                           "repetition": repetition, "evaluation_count": None,
                           "seed": 5 if args.variant == "contest" else None,
                           **result, "validation": validation,
                           "successful": result["returncode"] == 0 and not result["timeout"]
                                         and validation["feasible"],
                           "overrun": result["elapsed_seconds"] > budget + max(0.02, 0.1 * budget)}
                    stream.write(json.dumps(row, allow_nan=False) + "\n")
                    stream.flush()
                    count += 1
                    if count % 25 == 0 or count == total:
                        print(f"{count}/{total} runs archived", flush=True)
    print(summarize(args.output), end="")


if __name__ == "__main__":
    main()
