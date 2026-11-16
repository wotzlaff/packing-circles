# Version A fidelity contract (before implementation)

## Reference and scope

Use `legacy/circles-hackbfgs.tr.cpp` at legacy revision
`33a13c04c6b27a9be1b87db9a2a5bbc82c7ecbc0` as the working reference. This selection
is accepted for project work; it is not proof of the historical submission.
See [selection evidence](legacy-selection.md) and [source map](legacy-baseline.md).

Version A ports the explicit-radius squared-violation objective, analytic gradient,
rolling initialization, dense BFGS, strong-Wolfe search, callback-driven penalty
updates, restarts, feasibility scaling, and best repaired incumbent selection.
`main.cpp`'s softmax formulation is a secondary benchmark, not the definition of A.

No Rust optimizer is implemented yet. The following decisions constrain milestone
3; record numerical experiments before accepting additional deviations.

## Historical behavior to retain

- Keep default penalty values, gradient-square/violation thresholds, Wolfe
  constants, and one initialization trial as mapped in `legacy-baseline.md`.
- Preserve the 20-step centering schedule, including the effectively zero previous
  momentum state, until a separately measured B experiment changes it.
- Preserve the current last-trial behavior for multi-trial initialization in A;
  fixing the missing best-value update is a candidate B change. The default uses
  one trial, where this distinction has no effect.
- Compare objective and gradient on identical finite states. Do not assert
  bitwise full-solver parity across RNG implementations or timed termination.
- Validate actual repaired geometry and reparsed output independently. Exported
  results must be feasible even if the reference fails on the same input.

## Required safe departures

| Reference issue | Rust behavior | Evidence required |
| --- | --- | --- |
| Full-instance centering reads partially constructed positions out of bounds | Pass only the placed radii and coordinates to centering | Prefix-initialization tests; sanitizer failure archived for the reference |
| Rolling candidate missing, invalid `acos` input, or degenerate geometry | Detect failure and use a documented finite fallback placement or restart | Explicit degenerate fixtures; count and report fallbacks |
| Deadline before any optimized solution | Retain a valid initial packing before optimization starts | Zero/tiny-budget tests with finite feasible output |
| Zero/invalid BFGS curvature or nonfinite search state | Stop/reset according to an explicit policy and keep the feasible incumbent | Fixed-state regression probes and failure-path tests |
| Line-search exhaustion without a defined accepted step | Return an explicit failure instead of using an uninitialized step | Exhaustion fixture; no invalid state can enter BFGS |
| Pair scaling undefined for one circle/coincident centers | Handle one circle directly and recover coincident geometry explicitly | One-circle optimum and coincident-center tests |
| Rounding erases geometric slack | Validate rounded output; repair explicitly or refuse export | Diagonal-tangency regression plus round-trip validation |

The initializer memory bug makes the unmodified C++ results observational. There
is no meaningful requirement to reproduce values read outside a vector. Assess
numerical parity using valid component inputs, and assess end-to-end quality with
the measured, disclosed initialization difference.

## Validation sequence

1. Add a root-owned C++ probe including the unmodified source with a renamed
   `main`; expose objective/gradient on supplied states without initialization.
2. Generate at least 100 fixed nondegenerate states, including active/inactive
   containment and pair penalties. Compare with plan tolerances and central
   finite differences away from kinks.
3. Compare strong-Wolfe and BFGS steps with fixed starting states/work limits.
   Exercise penalty callbacks independently of a wall clock.
4. Integrate initialization, repair, cancellation, and the CLI. Only then run
   full A-versus-reference timing and quality qualification.

Update this document with implemented policies, probe commands, and measured
results as each component lands. A numerical-port gate has not passed yet.
