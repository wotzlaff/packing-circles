//! Legacy-compatible explicit-radius squared-violation objective.

use super::{Evaluation, NumericalError};
use crate::Instance;

/// Penalty coefficients for containment and pairwise overlap violations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PenaltyWeights {
    /// Weight for circles extending beyond the enclosing circle.
    pub containment: f64,
    /// Weight for overlapping circle pairs.
    pub pairwise: f64,
}

impl Default for PenaltyWeights {
    fn default() -> Self {
        Self {
            containment: 1e-7,
            pairwise: 1e-5,
        }
    }
}

/// Computes the objective and analytic gradient for `[R, x0, y0, ...]`.
///
/// The explicit radius may be negative during optimization, as in the reference
/// implementation. Radii in `instance` remain strictly positive.
pub fn evaluate(
    instance: &Instance,
    state: &[f64],
    weights: PenaltyWeights,
) -> Result<Evaluation, NumericalError> {
    validate(instance, state, weights)?;
    let mut gradient = vec![0.0; state.len()];
    let mut objective = state[0].max(0.0);
    gradient[0] = if state[0] > 0.0 { 1.0 } else { 0.0 };
    let n = instance.len();

    for i in 0..n {
        let xi = state[1 + 2 * i];
        let yi = state[1 + 2 * i + 1];
        let ri = instance.radii()[i];
        let radial_delta = state[0] - ri;
        if !radial_delta.is_finite() {
            return Err(NumericalError::NonFinite);
        }
        let radial = radial_delta.max(0.0);
        let violation = xi * xi + yi * yi - radial * radial;
        if !violation.is_finite() {
            return Err(NumericalError::NonFinite);
        }
        if violation > 0.0 {
            let term = weights.containment * violation * violation;
            let gr = -4.0 * weights.containment * violation * radial;
            let gx = 4.0 * weights.containment * violation * xi;
            let gy = 4.0 * weights.containment * violation * yi;
            if !term.is_finite() || !gr.is_finite() || !gx.is_finite() || !gy.is_finite() {
                return Err(NumericalError::NonFinite);
            }
            objective += term;
            gradient[0] += gr;
            gradient[1 + 2 * i] += gx;
            gradient[1 + 2 * i + 1] += gy;
        }
    }

    for i in 0..n {
        let xi = state[1 + 2 * i];
        let yi = state[1 + 2 * i + 1];
        let ri = instance.radii()[i];
        for j in 0..i {
            let dx = xi - state[1 + 2 * j];
            let dy = yi - state[1 + 2 * j + 1];
            let sum_r = ri + instance.radii()[j];
            let violation = sum_r * sum_r - (dx * dx + dy * dy);
            if !violation.is_finite() {
                return Err(NumericalError::NonFinite);
            }
            if violation > 0.0 {
                let term = weights.pairwise * violation * violation;
                let gx = 4.0 * weights.pairwise * violation * dx;
                let gy = 4.0 * weights.pairwise * violation * dy;
                if !term.is_finite() || !gx.is_finite() || !gy.is_finite() {
                    return Err(NumericalError::NonFinite);
                }
                objective += term;
                gradient[1 + 2 * i] -= gx;
                gradient[1 + 2 * j] += gx;
                gradient[1 + 2 * i + 1] -= gy;
                gradient[1 + 2 * j + 1] += gy;
            }
        }
    }
    if !objective.is_finite() || gradient.iter().any(|x| !x.is_finite()) {
        return Err(NumericalError::NonFinite);
    }
    Ok(Evaluation {
        value: objective,
        gradient,
    })
}

/// Computes only the objective, avoiding allocation and gradient arithmetic.
pub fn value(
    instance: &Instance,
    state: &[f64],
    weights: PenaltyWeights,
) -> Result<f64, NumericalError> {
    validate(instance, state, weights)?;
    let mut objective = state[0].max(0.0);
    let n = instance.len();
    for i in 0..n {
        let xi = state[1 + 2 * i];
        let yi = state[1 + 2 * i + 1];
        let radial_delta = state[0] - instance.radii()[i];
        if !radial_delta.is_finite() {
            return Err(NumericalError::NonFinite);
        }
        let radial = radial_delta.max(0.0);
        let violation = xi * xi + yi * yi - radial * radial;
        if !violation.is_finite() {
            return Err(NumericalError::NonFinite);
        }
        if violation > 0.0 {
            objective += weights.containment * violation * violation;
        }
        if !objective.is_finite() {
            return Err(NumericalError::NonFinite);
        }
    }
    for i in 0..n {
        let xi = state[1 + 2 * i];
        let yi = state[1 + 2 * i + 1];
        let ri = instance.radii()[i];
        for j in 0..i {
            let dx = xi - state[1 + 2 * j];
            let dy = yi - state[1 + 2 * j + 1];
            let sum_r = ri + instance.radii()[j];
            let violation = sum_r * sum_r - (dx * dx + dy * dy);
            if !violation.is_finite() {
                return Err(NumericalError::NonFinite);
            }
            if violation > 0.0 {
                objective += weights.pairwise * violation * violation;
            }
            if !objective.is_finite() {
                return Err(NumericalError::NonFinite);
            }
        }
    }
    Ok(objective)
}

fn validate(
    instance: &Instance,
    state: &[f64],
    weights: PenaltyWeights,
) -> Result<(), NumericalError> {
    if state.len() != 1 + 2 * instance.len() {
        return Err(NumericalError::InvalidInput("state has wrong length"));
    }
    if state.iter().any(|x| !x.is_finite()) {
        return Err(NumericalError::NonFinite);
    }
    if !weights.containment.is_finite()
        || !weights.pairwise.is_finite()
        || weights.containment <= 0.0
        || weights.pairwise <= 0.0
    {
        return Err(NumericalError::InvalidInput(
            "penalty weights must be positive and finite",
        ));
    }
    Ok(())
}
