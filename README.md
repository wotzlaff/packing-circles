# Dense Packing of Circles into a Circle of Minimal Radius

Rust reimplementation of a contest-winning circle-packing solver, with a measured
improvement track and a planned interactive WASM visualization.

The Rust workspace provides a seeded version A solver, contest-format I/O, and
independent validation. Version A's numerical components passed fixed-state C++
comparisons; end-to-end qualification is in progress. Version B and the website
are not implemented yet.
See the [implementation plan](docs/PLAN.md) for milestones and acceptance gates.

## Develop

Install stable Rust with rustfmt and Clippy. Run from the repository root:

```sh
cargo build --workspace --locked
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

No external Rust crates are required. GitHub Actions runs these checks together
with the Python benchmark tests and dataset verification.

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
