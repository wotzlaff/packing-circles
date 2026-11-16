# Implementation decisions

## Baseline and initial architecture

- Keep the original checkout ignored at `legacy/`. Root-owned wrappers compile
  its sources into ignored `bench/build/` without changing the reference files.
- Adopt `circles-hackbfgs.tr.cpp` as the working candidate after the source-selection
  investigation; historical submission provenance remains unproven. Retain
  `main.cpp` as a separate secondary baseline, not the algorithm to port as A.
- Generate development and held-out datasets with a specified SHA-256 byte stream
  instead of relying on language RNG implementations. Commit the actual inputs
  and per-instance hashes before tuning. Do not evaluate held-out data yet.
- Use Python's standard library for independent baseline validation and process
  measurements. This checker is deliberately separate from the future Rust solver.
  Preserve failures and check serialized output without silently repairing it.
- The candidate contains an AddressSanitizer-confirmed out-of-bounds initializer
  read. Preserve the unmodified C++ baseline as observational evidence. The Rust
  initializer will need bounded slices; compare valid numerical kernels on fixed
  inputs and explicitly document the initialization departure.
- Keep the initial Rust workspace dependency-free: `packing-core` owns geometry,
  validation, configuration and text I/O; `packing-cli` owns filesystem and process
  interfaces. Workers and rendering come later, outside the geometry layer.
- Four-decimal formatting validates both the original solution and its rounded
  representation, refusing output that loses feasibility. A future solver must
  supply sufficient slack or an explicitly tested repair, not rely on allowances.
- Repository licensing is not yet specified; do not invent a license in package
  metadata. Record that decision separately when the owner chooses one.
- Use cheaper subagents for bounded work (checker/tests, source mapping, Rust
  foundations) and have the primary agent review their integration. Avoid compiler
  load during serial baseline measurements; external machine load remains uncontrolled.

## Next numerical-port review

Before implementing A, specify handling of the initializer bounds bug, the
missing best-trial update, degenerate rolling geometry, finite line-search
failure, and BFGS curvature breakdown. Separate safety guards from improvements
that belong in B. See the full source map in `legacy-baseline.md`.

## Numerical component implementation

- Preserve C++ scalar/gradient loop order, strong-Wolfe bracket/zoom order, dense
  BFGS update, and callback refresh ordering. Intentionally retain the old squared
  gradient norm immediately after a callback refresh for scheduling fidelity.
- Treat finite work limits, explicit failed searches, matrix resets for invalid
  curvature, and prefix-safe initialization as necessary defined behavior. Tests
  cover these cases; do not emulate invalid C++ reads or uninitialized steps.
- The minimum pair scale is mathematically zero for coincident centers. Scheduling
  can use that fact to raise penalties, while radial repair must reject it.
- Store full fixed-state C++ oracles and replay them in CI. This checks fidelity
  without importing the legacy checkout into the repository or CI environment.
- Correct B's experiment shortlist to reflect the chosen explicit-radius objective:
  it has no exponential term. Softmax stabilization is only relevant to a future
  alternative objective, not a presumed improvement to the selected solver.
