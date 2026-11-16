#!/usr/bin/env python3
"""Compare complete development B cohorts against the archived A qualification."""
import argparse
from collections import defaultdict
import json
import math
from pathlib import Path
import random
import statistics

from compare import _valid


def report(a_path, b_path):
    a_rows = [json.loads(line) for line in Path(a_path).read_text().splitlines()]
    b_rows = [json.loads(line) for line in Path(b_path).read_text().splitlines()]
    groups = defaultdict(list)
    for row in a_rows:
        if row['returncode'] != 0 or row['timeout'] or row['overrun'] or not row['validation']['feasible']:
            raise ValueError('invalid historical A run')
        key = ('a', row['budget_seconds'], Path(row['instance']).name)
        groups[key].append(row)
    for row in b_rows:
        if not _valid(row):
            raise ValueError(f"invalid B run {row['run_id']}")
        key = (row['solver'], row['budget_seconds'], Path(row['instance']).name)
        groups[key].append(row)
    names = sorted({r['solver'] for r in b_rows})
    budgets = [0.1, 0.5, 1.95]
    instances = sorted({Path(r['instance']).name for r in a_rows})
    if len(instances) != 50 or len(a_rows) != 750 or len(b_rows) != len(names) * 750:
        raise ValueError('incomplete cohort')
    lines = ['# Version B development comparisons', '',
             'Per-instance medians of five seeds; ratios compare against historical A. '
             'Bootstrap uses 10,000 resamples of the 50 instance ratios, seed 20261116.', '',
             '| Candidate | Budget (s) | Geometric mean B/A | 95% bootstrap interval | Worst ratio | Regressions |',
             '| --- | ---: | ---: | ---: | ---: | ---: |']
    details = []
    for name in names:
        for budget in budgets:
            ratios = []
            regressions = []
            for instance in instances:
                a, b = groups[('a', budget, instance)], groups[(name, budget, instance)]
                for batch in (a, b):
                    if len(batch) != 5 or {r['seed'] for r in batch} != set(range(5, 10)):
                        raise ValueError(f'incomplete seeds: {name} {budget} {instance}')
                if {r['input_sha256'] for r in a} != {r['input_sha256'] for r in b}:
                    raise ValueError(f'input hash mismatch: {instance}')
                ratio = statistics.median(r['validation']['enclosing_radius'] for r in b) / statistics.median(r['validation']['enclosing_radius'] for r in a)
                ratios.append(ratio)
                if ratio > 1:
                    regressions.append((instance, ratio))
            logs = [math.log(x) for x in ratios]
            gm = math.exp(statistics.mean(logs))
            rng = random.Random(20261116)
            boot = sorted(math.exp(statistics.mean(rng.choices(logs, k=50))) for _ in range(10000))
            lines.append(f'| {name} | {budget:g} | {gm:.8f} | [{boot[249]:.8f}, {boot[9749]:.8f}] | {max(ratios):.8f} | {len(regressions)} |')
            details.extend(['', f'## {name}, {budget:g} s regressions', '',
                            ', '.join(f'{instance}: {ratio:.8f}' for instance, ratio in regressions) or 'None.'])
    return '\n'.join(lines + details) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('a')
    parser.add_argument('b')
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    Path(args.output).write_text(report(args.a, args.b))


if __name__ == '__main__':
    main()
