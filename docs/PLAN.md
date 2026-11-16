# Circle Packing Implementation Plan

## Scope and working rules

Build in the root repository. Leave `legacy/` as an ignored, separate reference
checkout; never import it or add it as a submodule. Deliver a faithful Rust
version A, an experimentally improved version B, and a browser application on
GitHub Pages. Commit each coherent step using short imperative subjects and push
after each commit. Each milestone may take several commits.

All milestones below are pending. Completion requires the listed evidence, not
just implementation. Record decisions, numerical exceptions, and experiment
results in `docs/`; update this plan when a gate passes or scope changes.

## Measurement contract

- **Correctness:** validate circle count, radii, finite coordinates, pairwise
  separation, and containment independently of the optimizer. For internal
  checks use tolerance `1e-9 * max(1, R, max_radius)` and report the actual maximum
  violation. Reparse four-decimal output and check that rounding preserves
  feasibility; report rounding allowances separately from internal tolerance.
- **Quality:** minimize enclosing radius `R`. Report `R / sqrt(sum(r_i²))` as an
  area-bound ratio, not an optimality certificate. For comparisons report
  percentage radius change, geometric-mean radius ratio, and worst regression.
- **Reproducibility:** save input hashes, solver commit, seed, configuration,
  compiler/build flags, hardware, elapsed time, evaluation counts, and feasibility.
  Fixed work budgets support repeatable tests; wall-clock runs measure usability.
- **Fairness:** use release builds, one process at a time on the same machine,
  equal end-to-end solver budgets including initialization and repair, and five
  runs per instance at 0.1, 0.5, and 1.95 seconds. Use fixed seeds for Rust; the
  unmodified legacy executable uses nondeterministic randomness, so compare its
  repeated-run distributions rather than claim paired seeds. Report time overruns.
- **Data:** generate and commit 50 development instances and 50 held-out instances,
  ten of each size 10, 20, 30, 40, and 50 in each set, with integer radii 20–99.
  Publish generator seeds and hashes before tuning. These are synthetic instances,
  not recovered contest data. Use development data for tuning and reserve held-out
  evaluation for release candidates; disclose repeated use.

