# Dense Packing of Circles into a Circle of Minimal Radius

Rust reimplementation of a contest-winning circle-packing solver, with a measured
improvement track and a planned interactive WASM visualization.

The Rust workspace provides seeded solvers, contest-format I/O, independent
validation, and resumable WASM execution in a Web Worker. Native version A passed
component parity checks and the [750-run quality/runtime qualification](docs/version-a-results.md).
Three [version B experiments](docs/version-b-experiments.md) remain experimental;
none met the improvement target. A browser development harness is available;
the interactive comparison site and GitHub Pages deployment remain planned.
See the [implementation plan](docs/PLAN.md) for milestones and acceptance gates.

## Develop

Install stable Rust with rustfmt and Clippy. Run from the repository root:

```sh
cargo build --workspace --locked
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The portable WASM numerical profile uses pinned `libm`; browser tests use Node.js
20+ and Playwright. GitHub Actions runs Rust, independent Python checks, and
Chromium worker tests.

## Run WASM locally

```sh
rustup target add wasm32-unknown-unknown
python3 tools/build_wasm.py
python3 -m http.server 8000 --bind 127.0.0.1 --directory web
```

Open `http://127.0.0.1:8000`. The harness runs A or an explicit B experiment with
pause, resume, cancellation, and checked solution output. Optimization executes
entirely in a worker. Input is limited to 50 circles and 16 KiB; retained history
is bounded. The binary is generated locally and ignored by Git.

```sh
npm ci --prefix web
cd web
npx playwright install chromium firefox
npm test
npm test -- --browser firefox
```

See [WASM architecture and measured results](docs/wasm.md). The browser and native
session probe use portable software math for cross-target agreement; they can
follow different trajectories from the platform-math CLI. Native timed quality
qualification does not automatically apply to this numerical profile.

## Inspect a packing

```sh
cargo run -p packing-cli -- validate tests/fixtures/two_unit_circles.in tests/fixtures/two_unit_circles.out
cargo run -p packing-cli -- format-instance tests/fixtures/two_unit_circles.in
cargo run -p packing-cli -- format-solution tests/fixtures/two_unit_circles.in tests/fixtures/two_unit_circles.out
```

Input contains the circle count followed by its radii. Solution output contains
the enclosing radius, then a row `radius x y` for each circle in original order.
The enclosing circle is centered at the origin. Validation checks containment,
pairwise separation, finite values, and matching radii/counts. Its tolerance is
`1e-9 * max(1, enclosing_radius, largest_input_radius)`.

`validate` returns exit status 0 for feasible output, 1 for a geometric violation,
and 2 for invalid input or another error. `format-solution` validates before and
after rounding to four decimals; it returns an error if rounding loses feasibility.
Formatting does not optimize or repair a packing.

## Solve an instance

```sh
cargo build -p packing-cli --release --locked
target/release/packing-cli solve bench/data/development/10_01.txt --algorithm a --seed 5 --time-limit 1.95
target/release/packing-cli solve bench/data/development/10_01.txt --fixed-work --evaluations 5000
```

Omit the input path or use `-` to read stdin. The solver writes a checked
four-decimal packing to stdout and one JSON run summary to stderr. The summary
includes the seed, limits, actual work counts, internal/output feasibility,
termination reason, and numerical recovery counts.

Native version A supports 1–50 circles. It retains a feasible fallback before
optimization, so zero budgets and cooperative cancellation still return a valid
result. `solve_with_control` exposes cancellation to Rust callers. Initialization
and final validation/export are bounded atomic work units.

Fixed-work mode disables the deadline and requires an explicit evaluation,
iteration, or restart cap. Identical seeds and work limits reproduce output and
counts within the same build; timed runs may differ. Evaluation counts cover
scalar or value/gradient calls, excluding initialization and validation.

## Record and replay progress

```sh
target/release/packing-cli solve bench/data/development/30_01.txt --fixed-work --evaluations 50000 --trace /tmp/packing-trace.jsonl > /tmp/packing-solution.txt
python3 bench/replay_trace.py /tmp/packing-trace.jsonl --solution /tmp/packing-solution.txt
```

Use a new trace filename; existing files are never overwritten. Each versioned
JSONL trace contains checked feasible incumbents and enough coordinates to replay
them without solving again. Ordinary snapshots are at least 100 ms apart; initial
and final snapshots are always recorded. Tracing preserves fixed-work output and
counters. An observer's runtime counts toward a timed solve.

The [trace study](docs/tracing-results.md) passed the 5% overhead gate: 150
fixed-work pairs preserved output/work exactly, with a measured median process
runtime increase of 3.25% on the recorded machine.

`bench/compare.py` runs configured legacy/Rust solvers sequentially and regenerates
comparison tables from raw records. See [benchmark commands](bench/README.md).
`packing_core::session::SolverSession` retains optimizer state across bounded
steps. The browser worker exposes pause/resume; the full visualization remains a
future milestone.

## Legacy baseline

`legacy/` stays a separate ignored checkout; it is never part of this repository
or CI. Local baseline runs require that checkout and GCC. See
[benchmark commands](bench/README.md), [algorithm map](docs/legacy-baseline.md),
and [source-selection evidence](docs/legacy-selection.md). Synthetic datasets
are explicitly distinct from original contest inputs.

The candidate legacy solver has a sanitizer-confirmed memory error. Raw historical
results are retained for comparison; undefined behavior is not a specification
for the Rust port. Required safe deviations will be documented and tested.

## Contribute

Follow [AGENTS.md](AGENTS.md). Work in the root repository, document decisions,
and commit/push coherent steps with short imperative subjects.
