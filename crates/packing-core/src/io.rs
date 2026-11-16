//! Parsers and checked formatter for the contest text formats.

use crate::geometry::{check_solution_values, validate};
use crate::{Instance, PackingError, Point, Solution};

/// Parses contest input: a circle count followed by that many radii.
pub fn parse_instance(input: &str) -> Result<Instance, PackingError> {
    let tokens = input.split_whitespace().collect::<Vec<_>>();
    let count = tokens
        .first()
        .ok_or_else(|| PackingError::Parse("missing circle count".into()))?
        .parse::<usize>()
        .map_err(|_| PackingError::Parse("circle count must be a nonnegative integer".into()))?;
    if count == 0 {
        return Err(PackingError::EmptyInstance);
    }
    if tokens.len().saturating_sub(1) != count {
        return Err(PackingError::Parse(format!(
            "expected {count} radii, found {}",
            tokens.len().saturating_sub(1)
        )));
    }
    let mut radii = Vec::with_capacity(count);
    for token in &tokens[1..] {
        radii.push(
            token
                .parse::<f64>()
                .map_err(|_| PackingError::Parse(format!("invalid radius token {token:?}")))?,
        );
    }
    Instance::new(radii)
}

/// Parses contest output: enclosing radius followed by rows of original radius, x, y.
pub fn parse_solution(instance: &Instance, input: &str) -> Result<Solution, PackingError> {
    let tokens = input.split_whitespace().collect::<Vec<_>>();
    let expected = 1 + 3 * instance.len();
    if tokens.len() != expected {
        return Err(PackingError::Parse(format!(
            "expected a radius and {} radius/x/y rows, found {} tokens",
            instance.len(),
            tokens.len()
        )));
    }
    let parse_number = |index: usize, description: &str| {
        tokens[index].parse::<f64>().map_err(|_| {
            PackingError::Parse(format!("invalid {description} token {:?}", tokens[index]))
        })
    };
    let radius = parse_number(0, "enclosing radius")?;
    let mut circles = Vec::with_capacity(instance.len());
    for index in 0..instance.len() {
        let row = 1 + 3 * index;
        let radius = parse_number(row, "circle radius")?;
        if radius != instance.radii()[index] {
            return Err(PackingError::Parse(format!(
                "circle radius at row {index} does not match the input order"
            )));
        }
        circles.push(Point::new(
            parse_number(row + 1, "x coordinate")?,
            parse_number(row + 2, "y coordinate")?,
        ));
    }
    let solution = Solution { radius, circles };
    check_solution_values(&solution)?;
    Ok(solution)
}

/// Formats a feasible solution with four decimals and checks the serialized result.
///
/// Radius rounds upward to preserve containment slack; coordinates round to nearest.
/// Since coordinate rounding can reduce pairwise gaps, an infeasible rounded result
/// is rejected instead of being silently emitted.
pub fn format_solution(instance: &Instance, solution: &Solution) -> Result<String, PackingError> {
    let report = validate(instance, solution)?;
    if !report.is_feasible {
        return Err(PackingError::InfeasibleSolution(report));
    }
    let scaled_radius = solution.radius * 10_000.0;
    if !scaled_radius.is_finite() {
        return Err(PackingError::OutputPrecisionOverflow);
    }
    let rounded_radius = scaled_radius.ceil() / 10_000.0;
    let mut output = format!("{rounded_radius:.4}\n");
    for (&radius, point) in instance.radii().iter().zip(&solution.circles) {
        output.push_str(&format!("{radius} {:.4} {:.4}\n", point.x, point.y));
    }
    let reparsed = parse_solution(instance, &output)?;
    let rounded_report = validate(instance, &reparsed)?;
    if !rounded_report.is_feasible {
        return Err(PackingError::RoundedOutputInfeasible(rounded_report));
    }
    Ok(output)
}
