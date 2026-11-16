#!/usr/bin/env python3
"""Build unmodified legacy sources and run them with explicit wall-clock budgets."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "bench" / "build"
SOURCES = {"contest": "circles-hackbfgs.tr.cpp", "main": "main.cpp"}
FLAGS = ["-O2", "-std=c++17"]


def git_revision(path):
    return subprocess.check_output(["git", "-C", str(path), "rev-parse", "HEAD"], text=True).strip()


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(variant, sanitize=False):
    """Compile outside legacy/ and retain exact build provenance beside the binary."""
    BUILD.mkdir(parents=True, exist_ok=True)
    source = ROOT / "legacy" / SOURCES[variant]
    binary = BUILD / (variant + ("-asan" if sanitize else ""))
    flags = (["-O1", "-g", "-std=c++17", "-fsanitize=address,undefined",
              "-fno-omit-frame-pointer"] if sanitize else FLAGS)
    command = ["g++", *flags, str(source), "-o", str(binary)]
    subprocess.run(command, check=True)
    metadata = {"variant": variant, "source": str(source.relative_to(ROOT)),
                "source_sha256": sha256(source), "legacy_commit": git_revision(source.parent),
                "compiler": subprocess.check_output(["g++", "--version"], text=True).splitlines()[0],
                "flags": flags, "binary_sha256": sha256(binary), "sanitizers": sanitize,
                "included_sources": {p.name: sha256(p) for p in
                                     ([source, source.parent / "fun_softmax.cpp"]
                                      if variant == "main" else [source])}}
    binary.with_suffix(".json").write_text(json.dumps(metadata, indent=2) + "\n")
    return binary


def command_for(variant, budget, binary=None):
    if not math.isfinite(budget) or budget <= 0:
        raise ValueError("budget must be positive and finite")
    executable = str(binary or (BUILD / variant))
    if variant == "contest":
        return [executable, "1e-7", "1e-5", "1e-5", "1e-5", "1", str(budget), "0.1", "0.95"]
    return [executable, "10000", "0.0001", "10", "50", str(budget), "1e-6", "0.2", "0.5"]


def run(variant, text, budget, binary=None):
    """Capture every outcome, including timeout/crash; never modify invalid output."""
    command = command_for(variant, budget, binary)
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    compile_parser = sub.add_parser("build")
    compile_parser.add_argument("--variant", choices=SOURCES, default="contest")
    compile_parser.add_argument("--sanitize", action="store_true")
    run_parser = sub.add_parser("run")
    run_parser.add_argument("--variant", choices=SOURCES, default="contest")
    run_parser.add_argument("--input", type=Path, required=True)
    run_parser.add_argument("--budget", type=float, default=1.95)
    args = parser.parse_args()
    if args.action == "build":
        print(build(args.variant, args.sanitize))
    else:
        result = run(args.variant, args.input.read_text(), args.budget)
        sys.stdout.write(result["stdout"])
        sys.stderr.write(result["stderr"])
        print(json.dumps({k: v for k, v in result.items() if k not in ("stdout", "stderr")}),
              file=sys.stderr)
        if result["timeout"] or result["returncode"] != 0:
            raise SystemExit(1)


if __name__ == "__main__":
    main()
