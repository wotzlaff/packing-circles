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

## Numerical component parity

The fixed-state probe includes the existing C++ source with its `main` renamed;
it calls numerical kernels directly without executing the unsafe initializer.
Build and rerun the complete comparison with a new output filename:

```sh
g++ -O2 -std=c++17 bench/reference_probe.cpp -o bench/build/reference-probe
cargo build -p packing-core --release --example numerical_probe --locked
python3 bench/check_numerical_parity.py --output /tmp/new-numerical-parity.json
```

Create `bench/build/` first if no legacy binary has been built. Both probes consume
the same packets: count, radii, two weights, `[R,x,y,...]` state and direction,
previous-value flag/value, Wolfe constants, iteration limit, gradient tolerance,
callback gradient tolerance, violation tolerance, and sample squared gradient.
Modes are `objective`, `line-search`, `bfgs`, `adaptive`, `schedule`, and `repair`.
The scheduler wrapper reproduces the callback from the reference `main`; the
other modes invoke the original C++ helpers. Its single-circle scale is normalized
to one for reporting; the archived repair comparisons use at least ten circles.

The [archived report](results/numerical-parity.json) contains exact input packets,
both outputs, callback traces, input/source/binary hashes, toolchain versions,
activation coverage, and finite-difference errors. Failed comparisons exit nonzero
and remain in the report. Random states use development radii only and explicitly
avoid penalty kinks. Semantic scheduler cases assert each intended branch.

CI can replay all C++ oracles without a legacy checkout or compiler:

```sh
python3 bench/check_numerical_parity.py --replay-report bench/results/numerical-parity.json
```

Build the release Rust probe first. Replay checks every scalar and callback event,
all input hashes/counts, and at least 100 independent finite-difference states.
See [the numerical result summary](../docs/numerical-parity.md) for scope and limits.

## Version A qualification

```sh
cargo build -p packing-cli --release --locked
python3 bench/qualify_version_a.py run --output /tmp/new-version-a.jsonl
python3 bench/qualify_version_a.py summarize bench/results/version-a-development.jsonl
```

Collection uses 50 development inputs, three budgets, and five Rust seeds (5–9),
taking about 11 minutes. It requires the same CPU/OS recorded in the frozen legacy
baseline; summarizing archived results works on other machines. Use a fresh output
path: the raw archive and metadata are never overwritten. `--limit 1 --budgets 0.1
--repetitions 1` is a smoke run and cannot qualify as a full benchmark.

The runner preserves solutions, JSON diagnostics, crashes/timeouts, external
elapsed time, source/binary hashes, and input provenance. Malformed summaries,
wrong seeds, incomplete coverage, invalid output, or deadline violations cannot
pass. Quality compares per-instance median radii before geometric aggregation.
The [qualification report](../docs/version-a-results.md) states thresholds and
limitations; the comparison uses the recorded legacy archive, not a fresh rerun.
