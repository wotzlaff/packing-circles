//! Dense inverse-Hessian BFGS following the selected legacy implementation.
//!
//! Work is bounded by objective calls and attempted iterations. A caller can
//! additionally enforce a deadline or cancellation in its evaluation closures.
//! Accepted state updates are retained when a later evaluation/search fails.

use super::line_search::{line_search, LineSearchConfig};
use super::{Evaluation, NumericalError};
use std::cell::Cell;

/// Limits and stopping tolerances for one BFGS invocation.
#[derive(Clone, Copy, Debug)]
pub struct BfgsConfig {
    pub max_iterations: usize,
    pub max_evaluations: u64,
    /// Compared to the squared Euclidean gradient norm, as in the reference.
    pub gradient_tolerance_squared: f64,
    pub line_search: LineSearchConfig,
}

impl Default for BfgsConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100_000,
            max_evaluations: 1_000_000,
            gradient_tolerance_squared: 0.0,
            line_search: LineSearchConfig::default(),
        }
    }
}

/// Penalty callbacks can refresh the gradient and/or terminate an invocation.
#[derive(Clone, Copy, Debug, Default)]
pub struct CallbackAction {
    pub refresh: bool,
    pub terminate: bool,
}

/// A callback observes state before each attempted BFGS iteration.
pub struct Iteration<'a> {
    pub index: usize,
    pub state: &'a [f64],
    pub value: f64,
    pub gradient_squared: f64,
}

/// Why a bounded BFGS call returned its last accepted state.
#[derive(Clone, Debug, PartialEq)]
pub enum BfgsTermination {
    Converged,
    IterationLimit,
    Callback,
    EvaluationFailed(NumericalError),
}

/// Work counters and final objective for one BFGS call.
#[derive(Clone, Debug)]
pub struct BfgsReport {
    pub iterations: usize,
    pub evaluations: u64,
    pub hessian_resets: usize,
    pub value: f64,
    pub gradient_squared: f64,
    pub termination: BfgsTermination,
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).fold(0.0, |sum, (a, b)| sum + a * b)
}

fn identity(size: usize) -> Vec<f64> {
    let mut matrix = vec![0.0; size * size];
    for index in 0..size {
        matrix[index * size + index] = 1.0;
    }
    matrix
}

fn checked(evaluation: Evaluation, size: usize) -> Result<Evaluation, NumericalError> {
    if evaluation.gradient.len() != size {
        return Err(NumericalError::InvalidInput("gradient dimension mismatch"));
    }
    if !evaluation.value.is_finite() || evaluation.gradient.iter().any(|v| !v.is_finite()) {
        return Err(NumericalError::NonFinite);
    }
    Ok(evaluation)
}

