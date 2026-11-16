//! Circle packing geometry, objective-independent measures, and feasibility checks.

use crate::PackingError;

/// A circle center in the origin-centered enclosing-circle coordinate frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Horizontal coordinate, in input length units.
    pub x: f64,
    /// Vertical coordinate, in input length units.
    pub y: f64,
}

impl Point {
    /// Creates a point. Finiteness is checked when used in a solution.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another point.
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

/// Circle radii in their original input order.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    radii: Vec<f64>,
}

impl Instance {
    /// Builds a nonempty instance with strictly positive, finite radii.
    pub fn new(radii: Vec<f64>) -> Result<Self, PackingError> {
        if radii.is_empty() {
            return Err(PackingError::EmptyInstance);
        }
        for (index, &value) in radii.iter().enumerate() {
            if !value.is_finite() || value <= 0.0 {
                return Err(PackingError::InvalidRadius { index, value });
            }
        }
        Ok(Self { radii })
    }

    /// Returns radii in original input order.
    pub fn radii(&self) -> &[f64] {
        &self.radii
    }

    /// Number of circles in the instance.
    pub fn len(&self) -> usize {
        self.radii.len()
    }

    /// An instance is never empty because construction rejects empty input.
    pub fn is_empty(&self) -> bool {
        self.radii.is_empty()
    }

    /// Area-derived lower bound sqrt(sum(r_i squared)) for enclosing radius.
    /// This bound is not generally attainable.
    pub fn area_lower_bound(&self) -> f64 {
        self.radii
            .iter()
            .fold(0.0_f64, |sum, radius| sum.hypot(*radius))
    }
}

/// A proposed enclosing radius and centers in the corresponding frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    /// Enclosing circle radius, centered at the origin.
    pub radius: f64,
    /// One center per instance radius, in the same order.
    pub circles: Vec<Point>,
}

impl Solution {
    /// Recomputes the smallest origin-centered enclosing radius for these centers.
    pub fn radius_about_origin(&self, instance: &Instance) -> Result<f64, PackingError> {
        check_count(instance, self)?;
        check_solution_values(self)?;
        let result = instance
            .radii
            .iter()
            .zip(&self.circles)
            .map(|(&radius, &point)| point.x.hypot(point.y) + radius)
            .fold(0.0_f64, f64::max);
        if !result.is_finite() {
            return Err(PackingError::GeometryOverflow);
        }
        Ok(result)
    }
}

/// Tolerance and maximum geometric violations from independent validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValidationReport {
    /// Largest positive pairwise overlap, clamped to zero when separated.
    pub max_overlap: f64,
    /// Largest positive distance(center, origin) + circle radius - enclosing radius.
    pub max_containment_violation: f64,
    /// Numerical allowance: 1e-9 * max(1, enclosing radius, largest circle radius).
    pub tolerance: f64,
    /// True when both reported violations are within tolerance.
    pub is_feasible: bool,
}

/// Checks count, finite values, containment, and all pairwise distances.
pub fn validate(
    instance: &Instance,
    solution: &Solution,
) -> Result<ValidationReport, PackingError> {
    check_count(instance, solution)?;
    check_solution_values(solution)?;
    let max_radius = instance.radii.iter().copied().fold(0.0_f64, f64::max);
    let tolerance = 1e-9 * 1.0_f64.max(solution.radius).max(max_radius);
    let mut max_overlap: f64 = 0.0;
    let mut max_containment_violation: f64 = 0.0;
    for (index, (&radius, &point)) in instance.radii.iter().zip(&solution.circles).enumerate() {
        let containment = point.x.hypot(point.y) + radius - solution.radius;
        if !containment.is_finite() {
            return Err(PackingError::GeometryOverflow);
        }
        max_containment_violation = max_containment_violation.max(containment.max(0.0));
        for other_index in 0..index {
            let required = radius + instance.radii[other_index];
            if !required.is_finite() {
                return Err(PackingError::GeometryOverflow);
            }
            let distance = point.distance(solution.circles[other_index]);
            let overlap = required - distance;
            if overlap.is_nan() {
                return Err(PackingError::GeometryOverflow);
            }
            max_overlap = max_overlap.max(overlap.max(0.0));
        }
    }
    Ok(ValidationReport {
        max_overlap,
        max_containment_violation,
        tolerance,
        is_feasible: max_overlap <= tolerance && max_containment_violation <= tolerance,
    })
}

pub(crate) fn check_count(instance: &Instance, solution: &Solution) -> Result<(), PackingError> {
    if instance.len() != solution.circles.len() {
        return Err(PackingError::CircleCount {
            expected: instance.len(),
            actual: solution.circles.len(),
        });
    }
    Ok(())
}

pub(crate) fn check_solution_values(solution: &Solution) -> Result<(), PackingError> {
    if !solution.radius.is_finite() || solution.radius <= 0.0 {
        return Err(PackingError::InvalidSolutionRadius(solution.radius));
    }
    for (index, point) in solution.circles.iter().enumerate() {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(PackingError::NonFiniteCoordinate { index });
        }
    }
    Ok(())
}
