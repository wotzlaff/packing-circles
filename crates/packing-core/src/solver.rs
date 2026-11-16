//! Version A restart orchestration and independently validated incumbents.
//!
//! Timed execution uses the native monotonic clock. Fixed-work mode disables the
//! clock limit, making seeded runs reproducible within the same build. Rendering,
//! filesystem access, and CLI formatting remain outside the optimization loop.

use crate::numerics::{
    evaluate, initialize, min_scale, minimize, repair, value, BfgsConfig, BfgsTermination,
    NumericalError, PenaltySchedule, PenaltyWeights, SplitMix64,
};
use crate::{format_solution, validate, Instance, PackingError, Point, Solution};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::time::Instant;

/// Seed and total limits shared across all initialization/optimization restarts.
#[derive(Clone, Copy, Debug)]
pub struct SolveOptions {
    pub seed: u64,
    /// `None` selects deterministic fixed-work execution. Zero returns a fallback.
    pub time_limit_seconds: Option<f64>,
    /// Scalar or value/gradient calls; initialization and validation are excluded.
    pub evaluation_limit: u64,
    /// Attempted BFGS iterations, including line searches that fail or are cut short.
    pub iteration_limit: usize,
    /// Maximum attempted rolling initializations/local optimization calls.
    pub restart_limit: usize,
}

impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            seed: 5,
            time_limit_seconds: Some(1.95),
            evaluation_limit: u64::MAX,
            iteration_limit: 1_000_000,
            restart_limit: 10_000,
        }
    }
}

/// Why the solver returned its best checked incumbent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    TimeLimit,
    EvaluationLimit,
    IterationLimit,
    RestartLimit,
    Cancelled,
    Trivial,
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TimeLimit => "time_limit",
            Self::EvaluationLimit => "evaluation_limit",
            Self::IterationLimit => "iteration_limit",
            Self::RestartLimit => "restart_limit",
            Self::Cancelled => "cancelled",
            Self::Trivial => "trivial",
        })
    }
}

/// A feasible internal solution, checked four-decimal output, and actual work.
#[derive(Clone, Debug)]
pub struct SolveResult {
    /// Radius includes export containment slack, except the exact one-circle case.
    pub solution: Solution,
    pub output: String,
    pub evaluations: u64,
    pub iterations: usize,
    pub restarts: usize,
    pub initialization_fallbacks: usize,
    /// Initializations/local searches that encountered a defined numerical failure.
    pub numerical_failures: usize,
    /// Includes fallback construction, initialization, optimization, and export.
    pub elapsed_seconds: f64,
    pub termination: StopReason,
}

struct Control<'a, C> {
    start: Instant,
    options: SolveOptions,
    cancellation: RefCell<&'a mut C>,
    evaluations: Cell<u64>,
    iterations: Cell<usize>,
    stopped: Cell<Option<StopReason>>,
}

