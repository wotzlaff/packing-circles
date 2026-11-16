# Interactive comparison site

The app in `web/` implements milestone 8. It runs entirely in-browser through
the resumable WASM worker; no uploaded instance is sent to a server. Version B
remains experimental: none of its three development candidates met the quality
target. Native quality qualification does not transfer automatically to the
browser's portable math profile.

## Use the app

Build and serve with the [README commands](../README.md#run-wasm-locally).
Choose a frozen synthetic development preset, equal circles, two circles, or
upload a contest-format instance (1–50 circles, at most 16 KiB). Seeds are decimal
u64 values; each active-time budget is 0.1–60 seconds. A/B comparisons use the
same input, seed, and budget, running A and then the selected B policy.

Pause/resume retain optimizer state. Cancellation retains a checked packing and
cancels queued comparisons; reset cancels active work and ignores late messages.
A rejected upload blocks Run until corrected or reset, preventing accidental
execution with the previous input. Parameter controls are frozen during a run.

The initial preview is a recorded, labeled portable-A development result, not
an automatic background search. The main view always identifies the checked
incumbent separately from raw accepted trial geometry. Contact lines mark near
tangencies; red trial outlines mark overlaps or containment violations. The
displayed ratio is `R / sqrt(sum(r_i²))`, at least one for a feasible packing;
it is not a certificate of optimality.

The history plots feasible radius against active elapsed time. Optional dashed
curves show the penalized objective on a separately labeled signed-log axis.
Penalty weights can change, so decreases across weight changes are not comparable
to radius improvements. Trial objective, coordinates, and weights come from the
same accepted evaluation; pending refreshes have no diagnostic snapshot.

After completion, the shared timeline and replay select each run's last recorded
frame at or before the same active time. Coordinates are never interpolated:
intermediate interpolation could collide even between two feasible packings.
Canvas and chart have textual equivalents, a recent history table, and native
keyboard controls. Chart labels adapt to narrow screens.

## Recordings and bounds

Export the selected checked four-decimal solution, or a versioned JSON recording
with configuration, counters, packings, diagnostics, build provenance and selected
replay position. Import independently checks finite geometry, containment,
separation, radii order, four-decimal output, counters, stopping state, and
monotonic time/radius. Invalid imports clear prior runs instead of showing stale
results as a newly imported recording. Import files are limited to 8 MiB.

The app retains at most two runs, 512 frames per run and 4 MiB of serialized
snapshot payload. It preserves first/latest frames, evicts intermediate frames
when necessary and displays the eviction count. The worker client retains at
most another 1 MiB; WASM linear memory is capped at 64 MiB. These are payload
bounds, not a claim about total browser heap. Source presets are synthetic
development data; held-out data remains unused.

## Architecture

- `app.js`: experiment lifecycle, sequential scheduling, controls, replay and exports.
- `data.js`: independent input/recording validation and bounded history.
- `render.js` / `styles.css`: canvas geometry, responsive SVG history and layout.
- `presets.js`: frozen development examples and labeled recorded previews.
- `solver.js` / `solver-worker.js`: existing bounded worker execution.
- `build_wasm.py`: binary plus compiler, source commit, dirty state and SHA-256 metadata.

`getAppState()` and `getPerformanceStats()` expose cloned developer diagnostics
for reproducible tests. Performance samples are bounded to 2048 entries.

## Validation

Run `npm run test:data --prefix web` and `npm run test:site --prefix web`.
The site suite defaults to a 30-second, 50-circle measurement; use
`-- --stress-seconds 3` for functional CI smoke checks. Short checks explicitly
do not qualify performance. `-- --browser firefox` selects Firefox.

Both browser suites passed upload validation, sequential A/B settings, real
replay, raw/contact/objective views, solution and recording round trips,
malformed imports, pause/resume, cancellation, reset during active work,
keyboard controls, and layouts at 360 px and 1440 px.

On Linux 6.1.0-44, Intel i7-8700, Node 24.22.1 and Playwright 1.64.1:

| Browser | Measurement | Control operations | Control response p95 | Render function p95 | Retained trace |
| --- | ---: | ---: | ---: | ---: | ---: |
| Chromium 156.0.8078.4 | 30 s | 98 | 20.6 ms | 0.9 ms | 316 frames / 1,810,880 bytes |
| Firefox 157.0 | 30 s | 100 | 13 ms | 5 ms | 316 frames / 1,809,305 bytes |

Both passed the predeclared `<100 ms` control and `<33 ms` render gates.
Control time measures click-handler entry to worker acknowledgement; render
time covers the application canvas/SVG/DOM render function, excluding asynchronous
GPU compositing. Tests include repeated pause/resume operations during the
50-circle run. These measurements apply to this reference machine and these
browser builds, not every device.

Raw reports retain timing samples, memory where available, browser/device
versions, WASM and frontend source hashes:
[Chromium](../bench/results/site-chromium.json),
[Firefox](../bench/results/site-firefox.json).
[Desktop screenshot](assets/site-desktop.png) · [Mobile screenshot](assets/site-mobile.png).

The Rust suite has 85 passing tests, Node validation has six, and Python has 47.
All 50 archived native-A fixed-work outputs/counters and 426 numerical reference
cases still pass. The original 40-case worker/native browser regression also
passes with the additional read-only diagnostics.
