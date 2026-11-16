#!/usr/bin/env python3
"""Replay archived fixed-work A output and work counters on development inputs."""
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    archive = json.loads(Path('bench/results/observer-regression.json').read_text())
    for record in archive['instances']:
        data = Path(record['instance']).read_bytes()
        if hashlib.sha256(data).hexdigest() != record['input_sha256']:
            raise ValueError(f"input hash mismatch: {record['instance']}")
        result = subprocess.run(['target/release/packing-cli', 'solve', '-', '--algorithm', 'a',
                                 '--seed', str(archive['seed']), '--fixed-work', '--evaluations',
                                 str(archive['evaluation_limit'])], input=data, capture_output=True, check=True)
        summary = json.loads(result.stderr)
        if hashlib.sha256(result.stdout).hexdigest() != record['output_sha256']:
            raise ValueError(f"output mismatch: {record['instance']}")
        for key, expected in record['counts'].items():
            if summary[key] != expected:
                raise ValueError(f"{key} mismatch: {record['instance']}")
    print(f"Matched outputs and counters for {len(archive['instances'])} development instances")


if __name__ == '__main__':
    main()
