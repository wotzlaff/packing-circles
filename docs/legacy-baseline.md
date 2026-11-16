# Legacy solver map

This is a source map for the provisional version A reference, `legacy/circles-hackbfgs.tr.cpp`; it is an inspection record, not a claim that the source is the proven contest submission. Selection evidence and the remaining provenance gap are in [legacy-selection.md](legacy-selection.md). The comparison source is `legacy/main.cpp`. Line references below refer to the legacy files as inspected; do not change those files to address quirks while establishing baseline parity.

## Candidate: `circles-hackbfgs.tr.cpp`

### Initialization and centering

- RNG state is a global `std::ranlux24_base` constructed from fixed seed sequence `{5}` (`circles-hackbfgs.tr.cpp:13-18`). Random draw sequence is deterministic for a fixed standard library/compiler implementation, but `std::shuffle` and distribution behavior are not a portable cross-library seed contract.
- `rollAroundHeuristic` seeds the first two circles tangent on the x-axis, then adds each next circle at candidate positions obtained by rolling around prior circles; it picks the candidate minimizing a recentered enclosing-radius estimate (`206-239`). `rollAround` starts at the rightmost prior circle, walks candidate tangencies around the existing arrangement, and uses a small angular offset `1e-10` (`149-204`).
- `findCenter` starts at `(0,0)` and runs 20 steps, initial step 20, decay 0.9, delay 0.9, retaining the best center encountered (`102-147`). The active farthest circle supplies the movement direction. `dx0`/`dy0` are initialized to zero and never assigned thereafter (`116-117`, `139-140`), so the apparent momentum blend is actually a 0.9-scaled current direction.
- `repeatedHeuristic` shuffles radii, calls the roll-around placement, recenters, and maps points back to original circle order (`241-264`). Defaults are one trial from `main` (`687-693`). Suspected bug: `bestVal` is initialized to infinity but never updated after accepting a trial (`244-262`), so every trial overwrites the incumbent and the function returns the *last* trial rather than the best. This defeats its name and makes the result depend on the final shuffle.
- Confirmed bounds bug: in `rollAroundHeuristic`, for each `j`, `pos` is only sized to cover circles through `j`, but candidate evaluation calls `findCenter(inst, pos)` using the full instance (`216-238`, especially `224`). For `j+1 < n`, `findCenter` reads `points[2*i]` through all `inst.size()` circles (`102-129`), beyond the initialized/currently allocated coordinate data. AddressSanitizer confirms a heap-buffer-overflow at line 124 on development input `10_01.txt`; see `bench/results/contest-asan.json`. Preserve the raw baseline, but use bounded slices for the Rust port and document this necessary departure. C++ quality/timing results are observational because undefined behavior can depend on allocation and compiler choices.
- `rollAround` can leave `iNext == -1` if no prior circle yields an admissible angular candidate, yet indexes `inst[i0]` after assigning it (`170-200`). This is another malformed/degenerate-geometry failure path.

### Objective and analytic gradient

Optimization vector is `x = [R, x_0, y_0, ..., x_(n-1), y_(n-1)]` (`687-725`). `computeFunGrad` minimizes `max(0,R)` plus squared positive-part penalties (`333-382`):

`P_large * max(0, x_i²+y_i²-max(0,R-r_i)²)²`

for each circle's containment in radius `R`, and

`P_pair * max(0, (r_i+r_j)²-((x_i-x_j)²+(y_i-y_j)²))²`

for each overlapping pair. The corresponding derivatives are accumulated only when a violation is strictly positive (`347-380`). For a valid positive-radius solution these encode `||p_i|| <= R-r_i` and pair separation. The objective does not itself enforce `R >= 0` smoothly: it clips its linear term at zero and clips the containment radius with `max(0,R-r_i)`.

`computeFun` repeats the same scalar objective without a gradient (`384-423`). No center coordinates are optimized: initialization/recentering and printed points use the origin as the enclosing-circle center.

### Line search and BFGS

- Strong-Wolfe search uses bracketing/doubling and a bisection `zoom` (`473-541`). The production call supplies `c1=.1` and `c2=.95` (`694-695`, `726-730`). The directional derivative is formed as `grad·direction` (`450-469`).
- Dense inverse-Hessian BFGS starts with identity; direction is `-H g`; if it is not sufficiently descending (`p·g > -1e-6 ||g||²`) the matrix resets to identity (`558-605`). The rank-two inverse-BFGS update follows accepted line-search steps (`622-655`). It returns on NaN objective/step (`571`, `614-619`, `627-631`). Suspected quirk: the NaN-step branch has a reset-and-continue after an unconditional return, so it is dead code (`614-620`). There is no curvature/finite guard before `rho=1/(y·s)` (`645-651`), so near-zero or invalid curvature can poison the matrix.
- In this variant BFGS is called with `maxSteps=0` (meaning unlimited) and gradient-square tolerance `0` (`726-730`); outer wall-clock time is the principal stopping bound. Its internal clock starts when `bfgs` starts, excluding initialization, while `main` passes remaining overall budget (`565-590`, `698`, `711-730`).

