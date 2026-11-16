#!/usr/bin/env python3
"""Independent parser and geometric validator for circle-packing solutions.

Contest input is ``n`` followed by n radii. Output is the enclosing radius,
then n rows of ``radius x y``.  Coordinates use a common center at the origin.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from typing import Any


INTERNAL_TOLERANCE_FACTOR = 1e-9
FOUR_DECIMAL_UNIT = 1e-4


def _finite_float(token: str, what: str) -> float:
    try:
        value = float(token)
    except ValueError as exc:
        raise ValueError(f"invalid {what}: {token!r}") from exc
    if not math.isfinite(value):
        raise ValueError(f"{what} must be finite")
    return value


def parse_instance(text: str) -> list[float]:
    """Parse positive finite radii from contest-format instance text."""
    tokens = text.split()
    if not tokens:
        raise ValueError("instance is empty")
    try:
        count = int(tokens[0], 10)
    except ValueError as exc:
        raise ValueError("circle count must be an integer") from exc
    if count <= 0:
        raise ValueError("circle count must be positive")
    if len(tokens) != count + 1:
        raise ValueError(f"expected {count} radii, found {len(tokens) - 1}")
    radii = [_finite_float(token, f"radius {index}") for index, token in enumerate(tokens[1:])]
    if any(radius <= 0 for radius in radii):
        raise ValueError("all radii must be positive")
    return radii


def parse_solution(text: str, radii: list[float]) -> dict[str, Any]:
    """Parse output into ``{'radius': R, 'circles': [(r, x, y), ...]}``."""
    tokens = text.split()
    expected = 1 + 3 * len(radii)
    if len(tokens) != expected:
        raise ValueError(f"expected enclosing radius and {len(radii)} circle rows ({expected} values), found {len(tokens)}")
    values = [_finite_float(token, f"solution value {index}") for index, token in enumerate(tokens)]
    enclosing_radius = values[0]
    if enclosing_radius <= 0:
        raise ValueError("enclosing radius must be positive")
    circles = [tuple(values[offset:offset + 3]) for offset in range(1, expected, 3)]
    return {"radius": enclosing_radius, "circles": circles}


def validate_solution(radii: list[float], solution: dict[str, Any]) -> dict[str, Any]:
    """Return JSON-safe feasibility metrics, retaining strict and rounded checks.

    The strict check uses ``1e-9 * max(1, R, max_radius)``. The output-rounding
    allowances model independently rounded four-decimal x/y and R fields;
    they are reported separately and never make ``feasible`` true.
    """
    if not radii or any(not math.isfinite(r) or r <= 0 for r in radii):
        raise ValueError("radii must be a nonempty list of positive finite values")
    try:
        radius = float(solution["radius"])
        circles = solution["circles"]
    except (KeyError, TypeError, ValueError) as exc:
        raise ValueError("solution must contain radius and circles") from exc
    if not math.isfinite(radius) or radius <= 0:
        raise ValueError("enclosing radius must be positive and finite")

    count_matches = isinstance(circles, (list, tuple)) and len(circles) == len(radii)
    if not count_matches:
        return {
            "feasible": False,
            "count_matches": False,
            "radii_match": False,
            "enclosing_radius": radius,
            "internal_tolerance": INTERNAL_TOLERANCE_FACTOR * max(1.0, radius, max(radii)),
            "max_containment_violation": None,
            "max_separation_violation": None,
            "max_violation": None,
            "rounding_allowances": _rounding_allowances(),
            "rounding_feasible": False,
        }

    parsed: list[tuple[float, float, float]] = []
    finite = True
    for circle in circles:
        try:
            if len(circle) != 3:
                finite = False
                break
            cr, x, y = (float(value) for value in circle)
        except (TypeError, ValueError):
            finite = False
            break
        if not all(math.isfinite(value) for value in (cr, x, y)):
            finite = False
            break
        parsed.append((cr, x, y))
    if not finite:
        return {
            "feasible": False,
            "count_matches": True,
            "radii_match": False,
            "finite": False,
            "enclosing_radius": radius,
            "internal_tolerance": INTERNAL_TOLERANCE_FACTOR * max(1.0, radius, max(radii)),
            "max_containment_violation": None,
            "max_separation_violation": None,
            "max_violation": None,
            "rounding_allowances": _rounding_allowances(),
            "rounding_feasible": False,
        }

    tolerance = INTERNAL_TOLERANCE_FACTOR * max(1.0, radius, max(radii))
    radii_match = all(abs(circle[0] - expected) <= tolerance for circle, expected in zip(parsed, radii))
    containment = [math.hypot(x, y) + expected_radius - radius
                   for expected_radius, (_, x, y) in zip(radii, parsed)]
    separation = [radii[i] + radii[j] - math.hypot(parsed[i][1] - parsed[j][1], parsed[i][2] - parsed[j][2])
                  for i in range(len(radii)) for j in range(i + 1, len(radii))]
    max_containment = max([0.0, *containment])
    max_separation = max([0.0, *separation])
    allowances = _rounding_allowances()
    feasible = radii_match and max_containment <= tolerance and max_separation <= tolerance
    rounding_feasible = (radii_match and max_containment <= tolerance + allowances["containment"]
                         and max_separation <= tolerance + allowances["separation"])
    return {
        "feasible": feasible,
        "count_matches": True,
        "radii_match": radii_match,
        "finite": True,
        "enclosing_radius": radius,
        "internal_tolerance": tolerance,
        "max_containment_violation": max_containment,
        "max_separation_violation": max_separation,
        "max_violation": max(max_containment, max_separation),
        "rounding_allowances": allowances,
        "rounding_feasible": rounding_feasible,
    }


def _rounding_allowances() -> dict[str, float]:
    half_unit = FOUR_DECIMAL_UNIT / 2
    return {
        "coordinate_component": half_unit,
        "enclosing_radius": half_unit,
        "containment": math.sqrt(2) * half_unit + half_unit,
        "separation": 2 * math.sqrt(2) * half_unit,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("instance", help="contest-format instance file")
    parser.add_argument("solution", help="contest-format solution file")
    args = parser.parse_args(argv)
    try:
        with open(args.instance, encoding="utf-8") as stream:
            radii = parse_instance(stream.read())
        with open(args.solution, encoding="utf-8") as stream:
            solution = parse_solution(stream.read(), radii)
        report = validate_solution(radii, solution)
    except (OSError, ValueError) as exc:
        print(json.dumps({"error": str(exc), "feasible": False}), file=sys.stdout)
        return 2
    print(json.dumps(report, sort_keys=True))
    return 0 if report["feasible"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
