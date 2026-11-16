#!/usr/bin/env python3
"""Compare fixed-work A to its archive or a reference built on the same host."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-binary', type=Path,
                        help='compare to this native reference on the same machine')
    parser.add_argument('--report', type=Path, help='retain all outputs and failures as JSON')
    args = parser.parse_args()
    archive = json.loads(Path('bench/results/observer-regression.json').read_text())
    rows = []
    failures = []
    for record in archive['instances']:
        data = Path(record['instance']).read_bytes()
        if hashlib.sha256(data).hexdigest() != record['input_sha256']:
            raise ValueError(f"input hash mismatch: {record['instance']}")
        options = ['solve', '-', '--algorithm', 'a', '--seed', str(archive['seed']),
                   '--fixed-work', '--evaluations', str(archive['evaluation_limit'])]
        expected_hash = record['output_sha256']
        expected_counts = record['counts']
        expected_output = None
        if args.reference_binary:
            reference = subprocess.run([str(args.reference_binary.resolve()), *options],
                                       input=data, capture_output=True, check=True)
            expected_hash = hashlib.sha256(reference.stdout).hexdigest()
            reference_summary = json.loads(reference.stderr)
            expected_counts = {key: reference_summary[key] for key in record['counts']}
            expected_output = reference.stdout.decode()
        result = subprocess.run(['target/release/packing-cli', *options],
                                input=data, capture_output=True, check=True)
        summary = json.loads(result.stderr)
        actual_hash = hashlib.sha256(result.stdout).hexdigest()
        differences = []
        if actual_hash != expected_hash:
            differences.append('output')
        for key, expected in expected_counts.items():
            if summary[key] != expected:
                differences.append(key)
        rows.append({'instance': record['instance'], 'input_sha256': record['input_sha256'],
                     'output': result.stdout.decode(), 'output_sha256': actual_hash,
                     'expected_output': expected_output, 'expected_output_sha256': expected_hash,
                     'counts': {key: summary[key] for key in expected_counts},
                     'expected_counts': expected_counts, 'differences': differences})
        if differences:
            failures.append(f"{record['instance']}: {', '.join(differences)}")
    if args.report:
        args.report.write_text(json.dumps({'reference_binary': str(args.reference_binary) if args.reference_binary else None,
                                          'seed': archive['seed'], 'evaluation_limit': archive['evaluation_limit'],
                                          'passed': not failures, 'instances': rows}, indent=2) + '\n')
    if failures:
        raise SystemExit('Regression mismatches:\n' + '\n'.join(failures))
    label = 'same-host reference' if args.reference_binary else 'archive'
    print(f"Matched {label} outputs and counters for {len(rows)} development instances")


if __name__ == '__main__':
    main()
