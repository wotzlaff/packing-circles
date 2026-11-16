//! Strong-Wolfe line search matching the legacy BFGS search order.

use super::{Evaluation, NumericalError};

/// Parameters for the strong-Wolfe conditions and bounded search loops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSearchConfig {
    pub c1: f64,
    pub c2: f64,
    pub max_bracket_steps: usize,
    pub max_zoom_steps: usize,
}

impl Default for LineSearchConfig {
    fn default() -> Self {
        Self {
            c1: 0.1,
            c2: 0.95,
            max_bracket_steps: 100,
            max_zoom_steps: 50,
        }
    }
}

/// An accepted point on the search ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSearchStep {
    pub step: f64,
    pub value: f64,
}

/// Find a step satisfying the legacy strong-Wolfe search conditions.
///
/// `previous_value` is the objective value at the preceding BFGS iterate and
/// is used only to choose the initial trial length. Search exhaustion is
/// reported explicitly rather than returning the legacy's undefined step.
pub fn line_search<V, G>(
    x: &[f64],
    direction: &[f64],
    previous_value: Option<f64>,
    config: LineSearchConfig,
    value: &mut V,
    gradient: &mut G,
) -> Result<LineSearchStep, NumericalError>
where
    V: FnMut(&[f64]) -> Result<f64, NumericalError>,
    G: FnMut(&[f64]) -> Result<Evaluation, NumericalError>,
{
    validate(x, direction, previous_value, config)?;
    let initial = at(x, direction, 0.0)?;
    let start = checked_evaluation(gradient(&initial)?, x.len())?;
    let f0 = start.value;
    let df0 = dot(&start.gradient, direction);
    if !df0.is_finite() {
        return Err(NumericalError::NonFinite);
    }
    if df0 >= 0.0 {
        return Err(NumericalError::InvalidInput(
            "direction is not a descent direction",
        ));
    }

    let mut tb = previous_value
        .filter(|fp| fp.is_finite())
        .map(|fp| (1.01 * 2.0 * (f0 - fp) / df0).min(1.0))
        .unwrap_or(1.0);
    if !tb.is_finite() || tb <= 0.0 {
        tb = 1.0;
    }
    let mut ta = 0.0;
    let mut fta = f0;

    for k in 0..config.max_bracket_steps {
        let ftb = checked_value(value(&at(x, direction, tb)?)?)?;
        if ftb > f0 + config.c1 * tb * df0 || (k > 0 && ftb >= fta) {
            return zoom(x, direction, ta, tb, fta, f0, df0, config, value, gradient);
        }
        let eval = checked_evaluation(gradient(&at(x, direction, tb)?)?, x.len())?;
        let dftb = dot(&eval.gradient, direction);
        if !dftb.is_finite() {
            return Err(NumericalError::NonFinite);
        }
        if dftb.abs() <= -config.c2 * df0 {
            return Ok(LineSearchStep {
                step: tb,
                value: ftb,
            });
        }
        if dftb >= 0.0 {
            return zoom(x, direction, tb, ta, ftb, f0, df0, config, value, gradient);
        }
        ta = tb;
        fta = ftb;
        tb *= 2.0;
        if !tb.is_finite() {
            return Err(NumericalError::NonFinite);
        }
    }
    Err(NumericalError::LineSearchFailed)
}

#[allow(clippy::too_many_arguments)]
fn zoom<V, G>(
    x: &[f64],
    direction: &[f64],
    mut a: f64,
    mut b: f64,
    mut fa: f64,
    f0: f64,
    df0: f64,
    config: LineSearchConfig,
    value: &mut V,
    gradient: &mut G,
) -> Result<LineSearchStep, NumericalError>
where
    V: FnMut(&[f64]) -> Result<f64, NumericalError>,
    G: FnMut(&[f64]) -> Result<Evaluation, NumericalError>,
{
    for _ in 0..config.max_zoom_steps {
        let t = 0.5 * (a + b);
        let ft = checked_value(value(&at(x, direction, t)?)?)?;
        if ft > f0 + config.c1 * t * df0 || ft >= fa {
            b = t;
        } else {
            let eval = checked_evaluation(gradient(&at(x, direction, t)?)?, x.len())?;
            let dft = dot(&eval.gradient, direction);
            if !dft.is_finite() {
                return Err(NumericalError::NonFinite);
            }
            if dft.abs() <= -config.c2 * df0 {
                return Ok(LineSearchStep { step: t, value: ft });
            }
            if dft * (b - a) >= 0.0 {
                b = a;
            }
            a = t;
            fa = ft;
        }
    }
    Err(NumericalError::LineSearchFailed)
}

fn validate(
    x: &[f64],
    direction: &[f64],
    previous_value: Option<f64>,
    config: LineSearchConfig,
) -> Result<(), NumericalError> {
    if x.is_empty() || x.len() != direction.len() {
        return Err(NumericalError::InvalidInput(
            "state and direction dimensions must match and be nonempty",
        ));
    }
    if !x.iter().chain(direction).all(|v| v.is_finite())
        || previous_value.is_some_and(|v| !v.is_finite())
    {
        return Err(NumericalError::NonFinite);
    }
    if !(config.c1 > 0.0 && config.c1 < config.c2 && config.c2 < 1.0)
        || config.max_bracket_steps == 0
        || config.max_zoom_steps == 0
    {
        return Err(NumericalError::InvalidInput(
            "invalid line-search configuration",
        ));
    }
    Ok(())
}

fn at(x: &[f64], direction: &[f64], step: f64) -> Result<Vec<f64>, NumericalError> {
    let point: Vec<f64> = x.iter().zip(direction).map(|(x, d)| x + step * d).collect();
    if point.iter().all(|coordinate| coordinate.is_finite()) {
        Ok(point)
    } else {
        Err(NumericalError::NonFinite)
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |sum, (a, b)| sum + a * b)
}

fn checked_value(value: f64) -> Result<f64, NumericalError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(NumericalError::NonFinite)
    }
}

fn checked_evaluation(
    evaluation: Evaluation,
    dimension: usize,
) -> Result<Evaluation, NumericalError> {
    if !evaluation.value.is_finite() {
        return Err(NumericalError::NonFinite);
    }
    if evaluation.gradient.len() != dimension {
        return Err(NumericalError::InvalidInput(
            "evaluation has invalid value or gradient dimension",
        ));
    }
    if !evaluation.gradient.iter().all(|v| v.is_finite()) {
        return Err(NumericalError::NonFinite);
    }
    Ok(evaluation)
}
