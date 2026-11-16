#!/usr/bin/env python3
"""Build and copy the worker binary without a JavaScript bundler."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    env = os.environ.copy()
    # Bound linear memory to 64 MiB (1024 WASM pages), including stack and heap.
    env["RUSTFLAGS"] = (env.get("RUSTFLAGS", "") + " -C link-arg=--max-memory=67108864").strip()
    subprocess.run([
        "cargo", "build", "-p", "packing-wasm", "--release", "--locked",
        "--target", "wasm32-unknown-unknown",
    ], cwd=ROOT, env=env, check=True)
    binary = ROOT / "target/wasm32-unknown-unknown/release/packing_wasm.wasm"
    # Respect Cargo's optional target-dir override when locating the artifact.
    if env.get("CARGO_TARGET_DIR"):
        binary = Path(env["CARGO_TARGET_DIR"]) / "wasm32-unknown-unknown/release/packing_wasm.wasm"
        if not binary.is_absolute():
            binary = ROOT / binary
    destination = ROOT / "web/packing_wasm.wasm"
    shutil.copyfile(binary, destination)
    commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    dirty = bool(subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, text=True, check=True).stdout)
    rustc = subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=True).stdout.strip()
    metadata = {"version": "0.1.0", "source_commit": commit, "dirty": dirty,
                "math_profile": "portable-libm-0.2.16", "rustc": rustc,
                "wasm_sha256": hashlib.sha256(destination.read_bytes()).hexdigest()}
    (ROOT / "web/build-info.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(f"Built {destination.relative_to(ROOT)} ({destination.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