### Adaptive penalties, repair, and output

- Defaults: containment penalty `1e-7`, pair penalty `1e-5`, callback gradient-square threshold `1e-5`, violation ratio threshold `1e-5`, one initialization trial, total budget `1.95 s`, Wolfe `c1=.1`, `c2=.95` (`687-695`). CLI positional overrides follow that exact order.
- On callback iterations with `gradSqr < tolGrad`, it computes enclosing-radius ratio `radiusReal/R` and pairwise scale violation `1/minScale`; each corresponding penalty doubles when its ratio exceeds `1+tolViolation`. Callback requests a fresh objective/gradient after penalty changes (`731-755`; callback refresh in BFGS `583-589`). If neither ratio fails, callback terminates; if either fails it continues with the increased penalty. These are independent checks, and each may double in one callback.
- After each optimizer call, `makeFeasible` scales every coordinate about the origin by `(1+5e-5)/minScale`, where `minScale` is the minimum pair distance divided by summed radii (`660-684`). This guarantees positive pairwise slack in ordinary finite `n>=2` inputs, but expands the enclosing radius too. Candidate incumbent quality is computed after this repair (`758-764`). There is no separate containment repair; enclosing radius is recomputed from repaired coordinates by `computeValue` (`48-59`).
- Output is radius plus `1e-4`, then one row per original radius and centered coordinate, fixed to four decimals for radius/coordinates and zero decimals for radius (`61-71`, called at `766`). This margin is not a proof that coordinate rounding preserves all pair gaps.
- Outer loop tries up to 10,000 fresh initializations and stops if elapsed time already exceeds budget (`704-715`, `765`). Initialization itself is included in the outer elapsed budget, though a single initialization can overrun before the check. If no iteration gets as far as setting `bestPoints`, output may be empty/invalid (`704-705`, `766`).
- `makeFeasible` has no special case for fewer than two circles: the pair scan leaves `minScale=+inf`, then coordinates become zero (`660-684`). For one circle this happens to place it at the origin; the radius is still recomputed correctly. Zero circles and invalid input are not guarded.

## Comparison: `legacy/main.cpp`

This source uses a different objective and initialization, so it is a comparison solver rather than a drop-in equivalent of the candidate.

- Initialization uses `random_device` to seed `ranlux24_base`, shuffles circle indices, and samples random directions while incrementally placing circles (`119-188`). It runs `trialInit` starts, default 10, each with 50 direction samples; it retains the best initialization (`484-495`). Randomness is not reproducible from a user-visible seed.
- Likely initialization bug: `centerHeuristik` fills `shuffled[i] = idx[i]` instead of `inst[idx[i]]` (`126-132`), then passes `shuffled` as radii to `findCenter` (`173-180`). Thus center estimates use circle indices as radii, although placement and final objective use `inst` (`137-154`, `484-495`).
- It includes `fun_softmax.cpp`; the smooth enclosure term is a radius-weighted soft maximum of `d_i+r_i`, with `alpha = penaltyLarge/sqrt(sum r_i²)` and an expectation-form analytic gradient (`fun_softmax.cpp:12-42`, objective-only form `68-94`). Pair-overlap penalty is the same positive-part squared overlap as the candidate (`fun_softmax.cpp:44-65`, `96-113`). It has no explicit radius variable or containment penalty; `penaltyLarge` controls softmax sharpness rather than a constraint penalty.
- Main defaults are `penaltyLarge=10000`, `penaltyPairwise=1e-4`, ten starts, 50 inner samples, `1.95 s`, BFGS gradient-square tolerance `1e-6`, Wolfe `c1=.2`, `c2=.5` (`497-505`). It optimizes coordinate-only vectors (`421-462`) and doubles both penalty values together up to 15 times after each initialization (`516-559`), keeping the best repaired solution.
- Its `makeFeasible` is the same global radial scaling repair and has the same `n<2` infinite-minScale edge case (`464-482`). It prints the same `+1e-4` radius margin and four-decimal coordinates (`52-62`, `560`).

## Wrapper interface and evidence status

The root-owned wrapper interface is:

```sh
python3 bench/legacy.py build --variant contest
python3 bench/legacy.py build --variant main
python3 bench/legacy.py run --variant contest --input PATH --budget SECONDS
python3 bench/legacy.py run --variant main --input PATH --budget SECONDS
```

Here `contest` means the provisional `circles-hackbfgs.tr.cpp`; `main` means `legacy/main.cpp`. Builds go to ignored `bench/build/`. Build metadata records compiler flags, source hashes, legacy revision, and binary hash. Baseline results are pending.
