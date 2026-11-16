# Version B development experiments

The following three single-policy candidates are frozen before measurement. All
retain A's rolling initialization, objective, repair, work limits, and incumbent
selection. They are experimental and require explicit `--algorithm b --variant`.

- `early-penalty`: raise the squared-gradient threshold for penalty updates from
  `1e-5` to `1e-3`. Hypothesis: refining feasibility earlier reduces wasted local
  optimization work; it may instead bias optimization away from compact basins.
- `tight-penalty`: lower the relative violation threshold from `1e-5` to `1e-6`.
  Hypothesis: more constraint refinement reduces repair expansion; extra work may
  crowd out restarts.
- `short-restarts`: cap each BFGS local search at 100 iterations. Hypothesis:
  exploring more initial packings improves best-of-run quality; early truncation
  may prevent promising basins from converging.

The configured serial development study uses 50 frozen development instances,
three budgets, and seeds 5–9. The comparison is to the historical A archive,
not contemporaneous paired execution. No held-out data is included. Results and
limitations will be added after the study; release qualification remains pending.
