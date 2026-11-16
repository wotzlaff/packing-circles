# Repository Guidelines

## Purpose & Architecture

Reimplement the contest-winning circle-packing solver in Rust. See the [contest](https://incyb.kiev.ua/en/event/glushkov-cyber-2022-11-01) and [problem definition](https://incyb.kiev.ua/storage/editor/files/problem-2022-10-17-en.pdf).

- **Version A:** preserve the legacy algorithm; random initialization and minor numerical differences may vary.
- **Version B:** improve solution quality, performance, or robustness, with measured comparisons against A.

The GitHub Pages site should execute through WASM, visualize optimization, and plot improvement history. Propose mathematical explanations and interactive experiments.

## Project Structure & Module Organization

Work in the root repository. Keep `legacy/` as a separate reference checkout. Never add it to the root repository or as a submodule; avoid unrelated edits.

`crates/packing-core/` owns geometry, I/O, and numerical kernels; `crates/packing-cli/` owns file commands. Rust integration tests live beside each crate, with shared data in `tests/fixtures/`. `bench/` contains independent Python checks, frozen synthetic inputs, and archived results. Browser work is planned under `web/`. Record decisions and milestone evidence in `docs/`; follow `docs/PLAN.md`.

## Build, Test & Development Commands

Use stable Rust, Python 3.10+, and GCC; run from the root:

- `cargo build --workspace --locked` — compile Rust tools.
- `cargo test --workspace --locked` — run unit and integration tests.
- `cargo fmt --all --check` — verify formatting.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — reject lint warnings.
- `python3 -m unittest discover -s bench -v` — test independent benchmark tooling.
- `python3 bench/generate_instances.py --check` — verify frozen data.
- `python3 bench/legacy.py build --variant contest` — build the selected legacy reference outside its checkout.

See `README.md` and `bench/README.md` for CLI examples and measurement commands. Full solvers and WASM execution are pending.

## Coding Style & Testing Guidelines

Use rustfmt, four-space indentation, `snake_case` functions/modules, and `PascalCase` types. Document APIs, mathematical assumptions, tolerances, and stopping criteria.

Use Rust's built-in tests with descriptive names such as `rejects_overlapping_circles`. Cover containment, separation, objective calculation, and numerical edge cases. Use fixed seeds and explicit tolerances. Compare A against legacy behavior and benchmark B under equal time budgets. Preserve raw failures; document necessary safe departures from legacy undefined behavior. No coverage percentage is established.

## Commit, Review & Agent Workflow

Commit each coherent step and push after each commit. Use short imperative commit subjects, such as `Add packing feasibility checks`.

PRs should explain changes, validation, numerical effects, and relevant issues; include screenshots for visual changes. Record decisions in Markdown. Use cheaper subagents for bounded implementation, tests, or reviews whenever reasonable; the primary agent integrates and verifies results.