/// Minimize in place with legacy line-search and inverse-BFGS update ordering.
///
/// Each scalar or value/gradient closure call consumes one evaluation. Callbacks
/// run before the iteration, including a zero-iteration-limit call only if an
/// iteration is attempted. An initial evaluation failure returns an error;
/// subsequent failures return a report and preserve the last accepted state.
/// Nonpositive/degenerate curvature resets the Hessian instead of dividing by
/// zero. Callback refresh intentionally retains the prior squared norm until
/// the next accepted step, matching the reference's scheduling behavior.
pub fn minimize<V, G, C>(
    state: &mut [f64],
    config: BfgsConfig,
    value: &mut V,
    gradient: &mut G,
    callback: &mut C,
) -> Result<BfgsReport, NumericalError>
where
    V: FnMut(&[f64]) -> Result<f64, NumericalError>,
    G: FnMut(&[f64]) -> Result<Evaluation, NumericalError>,
    C: FnMut(Iteration<'_>) -> Result<CallbackAction, NumericalError>,
{
    if state.is_empty() || state.iter().any(|v| !v.is_finite()) {
        return Err(NumericalError::InvalidInput(
            "BFGS requires a finite nonempty state",
        ));
    }
    if !config.gradient_tolerance_squared.is_finite() || config.gradient_tolerance_squared < 0.0 {
        return Err(NumericalError::InvalidInput(
            "gradient tolerance must be finite and nonnegative",
        ));
    }
    let search = config.line_search;
    if !(search.c1 > 0.0 && search.c1 < search.c2 && search.c2 < 1.0)
        || search.max_bracket_steps == 0
        || search.max_zoom_steps == 0
    {
        return Err(NumericalError::InvalidInput(
            "invalid line-search configuration",
        ));
    }
    let size = state.len();
    size.checked_mul(size)
        .ok_or(NumericalError::InvalidInput("matrix dimension overflow"))?;
    let evaluations = Cell::new(0_u64);
    let count = || {
        if evaluations.get() >= config.max_evaluations {
            Err(NumericalError::EvaluationLimit)
        } else {
            evaluations.set(evaluations.get() + 1);
            Ok(())
        }
    };
    let mut evaluate_value = |point: &[f64]| {
        count()?;
        let value = value(point)?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(NumericalError::NonFinite)
        }
    };
    let mut evaluate_gradient = |point: &[f64]| {
        count()?;
        checked(gradient(point)?, size)
    };
    let mut current = evaluate_gradient(state)?;
    let mut norm_squared = dot(&current.gradient, &current.gradient);
    if !norm_squared.is_finite() {
        return Err(NumericalError::NonFinite);
    }
    let mut matrix = identity(size);
    let mut previous_value = None;
    let mut report = BfgsReport {
        iterations: 0,
        evaluations: 0,
        hessian_resets: 0,
        value: current.value,
        gradient_squared: norm_squared,
        termination: BfgsTermination::IterationLimit,
    };
    if norm_squared == 0.0 || norm_squared < config.gradient_tolerance_squared {
        report.termination = BfgsTermination::Converged;
    } else {
        for index in 0..config.max_iterations {
            let action = match callback(Iteration {
                index,
                state,
                value: current.value,
                gradient_squared: norm_squared,
            }) {
                Ok(action) => action,
                Err(error) => {
                    report.termination = BfgsTermination::EvaluationFailed(error);
                    break;
                }
            };
            if action.refresh {
                match evaluate_gradient(state) {
                    Ok(updated) => current = updated,
                    Err(error) => {
                        report.termination = BfgsTermination::EvaluationFailed(error);
                        break;
                    }
                }
            }
            if action.terminate {
                report.termination = BfgsTermination::Callback;
                break;
            }
            report.iterations += 1;
            let mut direction = vec![0.0; size];
            for i in 0..size {
                for j in 0..size {
                    direction[i] -= matrix[i * size + j] * current.gradient[j];
                }
            }
            let slope = dot(&direction, &current.gradient);
            if !slope.is_finite() {
                report.termination = BfgsTermination::EvaluationFailed(NumericalError::NonFinite);
                break;
            }
            if slope > -1e-6 * norm_squared {
                matrix = identity(size);
                report.hessian_resets += 1;
                continue;
            }
            let step = match line_search(
                state,
                &direction,
                previous_value,
                config.line_search,
                &mut evaluate_value,
                &mut evaluate_gradient,
            ) {
                Ok(step) => step,
                Err(error) => {
                    report.termination = BfgsTermination::EvaluationFailed(error);
                    break;
                }
            };
            let next: Vec<_> = state
                .iter()
                .zip(&direction)
                .map(|(x, p)| x + step.step * p)
                .collect();
            if next.iter().any(|v| !v.is_finite()) {
                report.termination = BfgsTermination::EvaluationFailed(NumericalError::NonFinite);
                break;
            }
            let updated = match evaluate_gradient(&next) {
                Ok(updated) => updated,
                Err(error) => {
                    report.termination = BfgsTermination::EvaluationFailed(error);
                    break;
                }
            };
            let next_norm = dot(&updated.gradient, &updated.gradient);
            if !next_norm.is_finite() {
                report.termination = BfgsTermination::EvaluationFailed(NumericalError::NonFinite);
                break;
            }
            let change: Vec<_> = updated
                .gradient
                .iter()
                .zip(&current.gradient)
                .map(|(a, b)| a - b)
                .collect();
            let mut product = vec![0.0; size];
            for i in 0..size {
                for j in 0..size {
                    product[i] += matrix[i * size + j] * change[j];
                }
            }
            let quadratic = dot(&product, &change);
            let curvature = step.step * dot(&change, &direction);
            if !curvature.is_finite() || curvature <= 0.0 || !quadratic.is_finite() {
                matrix = identity(size);
                report.hessian_resets += 1;
            } else {
                let rho = 1.0 / curvature;
                for i in 0..size {
                    for j in 0..size {
                        matrix[i * size + j] += step.step
                            * rho
                            * ((1.0 + rho * quadratic) * step.step * direction[i] * direction[j]
                                - direction[i] * product[j]
                                - direction[j] * product[i]);
                    }
                }
                if matrix.iter().any(|v| !v.is_finite()) {
                    matrix = identity(size);
                    report.hessian_resets += 1;
                }
            }
            previous_value = Some(current.value);
            state.copy_from_slice(&next);
            current = updated;
            norm_squared = next_norm;
            if norm_squared == 0.0 || norm_squared < config.gradient_tolerance_squared {
                report.termination = BfgsTermination::Converged;
                break;
            }
        }
    }
    report.value = current.value;
    report.gradient_squared = norm_squared;
    report.evaluations = evaluations.get();
    Ok(report)
}
