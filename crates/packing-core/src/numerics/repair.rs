//! Radial feasibility repair corresponding to the legacy global scaling step.

use super::NumericalError;
use crate::{Instance, Point, Solution};

/// Minimum center-distance / required-distance ratio over every pair.
/// A single circle returns one; coincident centers return zero. Repair itself
/// rejects a zero scale because radial scaling cannot separate those centers.
pub fn min_scale(instance: &Instance, points: &[Point]) -> Result<f64, NumericalError> {
    min_scale_impl(instance, points, false)
}

/// Pairwise scale using pinned software hypot.
pub fn min_scale_portable(instance: &Instance, points: &[Point]) -> Result<f64, NumericalError> {
    min_scale_impl(instance, points, true)
}

fn min_scale_impl(
    instance: &Instance,
    points: &[Point],
    portable: bool,
) -> Result<f64, NumericalError> {
    check_points(instance, points)?;
    if points.len() == 1 {
        return Ok(1.0);
    }
    let mut scale = f64::INFINITY;
    for i in 0..points.len() {
        for j in 0..i {
            let d = super::hypot(
                points[i].x - points[j].x,
                points[i].y - points[j].y,
                portable,
            );
            let required = instance.radii()[i] + instance.radii()[j];
            if !d.is_finite() || !required.is_finite() {
                return Err(NumericalError::NonFinite);
            }
            scale = scale.min(d / required);
        }
    }
    if !scale.is_finite() || scale < 0.0 {
        return Err(NumericalError::NonFinite);
    }
    Ok(scale)
}

/// Scales all centers from the origin by `factor / min_scale` and recomputes
/// the enclosing radius. Factor must be finite and at least one.
pub fn repair(
    instance: &Instance,
    points: &[Point],
    factor: f64,
) -> Result<Solution, NumericalError> {
    repair_impl(instance, points, factor, false)
}

/// Repair using pinned software hypot.
pub fn repair_portable(
    instance: &Instance,
    points: &[Point],
    factor: f64,
) -> Result<Solution, NumericalError> {
    repair_impl(instance, points, factor, true)
}

fn repair_impl(
    instance: &Instance,
    points: &[Point],
    factor: f64,
    portable: bool,
) -> Result<Solution, NumericalError> {
    if !factor.is_finite() || factor < 1.0 {
        return Err(NumericalError::InvalidInput(
            "repair factor must be finite and at least one",
        ));
    }
    check_points(instance, points)?;
    if points.len() == 1 {
        return Ok(Solution {
            radius: instance.radii()[0],
            circles: vec![Point::new(0.0, 0.0)],
        });
    }
    let scale = min_scale_impl(instance, points, portable)?;
    if scale == 0.0 {
        return Err(NumericalError::InvalidInput(
            "coincident centers cannot be radially repaired",
        ));
    }
    let multiplier = factor / scale;
    if !multiplier.is_finite() {
        return Err(NumericalError::NonFinite);
    }
    let circles: Vec<Point> = points
        .iter()
        .map(|p| Point::new(p.x * multiplier, p.y * multiplier))
        .collect();
    if circles.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err(NumericalError::NonFinite);
    }
    let radius = circles
        .iter()
        .zip(instance.radii())
        .map(|(p, r)| super::hypot(p.x, p.y, portable) + r)
        .fold(0.0_f64, f64::max);
    if !radius.is_finite() {
        return Err(NumericalError::NonFinite);
    }
    Ok(Solution { radius, circles })
}

fn check_points(instance: &Instance, points: &[Point]) -> Result<(), NumericalError> {
    if points.len() != instance.len() {
        return Err(NumericalError::InvalidInput(
            "point count must match instance",
        ));
    }
    if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err(NumericalError::NonFinite);
    }
    Ok(())
}
