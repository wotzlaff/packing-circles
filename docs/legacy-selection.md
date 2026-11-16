# Estimating the contest solver

## Conclusion

`legacy/circles-hackbfgs.tr.cpp` is the leading candidate for the final solver.
Confidence is moderate: repository usage supports it, but no submission artifact
or contemporary development history establishes the contest winner.

## Evidence

- The legacy history has only two commits, both on 20 August 2024:
  `d75aded` adds an empty README; `33a13c0` imports all source variants together.
  There is no historical sequence from which to identify the final 2022 version.
- `legacy/visual.ipynb` actively compiles `circles-hackbfgs.tr.cpp` into `circles`;
  `circles.cpp` and `roll-heuristic-only.cpp` are commented alternatives. Saved
  execution counts and solution output support notebook use, although notebook
  cells can be edited after execution and are not submission records.
- `legacy/run_benchmark_params.py` invokes `./circles` with two penalty values,
  two tolerance values, an initialization trial count, and a time budget. This
  matches the argument layout of `circles-hackbfgs.tr.cpp`; it does not match
  `main.cpp`, `circles.cpp`, or `circles-hackbfgs.cpp`.
- The candidate defaults to a 1.95-second budget. Its optimizer includes an
  explicit radius variable and adapts penalties through BFGS callbacks. This is
  a materially different algorithm from `main.cpp`'s softmax objective.
- Counterevidence: `run_benchmark.py` and `generate_benchmark_solutions.py` invoke
  `./main`. Without a build recipe linking that binary to a source file, these
  references establish another workflow, not which source was submitted.

## Consequences for the plan

Treat `circles-hackbfgs.tr.cpp` as the provisional version A reference, retaining
`main.cpp` as a comparison baseline until provenance is confirmed. Freeze the
selection before porting numerical components. Compare objective and scheduling
against the selected source rather than assuming the softmax formulation.

The candidate uses a fixed `std::seed_seq {5}` and global `ranlux24_base`, whereas
`main.cpp` seeds initialization with `random_device`. Neither gives deterministic
wall-clock termination. Record RNG behavior per variant; do not claim paired
Rust/C++ seeds without matching both generators and distribution algorithms.

Reproduce the investigation with `git -C legacy log --all --format=fuller --stat`,
the build cell in `visual.ipynb`, and the argument lists in
`run_benchmark_params.py` and `circles-hackbfgs.tr.cpp`.
