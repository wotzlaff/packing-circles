# Version A numerical parity results

Milestone 3 passed on 2026-11-16 for implementation commit `f7d1d3e`.
The [complete report](../bench/results/numerical-parity.json) archives input packets,
C++/Rust output, per-iteration traces, source/binary hashes, and compiler versions.
Reproduction commands are in [bench/README.md](../bench/README.md).

## Results

| Component | Fixed comparisons | What is compared |
| --- | ---: | --- |
| Objective/gradient | 129 | Scalar-only and gradient evaluator; includes a negative trial radius |
| Radial repair | 128 | Minimum pair scale, repaired coordinates, actual enclosing radius |
| Strong-Wolfe search | 32 | Step length and objective; both prior-value and default initialization |
| Dense BFGS | 32 | Eight-step limit; final state/gradient and every callback state/value/norm |
| BFGS with adaptive penalties | 32 | Same fixed-work checks plus coefficients before every callback update |
| Penalty scheduling | 73 | Independent constraint updates, no-update/equality cases, coincidence, zero/negative trial radius |
| **Total** | **426** | **No failed comparisons** |

The gate is componentwise absolute `1e-9` plus relative `1e-8`. The largest
observed difference used **5.21e-8 of that allowance**. Separate central finite
differences on **128 states** had maximum relative gradient error **2.02e-8**,
below the required `1e-5`. Generated states explicitly avoid penalty kinks;
activation counts confirm active and inactive terms for both constraint families.

The C++ probe includes `circles-hackbfgs.tr.cpp` at legacy revision
`33a13c04c6b27a9be1b87db9a2a5bbc82c7ecbc0`, without modifying it. It calls the
original objective, line search, BFGS, and repair helpers directly. Scheduler
wrappers reproduce the production callback without running the unsafe initializer.
Measurements used GCC 12.2.0 (`-O2 -std=c++17`) and Rust 1.100.0 release builds.

## Regression coverage and limits

The workspace has **49 passing Rust tests** and the benchmark tooling has **21
passing Python tests**. Tests exercise the full development set's initializer
with two seeds, tangency/overlap/containment, finite arithmetic, line-search
exhaustion, callback boundaries, evaluation/iteration limits, cancellation,
subnormal BFGS curvature, zero-gradient convergence, and output rounding loss.

CI replays archived C++ output against the current Rust probe, so it requires no
legacy checkout. This guards future kernel changes against these fixed examples;
it is not a universal proof of numerical equivalence. Generated packets and full
traces remain available rather than replacing evidence with only a pass count.

Initialization is intentionally not compared through the original C++ entry point:
its confirmed invalid memory read has no meaningful Rust equivalent. RNG changes
and bounded safe recovery are documented in [version-a.md](version-a.md). Likewise,
the controlled numerical failures tested in Rust do not claim to reproduce C++
undefined behavior. No held-out inputs were evaluated.

This milestone qualifies components. End-to-end solving, timely cancellation,
serialized incumbent selection, radius quality, and wall-clock performance remain
milestone 4; these results make no claim about them.