impl<C: FnMut() -> bool> Control<'_, C> {
    fn poll(&self, check_iterations: bool) -> Result<(), NumericalError> {
        if self.stopped.get().is_some() {
            return Err(NumericalError::Cancelled);
        }
        let reason = if self
            .options
            .time_limit_seconds
            .is_some_and(|limit| self.start.elapsed().as_secs_f64() >= limit)
        {
            Some(StopReason::TimeLimit)
        } else if (self.cancellation.borrow_mut())() {
            Some(StopReason::Cancelled)
        } else if self.evaluations.get() >= self.options.evaluation_limit {
            Some(StopReason::EvaluationLimit)
        } else if check_iterations && self.iterations.get() >= self.options.iteration_limit {
            Some(StopReason::IterationLimit)
        } else {
            None
        };
        if let Some(reason) = reason {
            self.stopped.set(Some(reason));
            Err(NumericalError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn evaluation(&self) -> Result<(), NumericalError> {
        self.poll(false)?;
        self.evaluations.set(self.evaluations.get() + 1);
        Ok(())
    }
}

/// Solve up to fifty positive-radius circles with the version A algorithm.
pub fn solve(instance: &Instance, options: SolveOptions) -> Result<SolveResult, PackingError> {
    solve_with_control(instance, options, &mut || false)
}

/// Solve with cooperative cancellation polled at restart, iteration, and objective boundaries.
///
/// A valid fallback is constructed even when no optimization work is allowed.
/// One rolling initialization and final validation/export are bounded atomic
/// work units; the deadline is checked immediately before/after initialization.
/// Cancellation returns a solution rather than an error. Invalid inputs or
/// unrepresentable four-decimal geometry return a defined error.
pub fn solve_with_control<C: FnMut() -> bool>(
    instance: &Instance,
    options: SolveOptions,
    cancellation: &mut C,
) -> Result<SolveResult, PackingError> {
    let start = Instant::now();
    if options
        .time_limit_seconds
        .is_some_and(|limit| !limit.is_finite() || limit < 0.0)
    {
        return Err(PackingError::InvalidConfiguration(
            "time limit must be finite and nonnegative",
        ));
    }
    if instance.len() > 50 {
        return Err(PackingError::InvalidConfiguration(
            "version A supports at most fifty circles",
        ));
    }
    let mut best = export_candidate(instance, line_fallback(instance)?)?;
    let control = Control {
        start,
        options,
        cancellation: RefCell::new(cancellation),
        evaluations: Cell::new(0),
        iterations: Cell::new(0),
        stopped: Cell::new(None),
    };
    let mut rng = SplitMix64::new(options.seed);
    let mut restarts = 0;
    let mut initialization_fallbacks = 0;
    let mut numerical_failures = 0;
    if instance.len() == 1 {
        control.stopped.set(Some(StopReason::Trivial));
    }
    while control.stopped.get().is_none() {
        if control.poll(true).is_err() {
            break;
        }
        if restarts >= options.restart_limit {
            control.stopped.set(Some(StopReason::RestartLimit));
            break;
        }
        restarts += 1;
        let initialized = match initialize(instance, 1, &mut rng) {
            Ok(result) => result,
            Err(_) => {
                numerical_failures += 1;
                continue;
            }
        };
        initialization_fallbacks += initialized.fallback_count;
        if let Ok(solution) = repair(instance, &initialized.points, 1.00005) {
            consider(instance, solution, &mut best);
        }
        if control.poll(true).is_err() {
            break;
        }
        let scale = match min_scale(instance, &initialized.points) {
            Ok(scale) if scale > 0.0 => scale,
            _ => {
                numerical_failures += 1;
                continue;
            }
        };
        // Legacy's initial makeFeasible(..., 1-1e-3) deliberately introduces overlap.
        let points: Vec<_> = initialized
            .points
            .iter()
            .map(|p| Point::new(p.x * 0.999 / scale, p.y * 0.999 / scale))
            .collect();
        let radius = points
            .iter()
            .zip(instance.radii())
            .fold(0.0_f64, |r, (p, ri)| r.max(p.x.hypot(p.y) + ri));
        let mut state = Vec::with_capacity(1 + 2 * instance.len());
        state.push(radius - 1e-3);
        state.extend(points.iter().flat_map(|p| [p.x, p.y]));
        let weights = Cell::new(PenaltyWeights::default());
        let mut schedule = PenaltySchedule::default();
        let mut scalar = |x: &[f64]| {
            control.evaluation()?;
            value(instance, x, weights.get())
        };
        let mut gradient = |x: &[f64]| {
            control.evaluation()?;
            evaluate(instance, x, weights.get())
        };
        let mut callback = |iteration: crate::numerics::Iteration<'_>| {
            control.poll(true)?;
            schedule.weights = weights.get();
            let action = schedule.update(instance, iteration.state, iteration.gradient_squared)?;
            weights.set(schedule.weights);
            if !action.terminate {
                control.iterations.set(control.iterations.get() + 1);
            }
            Ok(action)
        };
        let config = BfgsConfig {
            max_iterations: options.iteration_limit - control.iterations.get(),
            max_evaluations: options.evaluation_limit - control.evaluations.get(),
            ..BfgsConfig::default()
        };
        let result = minimize(
            &mut state,
            config,
            &mut scalar,
            &mut gradient,
            &mut callback,
        );
        match result {
            Ok(report) => match report.termination {
                BfgsTermination::EvaluationFailed(NumericalError::EvaluationLimit) => {
                    control.stopped.set(Some(StopReason::EvaluationLimit));
                }
                BfgsTermination::EvaluationFailed(_) if control.stopped.get().is_none() => {
                    numerical_failures += 1
                }
                _ => (),
            },
            Err(_) if control.stopped.get().is_none() => numerical_failures += 1,
            Err(_) => (),
        }
        let optimized: Vec<_> = state[1..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| Point::new(p[0], p[1]))
            .collect();
        if let Ok(solution) = repair(instance, &optimized, 1.00005) {
            consider(instance, solution, &mut best);
        }
    }
    Ok(SolveResult {
        solution: best.0,
        output: best.1,
        evaluations: control.evaluations.get(),
        iterations: control.iterations.get(),
        restarts,
        initialization_fallbacks,
        numerical_failures,
        elapsed_seconds: start.elapsed().as_secs_f64(),
        termination: control.stopped.get().unwrap_or(StopReason::RestartLimit),
    })
}

fn consider(instance: &Instance, solution: Solution, best: &mut (Solution, String)) {
    // Avoid expensive formatting if even the unpadded radius cannot improve.
    if solution.radius >= best.0.radius {
        return;
    }
    if let Ok(candidate) = export_candidate(instance, solution) {
        if candidate.0.radius < best.0.radius {
            *best = candidate;
        }
    }
}

fn line_fallback(instance: &Instance) -> Result<Solution, PackingError> {
    let total = instance.radii().iter().sum::<f64>();
    let radius = total + instance.len().saturating_sub(1) as f64 * 1e-4;
    if !radius.is_finite() {
        return Err(PackingError::GeometryOverflow);
    }
    let mut edge = -radius;
    let mut circles = Vec::with_capacity(instance.len());
    for r in instance.radii() {
        let x = edge + r;
        circles.push(Point::new(x, 0.0));
        edge = x + r + 2e-4;
    }
    Ok(Solution { radius, circles })
}

fn export_candidate(
    instance: &Instance,
    mut solution: Solution,
) -> Result<(Solution, String), PackingError> {
    let report = validate(instance, &solution)?;
    if !report.is_feasible {
        return Err(PackingError::InfeasibleSolution(report));
    }
    let margin = if instance.len() == 1 { 0.0 } else { 1e-4 };
    solution.radius = solution.radius_about_origin(instance)? + margin;
    match format_solution(instance, &solution) {
        Ok(output) => return Ok((solution, output)),
        Err(PackingError::RoundedOutputInfeasible(_)) => (),
        Err(error) => return Err(error),
    }
    // Coordinate rounding loses at most sqrt(2)*1e-4 in pair separation. Leave
    // 2e-4 absolute slack if ordinary legacy repair did not already supply it.
    let mut factor = 1.0_f64;
    for i in 0..instance.len() {
        for j in 0..i {
            let distance = solution.circles[i].distance(solution.circles[j]);
            if distance == 0.0 {
                return Err(PackingError::RoundedOutputInfeasible(report));
            }
            factor = factor.max((instance.radii()[i] + instance.radii()[j] + 2e-4) / distance);
        }
    }
    if !factor.is_finite() {
        return Err(PackingError::GeometryOverflow);
    }
    for p in &mut solution.circles {
        p.x *= factor;
        p.y *= factor;
    }
    solution.radius = solution.radius_about_origin(instance)? + margin;
    let output = format_solution(instance, &solution)?;
    Ok((solution, output))
}
