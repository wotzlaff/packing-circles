#!/usr/bin/env python3
"""Compare fixed-state Rust kernels with probes of the unmodified C++ reference."""

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import platform
import random
import subprocess

from legacy import ROOT, BUILD, git_revision, sha256


ABS_TOL = 1e-9
REL_TOL = 1e-8


def scalar(radii, state, weights):
    """Independent mathematical objective for central finite differences."""
    value = max(0.0, state[0])
    for i, radius in enumerate(radii):
        x, y = state[1 + 2*i:3 + 2*i]
        excess = max(0.0, x*x + y*y - max(0.0, state[0]-radius)**2)
        value += weights[0] * excess**2
    for i, radius in enumerate(radii):
        for j in range(i):
            dx, dy = state[1+2*i]-state[1+2*j], state[2+2*i]-state[2+2*j]
            excess = max(0.0, (radius+radii[j])**2 - dx*dx - dy*dy)
            value += weights[1] * excess**2
    return value


def finite_difference_error(radii, state, weights, gradient):
    differences = []
    for i, component in enumerate(state):
        step = 1e-6 * max(1.0, abs(component))
        high, low = state.copy(), state.copy()
        high[i] += step
        low[i] -= step
        approximation = (scalar(radii, high, weights)-scalar(radii, low, weights)) / (2*step)
        differences.append(gradient[i]-approximation)
    return math.hypot(*differences) / max(1.0, math.hypot(*gradient))


def packet(radii, state, weights, direction=None, previous=None, steps=8, callback_tol=1e-5,
           sample_gradient_squared=0.0):
    """Version-one common packet consumed by both independent probe binaries."""
    direction = direction if direction is not None else [0.0]*len(state)
    fields = [len(radii), *radii, *weights, *state, *direction,
              int(previous is not None), previous or 0.0, 0.1, 0.95, steps,
              1e-24, callback_tol, 1e-5, sample_gradient_squared]
    return " ".join(map(str, fields)) + "\n"


def execute(binary, mode, text):
    result = subprocess.run([str(binary), mode], input=text, text=True, capture_output=True, timeout=15)
    if result.returncode != 0:
        raise ValueError(f"{binary.name} {mode}: exit {result.returncode}: {result.stderr}")
    def reject_constant(value):
        raise ValueError(f"nonfinite JSON constant: {value}")
    return json.loads(result.stdout, parse_constant=reject_constant)


def compare(expected, actual, path="result"):
    """Compare every exported scalar, state, and callback event, not just endpoints."""
    maximum = 0.0
    if isinstance(expected, bool) != isinstance(actual, bool):
        raise ValueError(f"{path}: boolean/numeric types differ")
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            raise ValueError(f"{path}: expected object")
        if expected.keys() != actual.keys():
            raise ValueError(f"{path}: output fields differ")
        for key in expected:
            maximum = max(maximum, compare(expected[key], actual[key], path+"."+key))
    elif isinstance(expected, list):
        if not isinstance(actual, list):
            raise ValueError(f"{path}: expected array")
        if len(expected) != len(actual):
            raise ValueError(f"{path}: lengths differ {len(expected)} != {len(actual)}")
        for index, (a, b) in enumerate(zip(expected, actual)):
            maximum = max(maximum, compare(a, b, f"{path}[{index}]"))
    elif isinstance(expected, bool):
        if expected != actual:
            raise ValueError(f"{path}: flags differ")
    else:
        if not isinstance(expected, (int,float)) or not isinstance(actual,(int,float)):
            raise ValueError(f"{path}: expected numeric scalar")
        if not math.isfinite(expected) or not math.isfinite(actual):
            raise ValueError(f"{path}: nonfinite result")
        allowance = ABS_TOL + REL_TOL * max(abs(expected), abs(actual))
        maximum = abs(expected-actual) / allowance
        if maximum > 1:
            raise ValueError(f"{path}: {expected} != {actual} ({maximum:.3g} × tolerance)")
    return maximum


