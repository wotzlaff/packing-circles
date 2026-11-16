# Reproducible legacy baseline

These tools use Python 3.10+ and GCC with C++17 support. They never edit `legacy/`.

```sh
python3 bench/generate_instances.py --check
python3 -m unittest discover -s bench -v
python3 bench/legacy.py build --variant contest
python3 bench/legacy.py build --variant main
python3 bench/run_baseline.py run --variant contest --output bench/results/contest-development.jsonl
python3 bench/run_baseline.py summarize bench/results/contest-development.jsonl
```

The full candidate baseline is 750 serial processes: 50 development inputs,
three budgets (0.1/0.5/1.95 seconds), five repetitions. Expected solver time is
about 11 minutes plus process overhead. Avoid concurrent builds or heavy work
while measuring. External machine load is not controlled, and is disclosed in
metadata. Held-out inputs are generated and hashed but not run during baseline
development. Do not tune against them.

Output files are created exclusively, never overwritten. Each JSONL row preserves
stdout, stderr, exit status, timeout, measured end-to-end process time, budget,
input hash, and independent output validation. Companion metadata records build
and machine details. Legacy evaluation counts/internal geometry are unavailable
and are not invented. A fixed C++ seed does not make timed runs deterministic.

The summary includes failed runs in counts, excludes them from quality statistics,
and reports deadline overruns separately. Both strict geometric feasibility and
four-decimal rounding allowances are recorded; allowances never promote an
invalid packing to a successful result.

For a quick check use `--limit 1 --budgets 0.1 --repetitions 1` with a new output
path. To check a saved solution independently:

```sh
python3 bench/check_solution.py INSTANCE.txt SOLUTION.txt
```

To reproduce the confirmed memory error outside the release benchmark:

```sh
python3 bench/legacy.py build --variant contest --sanitize
bench/build/contest-asan 1e-7 1e-5 1e-5 1e-5 1 0.1 < bench/data/development/10_01.txt
```

See [the algorithm map](../docs/legacy-baseline.md) for the safe-port implications.
