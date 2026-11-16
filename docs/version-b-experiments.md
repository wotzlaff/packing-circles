# Version B development experiments

The following three single-policy candidates are frozen before measurement. All
retain A's rolling initialization, objective, repair, work limits, and incumbent
selection. They are experimental and require explicit `--algorithm b --variant`.

- `early-penalty`: raise the squared-gradient threshold for penalty updates from
  `1e-5` to `1e-3`. Hypothesis: refining feasibility earlier reduces wasted local
  optimization work; it may instead bias optimization away from compact basins.
- `tight-penalty`: lower the relative violation threshold from `1e-5` to `1e-6`.
  Hypothesis: more constraint refinement reduces repair expansion; extra work may
  crowd out restarts.
- `short-restarts`: cap each BFGS local search at 100 iterations. Hypothesis:
  exploring more initial packings improves best-of-run quality; early truncation
  may prevent promising basins from converging.

The configured serial development study uses 50 frozen development instances,
three budgets, and seeds 5–9. The comparison is to the historical A archive,
not contemporaneous paired execution. No held-out data is included. Results and
limitations will be added after the study; release qualification remains pending.

## Results

The serial batch completed on 2026-11-16: 50 development instances × three
budgets × five seeds × three candidates, **2,250 runs**. Each output passed the
independent geometry and rounded-output checks. No executable or source hash
changed during a run. Six 0.1-second runs exceeded the deadline allowance; all
remain in the archive. No run timed out or crashed. Five runs (all
`tight-penalty`) reported one recovered numerical failure each; none had an
initialization fallback. Maximum observed process overrun above the requested
budget was 64.65 ms. These events are retained with their input, seed, counters,
and output in the raw data.

| Candidate | Budget | Complete instances | Geometric mean B/A | 95% bootstrap interval | Worst instance ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| early-penalty | 0.1 s | 49/50 | 1.000618 | [1.000065, 1.001243] | 1.010369 |
| early-penalty | 0.5 s | 50/50 | 0.999531 | [0.999169, 0.999885] | 1.002062 |
| early-penalty | 1.95 s | 50/50 | 0.999769 | [0.999313, 1.000138] | 1.001863 |
| tight-penalty | 0.1 s | 49/50 | 1.000730 | [1.000243, 1.001249] | 1.006651 |
| tight-penalty | 0.5 s | 50/50 | 0.999694 | [0.999457, 0.999915] | 1.001817 |
| tight-penalty | 1.95 s | 50/50 | 0.999958 | [0.999703, 1.000203] | 1.002242 |
| short-restarts | 0.1 s | 47/50 | 1.015757 | [1.013423, 1.018130] | 1.038543 |
| short-restarts | 0.5 s | 50/50 | 1.011714 | [1.009740, 1.013708] | 1.025572 |
| short-restarts | 1.95 s | 50/50 | 1.010541 | [1.008726, 1.012298] | 1.020921 |

Ratios are B/A after taking each instance's median of five output radii. The
confidence interval is the percentile interval of 10,000 bootstrap resamples of
instances, deterministic seed 20261116. An instance with any failed or late run
is excluded from that budget's primary ratio; all excluded late cohorts have
separate diagnostic ratios in the [complete comparison](../bench/results/version-b-development-comparison.md),
which lists every regression. The [runner summary](../bench/results/version-b-development-summary.md)
provides per-size counts and timings. No candidate reaches the required 0.99
geometric mean at 1.95 s, and both penalty variants' confidence intervals include
1. Short restarts also exceeds the 2% worst-regression gate. These are negative
development results, not release qualification.

## Reproduction and limitations

The frozen candidate code and configuration were committed as `5d726d9` before
collection. The release executable SHA-256 was
`26ccf75fc8f4e715937923e8de38557265925c82a0bbb0a70b0344bf4d2fe271`;
the [configuration](../bench/configs/version-b-development.json) hash was
`abaa76029c5526e0e627bbcd65f29b9f9fa69cc051face127e4f4b69f4a98e7f`.
The [raw results](../bench/results/version-b-development.jsonl) hash is
`8e8c7d942a4a7e080898c156ad348ff73214f1287c3c8b25b78321d27831e2a1`.
[Metadata](../bench/results/version-b-development.metadata.json) and each row
record toolchains, hardware, input, configuration, source and executable hashes,
commands, seeds, work counters and timings. The source and binary hashes were
checked around each run. The archived A comparison has SHA-256
`89ff1deaf05e9a3d2ed16794ecb45643e225510776f94865eeb3df86200d98bb`.

```sh
cargo build -p packing-cli --release --locked
python3 bench/compare.py run --config bench/configs/version-b-development.json --output /tmp/new-b.jsonl
python3 bench/compare.py summarize bench/results/version-b-development.jsonl --output /tmp/b-summary.md
python3 bench/report_version_b.py bench/results/version-a-development.jsonl bench/results/version-b-development.jsonl --output /tmp/b-comparison.md
```

Runs were serial on the recorded Intel i7-8700/Linux host, with compilation paused.
The A archive is historical rather than contemporaneously interleaved; external
load was uncontrolled and the host load average was elevated during the observed
short-budget overruns. This can affect time-limited quality and limits causal
interpretation. The 0.1-second tables have incomplete on-time cohorts. The
synthetic development set has been reused for tuning and does not estimate
performance on the untouched held-out set.

The integrated code passed workspace build/tests, rustfmt and Clippy; all 47
Python tests and frozen-data checks passed. The archived C++ oracle replay passed
426 cases and 128 finite-difference states. A's archived fixed-work outputs and
counters matched on all 50 development inputs. All three B candidates passed
fixed-work determinism, work-limit, cancellation, geometry and trace checks; a
traced CLI smoke run independently replayed each variant.

## Recommendation

Freeze `early-penalty` at commit `5d726d9` as the best of these three measured
experimental configurations, not as a release candidate: its 1.95-second mean
improved only 0.023% and its interval includes 1. The other two trials are
retained as unsuccessful ablations. Do not spend the held-out evaluation on this
result. Next, investigate corrected rolling initialization/centering and a
bounded alternative optimizer or restart allocation with fresh development
ablations, ideally using a contemporaneous A baseline on a quieter host. Freeze
a stronger configuration before any held-out qualification. The eventual release
still requires at least 1% mean reduction, worst median regression at most 2%,
100% feasible outputs and bootstrap upper bound below 1.
