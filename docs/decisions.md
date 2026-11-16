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

## Native version A assembly

- Save a validated line fallback before polling zero budgets/cancellation and save
  repaired initialization candidates before local optimization. This adds defined
  output where the legacy driver could leave its best-point vector empty.
- Count each scalar/gradient evaluation and attempted BFGS iteration across all
  restarts. Exclude initialization/export from evaluation counts but include them
  in measured time. Poll cancellation/deadline at objective and iteration boundaries.
- Keep the native solver at 1–50 circles to bound dense-Hessian storage and atomic
  initialization work; leave general-purpose geometry parsing unrestricted.
- Add export containment slack and, only if needed, absolute separation slack to
  survive four-decimal coordinates. Compare only validated, serialized incumbents.
- Qualify against the frozen 750-run legacy archive on the same recorded CPU/OS.
  Disclose that this is an archived baseline with undefined behavior and uncontrolled
  external load, not a contemporary paired-seed experiment. Rust uses seeds 5–9;
  the unmodified C++ executable uses its fixed seed sequence on each invocation.

## Trace and comparison implementation

- Put progress observation outside numerical kernels, at existing checked-incumbent
  updates. A compile-time disabled path avoids extra validation/allocation when
  tracing is off. Enabling observation adds no random draws/objective calls.
- Borrow snapshots during callbacks; serialize/copy only in the caller. Always
  emit initial/final frames, and separate ordinary improvements by at least 100 ms.
- Link trace headers and final counters to run summaries. Validate all geometry
  independently and require actual formatter rounding, including floating values
  just above a four-decimal boundary. Rounding allowances never qualify a frame.
- Keep a configured comparison runner separate from the predeclared version A
  qualification gate. Preserve failed observations; require complete valid
  repetitions before reporting comparative per-instance medians.
- Compare traced/plain runs at fixed work, alternating pair order, with both
  outputs/counters and runtime audited. Keep timings, metadata, and all traces;
  do not infer an optimization speedup from timing noise.

## Resumable execution and portable math

- Retain the qualified blocking native solver as the regression reference. Add
  an owned session that stores the dense Hessian, accepted iterate, gradient,
  strong-Wolfe bracket/zoom phase, penalty schedule, RNG, and global work counters.
  A yield preserves these values; resume never restarts optimization.
- Bound each work unit to one objective call or one transition, including an
  atomic rolling initialization at at most 50 circles. Let hosts supply monotonic
  active elapsed time so WASM needs no operating-system clock and pauses exclude
  waiting time. Cancellation repairs the last accepted iterate before returning.
- Initial browser comparisons exposed target-dependent math: at 5000 evaluations,
  `20_01.txt` gave radius 354.63090907057926 in Chromium versus
  354.8223311745897 natively, exceeding the `1e-6` relative gate. Small math
  differences can change initialization choices and subsequent BFGS trajectories.
- Provide `SolverSession::new_portable` with pinned `libm` 0.2.16 and
  `force-soft-floats` for initialization trigonometry and distance calculations.
  The feature disables architecture-specific math implementations, as described
  in the [upstream manifest](https://docs.rs/crate/libm/0.2.16/source/Cargo.toml.orig).
  Browser execution and the native session probe use this profile. Keep platform
  math in the CLI and `SolverSession::new`; exact archived native outputs and
  work counters remain regression requirements. Portable results are a distinct
  numerical profile and do not inherit native quality/runtime qualification.
- Use a small dependency-free WASM host ABI around the Rust session, with a
  fixed 16 KiB input buffer, checked JSON snapshots, and at most 64 work units
  per call. Expose seed words rather than requiring JavaScript number precision
  for a full 64-bit seed. Browser scheduling and rendering remain outside Rust.
- Allow zero-work ABI clock checkpoints for pause, resume, and cancellation, so
  control snapshots use current active time without consuming another optimizer
  transition. Test real deadline completion after a paused interval; accept the
  browser's finite clock resolution for immediate cancellation timestamps.

## Interactive search diagnostics

- Expose a read-only copy of the current accepted local iterate, objective,
  squared gradient, and evaluated penalty weights. Reading diagnostics performs
  no objective calls and consumes no randomness. Hide diagnostics while a
  penalty refresh is pending so coordinates, value, and weights describe the
  same evaluation. Keep the checked feasible incumbent separate from raw trial
  geometry; a low penalized objective is not an optimality certificate.
- Keep the site buildless, using the existing worker client, canvas geometry,
  SVG history, and native browser controls. Compare A and an explicitly selected
  B policy sequentially with identical input, seed, and active-time budget.
  Replay recorded feasible frames at a shared active time; do not interpolate
  coordinates, which could create collisions between feasible endpoints.
- Independently validate imported geometry and four-decimal output. Bound run
  recordings to two runs, 512 frames per run and 4 MiB of serialized snapshot
  payload; disclose any evicted intermediate frames. Retain the first and latest
  frames. The worker client retains at most another 1 MiB of recent snapshots.
- Scope native byte-for-byte archive replay to its qualification environment.
  The first Pages pipeline exposed that its Ubuntu/Rust-1.99 platform math differs
  from Debian/Rust-1.100. Require exact comparison to the pinned pre-site Rust
  solver compiled on the same CI host, alongside numerical oracle replay and
  portable native/WASM checks. Preserve the failed job evidence; retain the
  original archive command and unchanged numerical/quality thresholds.

## Make the early search visible

- The 100 ms worker heartbeat skipped most early search positions. Batch at most
  64 early feasible-incumbent and accepted-trial checkpoints, probing at most
  256 bounded steps during the first 250 ms. Reporting adds no optimizer
  evaluations or random draws; recording overhead counts toward active time.
- Expand the time axis near zero by default and zoom the vertical range past
  the starting line fallback. Keep the original values and elapsed-time mode
  available, and label the clipped fallback explicitly.
- Pace replay by recorded checkpoints, with at least eight seconds and roughly
  100 ms per checkpoint at normal pace. Share actual elapsed time between runs,
  hold recorded geometry between checkpoints, and provide previous/next controls
  for changes sharing a rounded timestamp. Preserve the first 64 checkpoints
  when the bounded history evicts later intermediate frames.