def replay_report(path, rust):
    """Run Rust against archived C++ oracles; CI needs no legacy checkout."""
    report = json.loads(path.read_text())
    if report.get("schema_version") != 1 or report.get("failures"):
        raise ValueError("replay requires a successful version-one reference report")
    cases=report["cases"]
    counts=Counter(record["mode"] for record in cases)
    if (dict(counts) != report["counts"] or counts["objective"] < 100
            or any(counts[mode] == 0 for mode in ("line-search","bfgs","adaptive","schedule","repair"))
            or len({(record["id"],record["mode"]) for record in cases}) != len(cases)):
        raise ValueError("incomplete or duplicate reference cases")
    maximum=0.0
    fd_count=0
    for record in cases:
        text=record["input"]
        if not record["passed"] or hashlib.sha256(text.encode()).hexdigest() != record["input_sha256"]:
            raise ValueError("invalid reference record: "+record["id"])
        actual=execute(rust,record["mode"],text)
        maximum=max(maximum,compare(record["cpp"],actual,record["id"]+"/"+record["mode"]))
        if "finite_difference_relative_error" in record:
            values=list(map(float,text.split()))
            n=int(values[0])
            radii=values[1:1+n]
            weights=values[1+n:3+n]
            state=values[3+n:4+3*n]
            error=finite_difference_error(radii,state,weights,actual["gradient"])
            if error >= 1e-5:
                raise ValueError(f"{record['id']}: finite-difference gate failed: {error}")
            fd_count+=1
    if fd_count < 100 or fd_count != report["finite_difference_states"]:
        raise ValueError("incomplete finite-difference replay")
    print(f"Replayed {len(cases)} C++ reference cases and {fd_count} finite-difference states; "
          f"maximum error {maximum:.3g} × tolerance")


