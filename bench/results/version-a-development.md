# Version A qualification

Status: **QUALIFIED**.

Runs: 750 / 750; all valid: True.
Legacy archive SHA-256: `1b4c3a34f3a96b04a470943c78f3980b7106a52f1bfc06b30bccc0bae6756c83`. This is a recorded baseline, not a contemporary rerun.
Baseline hardware: Intel(R) Core(TM) i7-8700 CPU @ 3.20GHz (Linux-6.1.0-44-amd64-x86_64-with-glibc2.36).
Legacy contest implementation has sanitizer-confirmed undefined behavior; results are historical evidence.
External machine load was uncontrolled during baseline collection.

| Budget (s) | Instances | Geometric mean of instance median radius ratios | Worst instance ratio | Gate |
| ---: | ---: | ---: | ---: | --- |
| 0.1 | 50 | 1.000829 | 1.015620 | PASS |
| 0.5 | 50 | 1.001542 | 1.009099 | PASS |
| 1.95 | 50 | 1.001543 | 1.008245 | PASS |

## Radius ratios by size

Each size cell reports the geometric mean and worst per-instance median ratio.

| Circles | Budget (s) | Instances | Geometric mean | Worst instance |
| ---: | ---: | ---: | ---: | ---: |
| 10 | 0.1 | 10 | 0.999856 | 1.006410 |
| 20 | 0.1 | 10 | 1.001824 | 1.007005 |
| 30 | 0.1 | 10 | 1.001568 | 1.015620 |
| 40 | 0.1 | 10 | 1.001939 | 1.013177 |
| 50 | 0.1 | 10 | 0.998963 | 1.004831 |
| 10 | 0.5 | 10 | 1.001480 | 1.006605 |
| 20 | 0.5 | 10 | 1.002882 | 1.006370 |
| 30 | 0.5 | 10 | 1.000917 | 1.009099 |
| 40 | 0.5 | 10 | 1.002216 | 1.006598 |
| 50 | 0.5 | 10 | 1.000218 | 1.008727 |
| 10 | 1.95 | 10 | 1.002177 | 1.008245 |
| 20 | 1.95 | 10 | 1.001956 | 1.007192 |
| 30 | 1.95 | 10 | 1.000540 | 1.007427 |
| 40 | 1.95 | 10 | 1.000783 | 1.006414 |
| 50 | 1.95 | 10 | 1.002258 | 1.005725 |

Ratios use enclosing radius and are computed by taking each instance's median across five repetitions first, then the geometric mean across instances.
The 1.95 s gate requires geometric mean ≤ 1.01, worst instance ≤ 1.05, complete exact coverage, 100% feasible outputs, and no deadline overruns.
Deadline allowance: budget + max(20 ms, 10% of budget).
