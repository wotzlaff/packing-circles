# Terra handoff: version B development experiments

## Starting point

Milestones 1–5 are complete at `cad2da9`. Version A is qualified; optional traces,
independent replay, and configured comparisons work. Read `AGENTS.md`,
`docs/PLAN.md`, `docs/version-a.md`, and `docs/tracing-results.md` before changes.

## Assigned work

Implement and evaluate at least three independently selectable version B
candidates. Favor bounded changes with clear hypotheses: better initialization or
centering, penalty/restart policies, or another local optimizer. Record each
hypothesis and freeze its parameters before its measured run. Preserve negative
results. Candidates may remain experimental if none meets the release target.

Keep A's behavior and CLI available. Prefer separate B modules and explicit
variant/configuration metadata; never silently run one candidate under another's
label. Extend the existing runner instead of creating another benchmark framework.

Run meaningful tests, formatting, Clippy, and numerical-reference replay. Verify
A still reproduces its fixed-work outputs/counters, and verify B's internal and
serialized geometry, work limits, cancellation, deterministic runs, and traces.

Collect development-set ablations for all three candidates, following the plan's
same-budget serial protocol and five repetitions. Record source/executable/input
hashes, seeds, failures, work counts, runtime, and per-instance median radius
ratios against A. Compare using geometric means and disclose every regression.
Pause compilation and other local benchmark processes during measured runs.

## Boundaries

- Work in the root repository. Keep `legacy/` ignored, unchanged, and separate.
- Do not modify frozen inputs, old measurements, or release thresholds.
- **Do not evaluate held-out inputs in this assignment.** Stop after development
  experiments and recommend a frozen candidate for subsequent release review.
- Do not start WASM or website work, or claim B improved without measurement.
- Commit coherent steps and push after each commit, as required by `AGENTS.md`.

## Deliverables

- Tested candidate implementations and reproducible configurations/commands.
- Raw development results and regenerated comparison/ablation tables.
- `docs/version-b-experiments.md` describing hypotheses, measured outcomes,
  unsuccessful trials, limitations, and the recommended next action.
- Updated milestone status that distinguishes development completion from
  held-out release qualification.

The eventual B release gate remains at least 1% geometric-mean radius reduction
relative to A at 1.95 seconds, no per-instance median regression above 2%, all
outputs feasible, and a bootstrap 95% confidence interval with ratio upper bound
below 1. Report shorter-budget results separately; never weaken these thresholds
after seeing results.
