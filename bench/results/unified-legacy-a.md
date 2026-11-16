# Solver comparison

Generated from `unified-legacy-a.jsonl`. Invalid outputs and deadline overruns remain visible and do not enter quality statistics.

| Solver | Circles | Budget (s) | Runs | Valid | Overruns | Median elapsed (s) | Median radius |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| legacy | 10 | 0.1 | 2 | 2 | 0 | 0.101420 | 215.5579 |
| legacy | 20 | 0.1 | 2 | 2 | 0 | 0.101563 | 347.2555 |
| legacy | 30 | 0.1 | 2 | 2 | 0 | 0.104473 | 408.92345 |
| legacy | 40 | 0.1 | 2 | 2 | 0 | 0.102996 | 402.2759 |
| legacy | 50 | 0.1 | 2 | 2 | 0 | 0.103628 | 506.6575 |
| legacy | 10 | 0.5 | 2 | 2 | 0 | 0.501445 | 215.7291 |
| legacy | 20 | 0.5 | 2 | 2 | 0 | 0.503382 | 345.7553 |
| legacy | 30 | 0.5 | 2 | 2 | 0 | 0.504169 | 407.1288 |
| legacy | 40 | 0.5 | 2 | 2 | 0 | 0.502652 | 399.09 |
| legacy | 50 | 0.5 | 2 | 2 | 0 | 0.503376 | 508.8287 |
| rust-a | 10 | 0.1 | 2 | 2 | 0 | 0.101198 | 216.74105 |
| rust-a | 20 | 0.1 | 2 | 2 | 0 | 0.101114 | 346.0157 |
| rust-a | 30 | 0.1 | 2 | 2 | 0 | 0.102685 | 413.3599 |
| rust-a | 40 | 0.1 | 2 | 2 | 0 | 0.101352 | 403.84475 |
| rust-a | 50 | 0.1 | 2 | 2 | 0 | 0.101226 | 507.47165 |
| rust-a | 10 | 0.5 | 2 | 2 | 0 | 0.501104 | 215.1294 |
| rust-a | 20 | 0.5 | 2 | 2 | 0 | 0.503267 | 345.82955 |
| rust-a | 30 | 0.5 | 2 | 2 | 0 | 0.502574 | 407.29685 |
| rust-a | 40 | 0.5 | 2 | 2 | 0 | 0.501308 | 399.5736 |
| rust-a | 50 | 0.5 | 2 | 2 | 0 | 0.504830 | 506.6876 |

## Per-instance ratios at 0.1 s

Ratios use per-instance medians against the first configured solver. Only common instances with every expected repetition valid and on time enter comparisons.

| Solver | Common instances | Geometric mean radius ratio |
| --- | ---: | ---: |
| legacy | 5 | 1.00000000 |
| rust-a | 5 | 1.00364377 |

## Per-instance ratios at 0.5 s

Ratios use per-instance medians against the first configured solver. Only common instances with every expected repetition valid and on time enter comparisons.

| Solver | Common instances | Geometric mean radius ratio |
| --- | ---: | ---: |
| legacy | 5 | 1.00000000 |
| rust-a | 5 | 0.99896811 |
