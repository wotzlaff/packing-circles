#!/usr/bin/env python3
"""Generate frozen synthetic inputs without dependencies or Python RNG coupling."""

import argparse
import hashlib
import json
from pathlib import Path


DATA = Path(__file__).resolve().parent / "data"
SEEDS = {"development": "circles-development-v1", "held-out": "circles-held-out-v1"}


def radius_stream(seed, count):
    """Hash-counter sampling with rejection avoids modulo bias (radii 20..99)."""
    values = []
    counter = 0
    while len(values) < count:
        digest = hashlib.sha256(f"{seed}:{counter}".encode("ascii")).digest()
        values.extend(20 + value % 80 for value in digest if value < 240)
        counter += 1
    return values[:count]


def expected_files():
    files = {}
    entries = []
    for split, seed in SEEDS.items():
        for size in (10, 20, 30, 40, 50):
            for index in range(1, 11):
                name = f"{split}/{size}_{index:02}.txt"
                instance_seed = f"{seed}:{size}:{index}"
                radii = radius_stream(instance_seed, size)
                content = ("\n".join(map(str, [size, *radii])) + "\n").encode("ascii")
                files[name] = content
                entries.append({"path": name, "count": size, "seed": instance_seed,
                                "sha256": hashlib.sha256(content).hexdigest()})
    manifest = {"schema_version": 1, "provenance": "synthetic; not original contest data",
                "generator": "SHA-256 counter bytes; reject >=240; radius=20+byte%80",
                "seeds": SEEDS, "instances": entries}
    files["manifest.json"] = (json.dumps(manifest, indent=2) + "\n").encode("ascii")
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify committed data without writing")
    args = parser.parse_args()
    for name, content in expected_files().items():
        path = DATA / name
        if args.check:
            if not path.exists() or path.read_bytes() != content:
                raise SystemExit(f"dataset mismatch: {path}")
        else:
            if path.exists() and path.read_bytes() != content:
                raise SystemExit(f"refusing to overwrite changed dataset: {path}")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
    print("Verified" if args.check else "Generated", "100 synthetic inputs and manifest")


if __name__ == "__main__":
    main()
