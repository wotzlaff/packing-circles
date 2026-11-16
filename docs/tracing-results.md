# Optimization trace and comparison results

**Milestone 5 passed on 2026-11-16.** The observer, CLI JSONL writer, independent
replayer, configured comparison runner, and overhead protocol are implemented.
The measured median runtime increase was **3.25%**, below the declared **5%** gate.

## Fixed-work overhead study

The study used all 50 frozen development inputs, three seeds (5–7), and 50,000
objective evaluations per solve. Each input/seed ran once plain and once traced:
**150 pairs / 300 measured processes**. Pair order alternated by input and
repetition. One unmeasured warmup per mode preceded collection.

For each pair, divide traced external process time by plain time. Take the median
of three ratios per input, then the median across the 50 inputs. This includes CLI
startup, JSON encoding, file creation/write/flush, and solver work, rather than
only timing the callback. Traces cap ordinary updates at 10 Hz, with initial/final
exceptions. The observed dataset contained **449 snapshots**, including **149
ordinary incumbent events**.

| Circles | Inputs | Pairs | Median runtime increase |
| ---: | ---: | ---: | ---: |
| 10 | 10 | 30 | +3.53% |
| 20 | 10 | 30 | +3.88% |
| 30 | 10 | 30 | +2.55% |
| 40 | 10 | 30 | +2.04% |
| 50 | 10 | 30 | +3.21% |
| All | 50 | 150 | **+3.25%** |

All paired outputs were byte-identical and all evaluation/iteration/restart,
fallback, recovery, and termination counters matched. Every requested seed/work
limit matched the summaries and trace headers; every run reached its evaluation
limit. Every snapshot passed independent strict geometry and export checks, and
all final frames matched stdout and the run counters. The full cohort passed.

Measurement used Rust 1.100.0 release builds on an Intel Core i7-8700/Linux host.
Local compilation was paused; external load was uncontrolled. This measures the
default CLI writer and these fixtures, not the cost of arbitrary observer callbacks
or browser rendering. The complete [raw report](../bench/results/trace-overhead.json)
stores both runs, timing ratios, counters, provenance, and replay results. All
[150 trace files](../bench/results/trace-overhead.traces/) are retained; for example,
[50 circles, seed 5](../bench/results/trace-overhead.traces/50_01-rep0.jsonl).

## Regression and unified-runner checks

A separate [50-input regression](../bench/results/observer-regression.json) compared
the new plain binary against the preserved, qualified version A binary at 5,000
evaluations/seed 5. Every output and work counter matched. The report records both
binary hashes and all input/output hashes.

The configured [legacy/A demonstration](../bench/results/unified-legacy-a.jsonl)
ran five development inputs, two budgets, and two repetitions per solver:
**40 valid runs**, with **20 valid Rust traces**. Its
[Markdown tables](../bench/results/unified-legacy-a.md) reproduce exactly via one
summary command. Legacy executables expose only final output, so their progress is
explicitly unavailable. These smoke comparisons exercise the runner; version A's
full quality qualification remains the [milestone 4 report](version-a-results.md).

The runner accepts alternate executable/source paths and explicit algorithm
descriptors, preserves failures and deadline violations, and excludes incomplete
or failed repetitions from comparative radius statistics. Configuring B against
the current A-only CLI records a failure; it does not silently run A as B.

## Validation and next step

All **73 Rust tests** and **45 Python tests** pass. The **426 archived C++ component
comparisons and 128 finite-difference checks** still pass. CI exercises an actual
CLI trace through the unified runner and independent replayer, without a legacy
checkout. Reproduction commands are in [bench/README.md](../bench/README.md).

No held-out inputs were evaluated. Next is milestone 6: version B experiments and
ablations. The current observer records history and supports cooperative stopping;
preserving an in-progress Hessian/line-search for pause/resume remains milestone 7.
