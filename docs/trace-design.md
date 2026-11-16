# Run and optimization trace design

This is a design for milestone 5, not a description of implemented trace support.
The solver currently returns a checked incumbent and aggregate counts; it does not
emit incumbent events or resume an interrupted search.

## Run record

Store one versioned JSON object per line in a run manifest, with schema version,
run ID, solver name and revision, algorithm/configuration, seed, requested work and
time limits, termination reason, input hash, and instance size. Include runtime
environment fields needed to interpret measurements: compiler/build profile,
machine/OS, and elapsed time. For legacy runs, preserve the exact executable and
source hashes, command, and baseline variant. The harness should accept explicit
baseline-archive and executable paths so another machine can compare against its
own fresh legacy measurements. Record paths and hashes; keep this repository's
`legacy/` checkout in place and ignored.

The manifest also records final status and result: output hash, exported radius,
internal feasibility, reparsed-output feasibility, maximum overlap and containment
violation, objective evaluation count, iterations, restarts, initialization
fallbacks, numerical failures, and elapsed seconds. Invalid or failed runs remain
in JSONL and cannot be counted as improvements.

## Incumbent events

Each trace is a JSONL sequence with a header linking it to a run ID, followed by
incumbent events and a final event. Events carry schema version, event sequence,
elapsed seconds, actual objective evaluation count, incumbent radius, maximum
geometric violation, and all circle center coordinates in input order. Include
the evaluation's feasibility result and distinguish internal coordinates/radius
from the four-decimal exported solution. Initial and final snapshots are always
allowed; ordinary snapshots are separated by at least 100 ms. Keep coordinates so a
viewer can replay the packing without rerunning the solver.

Record only independently validated feasible incumbents. Their radii must be
monotonic nonincreasing. The penalized optimization objective is a separate series:
penalty weights change during the search, so its raw value can rise and values from
different penalty phases are not comparable. It must never be presented as the
certified best feasible radius. A replay of the final event must reproduce the
reported output after the same formatter and parser checks.

Tracing is opt-in and disabled by default. Enabling it must preserve fixed-work
output and work counts. Measure trace overhead in fixed-work runs on
the development set; the median runtime increase must remain below 5%. Keep raw
traces alongside summaries so figures can be regenerated and failed runs audited.

## Unified comparison and resumable execution

One harness should run legacy, A, and B sequentially with explicit, equal budgets,
write raw JSONL plus traces, and regenerate a Markdown summary from those records.
Identify each algorithm and configuration independently; never infer that two
executables are the same baseline from their filenames. Preserve input, executable,
source, and configuration hashes with each result.

Milestone 7 needs a resumable solver session exposed through bounded `step` calls.
The session must retain BFGS inverse-Hessian state, line-search bracket/state,
RNG state, and penalty schedule phase across calls, as well as incumbent and
counters. The current `minimize` call keeps its search state on the stack;
cooperative cancellation returns an incumbent but discards the Hessian and active
line-search state. It therefore supports stopping, not pause-and-resume. Refactor
those states into explicit owned data before claiming resumability or native/WASM
step parity.

Inject a monotonic clock into the session so native `Instant` and browser timing
can share the same numerical code without relying on native clock APIs in WASM.