def configurations():
    """Only development radii; no optimization or held-out evaluation is involved."""
    rng = random.Random(2026111603)
    manifest = json.loads((ROOT/"bench/data/manifest.json").read_text())
    entries = [e for e in manifest["instances"] if e["path"].startswith("development/")]
    for i in range(128):
        entry = entries[i % len(entries)]
        path = ROOT/"bench/data"/entry["path"]
        if sha256(path) != entry["sha256"]:
            raise ValueError("development data hash mismatch")
        radii = list(map(float, path.read_text().split()[1:]))
        scale = math.hypot(*radii)
        # Continuous states avoid exact tangencies/kinks; deliberately cover
        # both active and inactive containment and pairwise penalty terms.
        for _ in range(10000):
            state = [scale*rng.uniform(0.8, 1.7)]
            state.extend(rng.uniform(-scale, scale) for _ in range(2*len(radii)))
            margins = []
            for j, radius in enumerate(radii):
                x,y = state[1+2*j:3+2*j]
                margins.append(abs(x*x+y*y-max(0.,state[0]-radius)**2))
                for k in range(j):
                    dx,dy = x-state[1+2*k], y-state[2+2*k]
                    margins.append(abs((radius+radii[k])**2-dx*dx-dy*dy))
            if min(margins) > 1e-5*scale*scale:
                break
        else:
            raise ValueError("could not generate a configuration away from penalty kinks")
        weights = [1e-7 * 10**rng.uniform(-1, 2), 1e-5 * 10**rng.uniform(-1, 2)]
        yield f"state-{i:03}", radii, state, weights


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action=parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--output", type=Path)
    action.add_argument("--replay-report", type=Path,
                        help="compare Rust with archived C++ outputs without a legacy checkout")
    parser.add_argument("--cpp", type=Path, default=BUILD/"reference-probe")
    parser.add_argument("--rust", type=Path, default=ROOT/"target/release/examples/numerical_probe")
    args = parser.parse_args()
    if args.replay_report:
        replay_report(args.replay_report,args.rust)
        return
    if args.output.exists():
        parser.error("refusing to overwrite an existing report")
    records = []
    failures = []
    fd_errors = []
    maximum = 0.0
    activation_counts = Counter()

    def check(case_id, mode, text):
        nonlocal maximum
        record = {"id": case_id, "mode": mode, "input": text,
                  "input_sha256": hashlib.sha256(text.encode()).hexdigest()}
        try:
            record["cpp"] = execute(args.cpp, mode, text)
            record["rust"] = execute(args.rust, mode, text)
            record["max_tolerance_fraction"] = compare(record["cpp"], record["rust"])
            maximum = max(maximum, record["max_tolerance_fraction"])
            record["passed"] = True
        except (ValueError, subprocess.TimeoutExpired) as exc:
            record.update({"passed": False, "error": str(exc)})
            failures.append(case_id+"/"+mode+": "+str(exc))
        records.append(record)
        return record.get("rust")

    for index, (case_id, radii, state, weights) in enumerate(configurations()):
        for j, radius in enumerate(radii):
            x,y = state[1+2*j:3+2*j]
            activation_counts["containment_active" if x*x+y*y > max(0.,state[0]-radius)**2
                              else "containment_inactive"] += 1
            for k in range(j):
                dx,dy = x-state[1+2*k], y-state[2+2*k]
                activation_counts["pairwise_active" if dx*dx+dy*dy < (radius+radii[k])**2
                                  else "pairwise_inactive"] += 1
        objective = check(case_id, "objective", packet(radii, state, weights))
        if objective:
            error = finite_difference_error(radii, state, weights, objective["gradient"])
            fd_errors.append(error)
            records[-1]["finite_difference_relative_error"] = error
            if error >= 1e-5:
                failures.append(f"{case_id}: finite difference relative error {error}")
                records[-1]["passed"] = False
        check(case_id, "repair", packet(radii, state, weights))
        if index < 32 and objective:
            direction = [-g for g in objective["gradient"]]
            previous = objective["value"]*1.1 if index % 2 else None
            check(case_id, "line-search", packet(radii,state,weights,direction,previous,steps=100))
            check(case_id, "bfgs", packet(radii,state,weights))
            check(case_id, "adaptive", packet(radii,state,weights,callback_tol=1e100))
            check(case_id, "schedule", packet(radii,state,weights))
            check(case_id+"-inactive", "schedule", packet(radii,state,weights,sample_gradient_squared=1.0))

    # Semantic expectations ensure intended branches actually execute.
    branches = [([1.,-1.,0.,1.,0.], [2e-7,1e-5], False),
                ([3.,-.5,0.,.5,0.], [1e-7,2e-5], False),
                ([1.,-.5,0.,.5,0.], [2e-7,2e-5], False),
                ([2.,-1.,0.,1.,0.], [1e-7,1e-5], True),
                ([2.,0.,0.,0.,0.], [1e-7,2e-5], False),
                ([-1.,-.5,0.,.5,0.], [1e-7,2e-5], False),
                ([0.,-1.,0.,1.,0.], [2e-7,1e-5], False)]
    for index,(state,weights,terminate) in enumerate(branches):
        result=check(f"schedule-branch-{index}", "schedule", packet([1.,1.],state,[1e-7,1e-5]))
        if result != {"weights":weights,"refresh":True,"terminate":terminate}:
            failures.append(f"schedule-branch-{index}: intended branch was not observed")
    for gradient_squared in (1e-5,1e-5*(1+1e-8)):
        result=check(f"schedule-threshold-{gradient_squared}","schedule",
                     packet([1.,1.],[2.,-1.,0.,1.,0.],[1e-7,1e-5],sample_gradient_squared=gradient_squared))
        if result != {"weights":[1e-7,1e-5],"refresh":False,"terminate":False}:
            failures.append("gradient threshold must not trigger a refresh at or above equality")
    check("negative-radius", "objective", packet([1.],[-2.,1.,0.],[1e-7,1e-5]))
    if len(fd_errors) != 128 or any(activation_counts[key] == 0 for key in
            ("containment_active","containment_inactive","pairwise_active","pairwise_inactive")):
        failures.append("objective/finite-difference coverage gate was not met")
    report = {"schema_version": 1, "created_utc": datetime.now(timezone.utc).isoformat(),
              "root_commit": git_revision(ROOT), "legacy_commit": git_revision(ROOT/"legacy"),
              "source_sha256": sha256(ROOT/"legacy/circles-hackbfgs.tr.cpp"),
              "cpp_probe_sha256": sha256(ROOT/"bench/reference_probe.cpp"),
              "rust_probe_sha256": sha256(ROOT/"crates/packing-core/examples/numerical_probe.rs"),
              "harness_sha256": sha256(Path(__file__)),
              "rust_sources": {str(p.relative_to(ROOT)):sha256(p)
                               for p in sorted((ROOT/"crates/packing-core/src").rglob("*.rs"))},
              "manifest_sha256": sha256(ROOT/"bench/data/manifest.json"),
              "python": platform.python_version(),
              "cpp_binary_sha256": sha256(args.cpp), "rust_binary_sha256": sha256(args.rust),
              "compiler": subprocess.check_output(["g++","--version"],text=True).splitlines()[0],
              "rustc": subprocess.check_output(["rustc","--version"],text=True).strip(),
              "cpp_flags": ["-O2","-std=c++17"], "rust_profile": "release",
              "seed": 2026111603, "absolute_tolerance": ABS_TOL, "relative_tolerance": REL_TOL,
              "counts": dict(Counter(r["mode"] for r in records)),
              "activation_counts":dict(activation_counts),
              "max_tolerance_fraction": maximum,
              "finite_difference_states":len(fd_errors),
              "max_finite_difference_relative_error": max(fd_errors,default=None),
              "failures":failures, "cases":records}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as stream:
        json.dump(report, stream, indent=2, allow_nan=False)
        stream.write("\n")
    print(json.dumps({k:v for k,v in report.items() if k != "cases"},indent=2))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
