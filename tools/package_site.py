#!/usr/bin/env python3
"""Package only public static assets after validating WASM build provenance."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / "web"
ASSETS = (
    "index.html", "styles.css", "app.js", "data.js", "presets.js", "render.js",
    "solver.js", "solver-worker.js", "packing_wasm.wasm", "build-info.json",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    missing = [name for name in ASSETS if not (WEB / name).is_file()]
    if missing:
        parser.error("missing public assets: " + ", ".join(missing))
    try:
        metadata = json.loads((WEB / "build-info.json").read_text(encoding="utf-8"))
        actual_hash = hashlib.sha256((WEB / "packing_wasm.wasm").read_bytes()).hexdigest()
        commit = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, check=True,
            capture_output=True, text=True,
        ).stdout.strip()
        if metadata.get("wasm_sha256") != actual_hash:
            parser.error("WASM SHA-256 does not match build-info.json")
        if metadata.get("source_commit") != commit:
            parser.error("build-info.json source commit does not match HEAD")
    except (ValueError, AttributeError, OSError, subprocess.CalledProcessError) as error:
        parser.error(f"cannot validate build metadata: {error}")

    html = (WEB / "index.html").read_text(encoding="utf-8")
    asset_prefix = f"./assets/{commit[:12]}/"
    for name in ("styles.css", "app.js"):
        original = f'"./{name}"'
        if html.count(original) != 1:
            parser.error(f"index.html must contain exactly one {original} reference")
        html = html.replace(original, f'"{asset_prefix}{name}"')
    marker = '<span id="build-info">Local development build</span>'
    if marker not in html:
        parser.error("index.html is missing the build provenance marker")
    html = html.replace(marker, f'<span id="build-info">Browser build <a href="https://github.com/wotzlaff/packing-circles/commit/{commit}">{commit[:7]}</a> · portable math</span>')

    output = args.output
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        parser.error("output must be a new or empty directory")
    output.mkdir(parents=True, exist_ok=True)
    assets_dir = output / "assets" / commit[:12]
    assets_dir.mkdir(parents=True)
    (output / "index.html").write_text(html, encoding="utf-8")
    for name in ASSETS:
        if name != "index.html":
            shutil.copyfile(WEB / name, assets_dir / name)
    (output / ".nojekyll").touch()
    print(f"Packaged {len(ASSETS)} public assets in {output}")


if __name__ == "__main__":
    main()