The size/radius ranges and four-decimal format follow the
[contest specification](https://incyb.kiev.ua/storage/editor/files/problem-2022-10-17-en.pdf).
The time budgets above are project benchmarks, not asserted contest requirements.

## 1. Freeze the legacy baseline and map the algorithm

**Deliver:** `docs/legacy-baseline.md`, a root-owned build/run wrapper, dataset
generator, independent solution checker, and baseline results under
`bench/results/`. Put temporary binaries outside `legacy/`.

Map `main.cpp` and `fun_softmax.cpp` to initialization, centering, objective and
gradient, strong-Wolfe search, BFGS, penalty scheduling, repair, and output. Record
defaults, termination conditions, and suspected quirks before deciding how A
handles them. Identify whether `main.cpp` is the intended winning entry point;
until confirmed, label it the provisional reference.

**Done when:** both datasets and their hashes exist; checker tests reject overlap,
out-of-bounds circles, missing circles, and nonfinite values; legacy runs complete
on all 50 development instances at all three budgets with five repetitions.
Archive failures and invalid results rather than silently dropping them. Preserve
the observed two-unit-circle result as a smoke case, not an optimality guarantee.

## 2. Establish the Rust core and continuous checks

**Deliver:** a Cargo workspace with `crates/packing-core` and
`crates/packing-cli`; geometry, configuration, input/output, and validation modules;
integration fixtures under `tests/fixtures/`; CI for formatting, Clippy, and tests.

**Done when:** the CLI round-trips contest-format input/output; tests cover invalid
input, tangency, overlap, translation, scale changes, and the known optima for one
circle (`R = r`) and two circles (`R = r1 + r2`). All documented Cargo commands
pass. The library has no browser dependencies. Public APIs document units,
tolerances, and failure behavior.

## 3. Port and verify version A's numerical components

**Deliver:** separate initialization, objective, line-search, BFGS, and repair
modules, with deterministic RNG injection and evaluation/iteration limits.
Create root-owned reference probes that include the existing C++ source without
editing it. Document allowed differences in `docs/version-a.md`.

**Done when:** objective and gradient agree with C++ on at least 100 fixed,
nondegenerate configurations using combined `1e-9` absolute / `1e-8` relative
tolerances. Central finite-difference checks achieve relative gradient error
below `1e-5` on well-conditioned fixtures away from kinks. Fixed-initial-state,
fixed-work probes compare line search, BFGS, and penalty scheduling against C++;
every divergence is explained and covered by a regression test. Singular and
overflow cases return documented outcomes without panics or invalid output.

Do not silently repair legacy behavior while claiming parity: isolate necessary
guards, preserve ordinary-path behavior, and defer algorithm changes to B.

## 4. Complete and qualify version A

**Deliver:** `packing-cli solve --algorithm a --seed ... --time-limit ...`, best
feasible incumbent tracking, cancellation, and machine-readable run summaries.
Run the complete legacy/A comparison and publish `docs/version-a-results.md`.

**Done when:** A emits valid internal and reparsed output on every benchmark run;
fixed-seed, fixed-work runs repeat on the same build; component parity gates pass.
Target a geometric mean of per-instance median `R_A / R_legacy <= 1.01` and no
per-instance median regression above 5% at 1.95 seconds. Report all sizes and
budgets. Investigate misses and explicitly revise the gate with evidence if
initialization differences prevent it; do not declare parity from aggregate
quality alone. Deadline tests allow at most `max(20 ms, 10% of budget)` overrun
on the recorded benchmark machine.

## 5. Build the experiment harness and optimization trace

**Deliver:** one benchmark command for legacy/A/B, JSONL raw results, a Markdown
summary generator, and a versioned trace format. Each incumbent event records
elapsed time, radius, violation, evaluations, and enough coordinates to replay it.

**Done when:** one command reproduces summary statistics from archived results;
invalid runs remain visible and cannot count as improvements; trace replay ends
at the reported best solution; feasible-incumbent radii never increase. At a
maximum of ten snapshots per second, trace recording adds less than 5% median
runtime in fixed-work comparisons on the development set. If missed, reduce
snapshot frequency and document the measured tradeoff.

## 6. Develop and qualify version B

**Deliver:** independently switchable experiments with hypotheses and ablations.
Start with stabilized exponential evaluation and singularity handling, then test
corrected/alternative initialization, adaptive penalties and restart allocation,
and L-BFGS versus dense BFGS. Treat these as candidates, not predetermined wins.
Keep A available and unchanged by B's algorithm choices.

**Done when:** at least three candidates have recorded development-set results
including unsuccessful trials. Freeze the selected configuration before held-out
evaluation. The B release target at 1.95 seconds is at least 1% geometric-mean
radius reduction relative to A using per-instance medians, no per-instance median
regression above 2%, and 100% feasible outputs. Include a bootstrap 95% confidence
interval over instances and require its geometric-mean ratio upper bound below
1. Report 0.1/0.5-second results and all regressions separately.

If no candidate passes, publish the negative result and keep B experimental;
do not weaken the target retrospectively or market an unmeasured improvement.

## 7. Expose resumable execution through WASM

**Deliver:** `crates/packing-wasm`, a bounded `step` API shared with the native
solver, seeded configuration, progress events, and cancellation. Run optimization
in a Web Worker. Define limits for uploaded input and retained trace memory.

**Done when:** both algorithms compile for `wasm32-unknown-unknown`; ten fixtures
pass browser-side validation; fixed-work native/WASM runs differ by at most
`1e-6` relative final radius on these fixtures, with divergence investigated.
Repeated worker runs with the same seed/configuration reproduce within the same
browser build. A 50-circle run can be cancelled within 100 ms and progress reaches
the UI at least five times per second on a documented reference machine. No
solver task executes on the UI thread.

## 8. Build the interactive comparison website

**Deliver:** a site under `web/` with instance presets and upload, A/B selection,
seed and time controls, run/pause/resume/reset, packing animation, and an
improvement-history chart. Offer export of solutions and run metadata. Display
feasibility, elapsed time, best radius, and the area-bound ratio.

Compare live A/B runs sequentially with equal budgets to avoid CPU contention;
retain traces for synchronized replay. Show pending/best-feasible states clearly.
Add two explanatory views: active contacts/overlaps and the optimization
objective versus the best feasible radius.

**Done when:** browser tests cover upload validation, both solvers, pause/resume,
cancel, reset, replay, and export/reimport. Controls work by keyboard; charts and
canvas have textual equivalents; layout works at 360 px and 1440 px. During a
50-circle run, control response is below 100 ms at the 95th percentile and render
time below 33 ms at the 95th percentile on the reference machine, measured over
30 seconds. Record browser/device details and trace-memory use.

## 9. Publish and verify the GitHub Pages release

**Deliver:** CI that builds/tests Rust and WASM, runs browser smoke checks, and
deploys versioned static assets to Pages. Add setup instructions, an architecture
map, benchmark reproduction instructions, and links to experiment reports.

**Done when:** the public URL loads with the repository base path, runs A and B
entirely in-browser, and exports a validated solution in current stable Firefox
and Chromium. Verify first load, reload, worker/WASM loading, and clear errors.
Show the solver commit and benchmark environment with published comparison data.
Record the release URL and commit in this document.

## Review points and unresolved inputs

- Confirm the historical winning entry point before freezing A; meanwhile inspect
  and measure `main.cpp` as the provisional baseline.
- If original contest inputs/results become available, archive provenance and add
  a separate evaluation track; never substitute synthetic data without labeling it.
- Select frontend tooling during milestone 8 based on worker, chart, and static
  hosting needs. Record the choice then; no frontend framework is required yet.
- Runtime and UI thresholds refer to recorded hardware, not universal guarantees.
  Revise targets only prospectively, with an explanation committed to `docs/`.

Dependencies: 1 → 2 → 3 → 4 → 5. Milestone 6 follows 5; milestone 7 can proceed
after 5 while B experiments continue. Milestone 8 needs 7; milestone 9 needs
qualified A, a measured B status, and 8. The next concrete action is milestone 1.
