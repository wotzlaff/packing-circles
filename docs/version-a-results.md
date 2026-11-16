# Version A qualification results

**Milestone 4 passed on 2026-11-16.** Version A provides native solving, checked
incumbents, cooperative cancellation, deterministic fixed-work execution, and
machine-readable run summaries. See [README.md](../README.md#solve-an-instance)
for commands and [the fidelity contract](version-a.md) for safe deviations.

## Measured results

| Budget | Geometric mean radius ratio A/legacy | Worst instance ratio | Valid/on-time runs |
| ---: | ---: | ---: | ---: |
| 0.1 s | 1.000829 (+0.083%) | 1.015620 (+1.562%) | 250/250 |
| 0.5 s | 1.001542 (+0.154%) | 1.009099 (+0.910%) | 250/250 |
| 1.95 s | 1.001543 (+0.154%) | 1.008245 (+0.824%) | 250/250 |

The primary 1.95-second gate required geometric mean ≤ 1.01 and worst instance
≤ 1.05. All budgets required complete exact coverage, feasible output, and no
process-time overrun beyond budget + `max(20 ms, 10% of budget)`. **All gates passed.**
The shorter budgets have no separate radius-regression threshold.

Ratios first take the median of five runs for each instance/algorithm, then
compute A/legacy per instance. The table reports the geometric mean and maximum
of those 50 ratios, not pooled run medians. This qualifies legacy-level quality;
it is not an improvement claim for version B.

## Protocol and evidence

- 50 frozen synthetic development instances, ten each with 10/20/30/40/50 circles.
  Each instance ran five times at 0.1/0.5/1.95 seconds: **750 Rust runs** compared
  with the prior **750 legacy runs**. Rust used seeds 5–9; C++ restarts each process
  with its fixed seed sequence. There is no paired RNG-stream claim.
- Rust release binary/source at implementation `4d39933`, harness `0d8eae3`;
  metadata records their exact hashes and toolchain. Measurements used an Intel
  Core i7-8700, Linux 6.1.0-44-amd64, and Rust 1.100.0. The legacy archive used
  GCC 12.2.0 with `-O2 -std=c++17` on the same recorded CPU/OS.
- Local compilation stayed paused. All measured binary/source hashes remained
  unchanged; external machine load was uncontrolled. The legacy measurements
  are an archived baseline, not a contemporary interleaved rerun.
- Every output passed independent geometric checks. Completeness, unique
  input/budget/repetition coverage, input hashes, declared work caps, summary
  agreement, and raw-output validation were verified after collection.
- There were no crashes, timeouts, numerical recoveries, or initialization
  fallbacks in these runs. Maximum observed process overhead beyond the requested
  budget was 33.67 ms; every run remained within its budget-specific allowance.
- The 50 held-out inputs were not evaluated. The unmodified C++ baseline has a
  confirmed invalid memory read, so these are observations against that specific
  build/environment, not proof of universal numerical identity or runtime bounds.

Artifacts: [raw runs](../bench/results/version-a-development.jsonl),
[provenance](../bench/results/version-a-development.metadata.json),
[derived statistics](../bench/results/version-a-development.summary.json), and
[full tables by size](../bench/results/version-a-development.md).

## Validation and next step

All **68 Rust tests** and **26 Python tests** pass. Coverage includes all 50
development instances, zero/tiny budgets, independent work caps, cancellation
before/during optimization, exact fixed-work repeatability, and rounded output.
The **426 C++ component comparisons and 128 finite-difference checks** still pass
after assembly, and [implementation CI passed](https://github.com/wotzlaff/packing-circles/actions/runs/43852864197).

Next is milestone 5: optional incumbent events, replayable improvement histories,
a unified comparison runner, and a measured trace-overhead gate. The current
solver can stop cooperatively; preserving a running BFGS session for pause/resume
remains milestone 7. See [trace design](trace-design.md).
