# Repository Guidelines

## Purpose & Architecture

Reimplement the contest-winning circle-packing solver in Rust with clear module boundaries. See the [contest](https://incyb.kiev.ua/en/event/glushkov-cyber-2022-11-01) and [problem definition](https://incyb.kiev.ua/storage/editor/files/problem-2022-10-17-en.pdf).

- **Version A:** preserve the legacy algorithm; random initialization and minor numerical differences may vary.
- **Version B:** improve solution quality, performance, or robustness, with measured comparisons against A.

The eventual GitHub Pages site should execute through WASM, visualize optimization live, and plot improvement history. Propose useful mathematical explanations and interactive experiments.

## Project Structure & Module Organization

`legacy/` is currently a separate Git checkout containing C++ solvers, Python benchmark utilities, and exploratory notebooks. Treat it as reference material; avoid unrelated edits.

The root has no Rust package, website, dedicated test directory, or asset pipeline yet. When introducing these, document their actual locations and commands. Keep solver logic independent of browser rendering, and record implementation decisions and known issues in Markdown under `docs/`.

## Build, Test & Development Commands

No root build or test command exists yet. With GCC installed, build the legacy solver from the repository root:

```sh
g++ -O2 -std=c++17 legacy/main.cpp -o /tmp/circles-legacy-main
printf '2\n1 1\n' | /tmp/circles-legacy-main
```

This smoke input packs two unit circles. From `legacy/`, `python3 run_benchmark.py` requires a compiled `./main` and matching files under `data/`; benchmark inputs are not currently included.

Once a Cargo package exists, use:

- `cargo build` — compile the Rust implementation.
- `cargo test` — run unit and integration tests.
- `cargo fmt --check` — verify standard Rust formatting.
- `cargo clippy --all-targets -- -D warnings` — reject lint warnings.

Document browser/WASM commands when that tooling is introduced.

## Coding Style & Testing Guidelines

Use rustfmt's four-space indentation, `snake_case` functions/modules, and `PascalCase` types. Document public APIs, mathematical assumptions, tolerances, and stopping criteria.

Use Rust's built-in test framework with descriptive names such as `rejects_overlapping_circles`. Cover containment, pairwise separation, objective calculation, and numerical edge cases. Use fixed seeds for reproducibility and explicit floating-point tolerances. Compare A against legacy behavior and benchmark B under equivalent time budgets; record quality and runtime. No coverage percentage is established.

## Commit, Review & Agent Workflow

Commit each coherent step and push after each commit. The root has no commit history; proposed subjects use short imperative wording, such as `Add packing feasibility checks`.

PRs should explain changes, validation, numerical effects, and relevant issues; include screenshots for visual changes. Record decisions and unresolved questions in Markdown. Delegate bounded tasks to cheaper subagents when reasonable.

