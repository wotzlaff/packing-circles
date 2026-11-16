# Legacy baseline results

Generated from `contest-development.jsonl`.

The unmodified contest candidate has a sanitizer-confirmed out-of-bounds read.
These results describe observed behavior, not reproducibility or memory safety.
See [the source map](../../docs/legacy-baseline.md).

Validation covers serialized output only; internal states are unavailable.
Each cell groups ten instances × five repetitions in the full protocol.
Quality ratios include successful, strictly feasible runs only.

| Variant | Budget (s) | N | Runs | Valid | Crashes | Timeouts | Overruns | Median elapsed (s) | Median area ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| contest | 0.1 | 10 | 50 | 50 | 0 | 0 | 0 | 0.101378 | 1.136703 |
| contest | 0.1 | 20 | 50 | 50 | 0 | 0 | 0 | 0.101635 | 1.128124 |
| contest | 0.1 | 30 | 50 | 50 | 0 | 0 | 0 | 0.102036 | 1.123388 |
| contest | 0.1 | 40 | 50 | 50 | 0 | 0 | 0 | 0.102657 | 1.125537 |
| contest | 0.1 | 50 | 50 | 50 | 0 | 0 | 0 | 0.103404 | 1.121389 |
| contest | 0.5 | 10 | 50 | 50 | 0 | 0 | 0 | 0.501400 | 1.130561 |
| contest | 0.5 | 20 | 50 | 50 | 0 | 0 | 0 | 0.501661 | 1.122440 |
| contest | 0.5 | 30 | 50 | 50 | 0 | 0 | 0 | 0.502039 | 1.122268 |
| contest | 0.5 | 40 | 50 | 50 | 0 | 0 | 0 | 0.502541 | 1.117382 |
| contest | 0.5 | 50 | 50 | 50 | 0 | 0 | 0 | 0.503494 | 1.116039 |
| contest | 1.95 | 10 | 50 | 50 | 0 | 0 | 0 | 1.951417 | 1.124133 |
| contest | 1.95 | 20 | 50 | 50 | 0 | 0 | 0 | 1.951636 | 1.118268 |
| contest | 1.95 | 30 | 50 | 50 | 0 | 0 | 0 | 1.952108 | 1.117055 |
| contest | 1.95 | 40 | 50 | 50 | 0 | 0 | 0 | 1.952579 | 1.114756 |
| contest | 1.95 | 50 | 50 | 50 | 0 | 0 | 0 | 1.953516 | 1.111446 |

Overrun threshold: budget + max(20 ms, 10% of budget).
Crashes, invalid output, and timeouts are retained in the raw JSONL.
Area ratios are lower-bound ratios, not proof of optimality.
