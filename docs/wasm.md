# Resumable WASM execution

Milestone 7 is complete. The development harness runs version A and all three
experimental B policies in a Web Worker. The comparison website and deployment
remain milestones 8–9; B has not met its quality target.

## Build and run

```sh
rustup target add wasm32-unknown-unknown
python3 tools/build_wasm.py
python3 -m http.server 8000 --bind 127.0.0.1 --directory web
```

Open `http://127.0.0.1:8000`. Assets resolve relative to their modules, including
the worker and WASM binary, so they also work under a repository base path.
The build copies an ignored binary into `web/`; no bundler or WASM bindgen CLI
is required. Rust downloads the pinned `libm` dependency through Cargo.

## Execution contract

`packing_core::session::SolverSession` owns the accepted iterate, dense inverse
Hessian, gradient, penalty weights, strong-Wolfe bracket/zoom state, RNG, and
work counters. `step(max_work, elapsed_seconds)` performs at most that many work
units. Each unit is one oracle call or one bounded transition; initialization
and repair are atomic at the 50-circle limit. Yielding preserves optimization
state. `cancel()` repairs the last accepted iterate and returns the best checked
incumbent; `result()` provides a snapshot at any boundary.

The host supplies finite, monotonic active elapsed time. The worker excludes
paused intervals and includes scheduling delays while running. It calls at most
64 work units at once, yields to its event loop after approximately 8 ms, and
sends progress heartbeats approximately every 100 ms even without improvement.
Time limits are checked at step boundaries. Atomic initialization can extend
a pump; the recorded cancellation measurement checks its practical effect.

```mermaid
flowchart LR
  UI[Harness / future visualization] --> Client[PackingWorker client]
  Client <-->|messages| Worker[Worker scheduler]
  Worker --> ABI[packing-wasm ABI]
  ABI --> Session[Shared native / WASM session]
```

`web/solver.js` exports `PackingWorker`, an `EventTarget`. Call
`start({input, algorithm, seed, evaluationLimit, iterationLimit, restartLimit,
timeLimitSeconds})`; use `pause()`, `resume()`, `cancel()`, and `dispose()`.
The seed is a decimal u64 string. Algorithm IDs are 0=A, 1=early penalty,
2=tight penalty, and 3=short restarts. A null time limit selects fixed work.
Messages contain `type`, `runId`, and a checked `snapshot`, or an `error` string.
Types include `ready`, `progress`, `paused`, `resumed`, `done`, and `error`.
Replacing a run filters messages from older IDs. The UI never instantiates WASM.

The raw ABI and pointer lifetimes are documented in
[`packing-wasm`](../crates/packing-wasm/src/lib.rs). Input is limited to 16 KiB
UTF-8 and 1–50 circles. WASM linear memory is capped at 64 MiB. The client retains
at most 256 recent snapshots and 1 MiB of serialized snapshot payload; it evicts
older frames and reports their count through `exportTrace()`. These are payload
limits, not measurements of total JavaScript heap usage. The worker keeps one
current session and snapshot. Initialization errors never produce a packing.

## Numerical profiles

`SolverSession::new` retains platform math and matches the blocking native solver
exactly in the recorded regression tests. `new_portable` uses `libm` 0.2.16 with
architecture-specific math disabled for initialization trigonometry and solver
distances. The WASM ABI and native `session_probe` use this portable profile.
Objective/gradient and BFGS arithmetic retain their original ordering. Geometry
and serialized output still undergo independent feasibility checks.

An [initial platform-math run](../bench/results/wasm-platform-math-initial.json)
failed the cross-target gate on `20_01.txt`: native radius 354.8223311745897 versus
WASM 354.63090907057926 at 5000 evaluations. Switching both targets to the pinned
profile restored agreement. Small initialization differences can amplify through
later iterations; portable results must not be presented as bitwise CLI replays.
The CLI's 50 archived fixed-work outputs/counters and all 426 C++ component cases
still pass. Portable quality and timed performance have not been requalified
against the complete legacy archive.

## Verification and measurements

```sh
npm ci --prefix web
cd web
npx playwright install chromium firefox
npm test -- --report /tmp/wasm-chromium.json
npm test -- --browser firefox --report /tmp/wasm-firefox.json
```

The test harness starts and stops its own loopback server. It checks ten frozen
development fixtures, two each at N=10/20/30/40/50, across all four policies:
40 native/WASM comparisons per browser, seed 5, 5000 evaluations. It independently
checks internal geometry and four-decimal output, same-build determinism,
pause/resume output/work invariance, malformed/oversized inputs, cancellation,
run replacement, bounded history, and main-thread timer responsiveness.
Held-out inputs were not used.

Measured on Linux x86-64, Intel i7-8700, Rust 1.100.0, Node 24.22.1,
Playwright 1.64.1, release WASM (171235 bytes):

| Browser | Comparisons | Maximum relative radius difference | Cancel latency | Progress in 2 s | Maximum progress gap |
| --- | ---: | ---: | ---: | ---: | ---: |
| Chromium 156.0.8078.4 | 40 | 0 | 5.2 ms | 18 | 142.5 ms |
| Firefox 157.0 | 40 | 0 | 5 ms | 18 | 142 ms |

Both browsers retained responsive main-thread 10 ms timers (201/198 ticks) during
the 50-circle run. These are single-machine integration measurements, not a
universal latency guarantee or the longer milestone-8 UI performance study.
Raw reports include checked packings, work counters, input hashes, compiler and
browser versions, and the WASM hash:
[Chromium](../bench/results/wasm-chromium.json),
[Firefox](../bench/results/wasm-firefox.json).

The Rust suite has 84 passing tests; Python has 47. Formatting, Clippy, frozen A
replay, C++ numerical replay, and WASM release compilation pass. CI builds WASM
and runs the Chromium worker harness; Firefox was additionally checked locally.
