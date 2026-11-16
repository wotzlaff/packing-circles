//! Shared failures from instance construction, geometry, and serialization.

use crate::geometry::ValidationReport;
use std::fmt::{self, Display, Formatter};

/// Errors returned by construction, parsing, formatting, and validation.
#[derive(Debug, Clone, PartialEq)]
pub enum PackingError {
    EmptyInstance,
    InvalidRadius { index: usize, value: f64 },
    InvalidSolutionRadius(f64),
    NonFiniteCoordinate { index: usize },
    CircleCount { expected: usize, actual: usize },
    Parse(String),
    InfeasibleSolution(ValidationReport),
    RoundedOutputInfeasible(ValidationReport),
    InvalidConfiguration(&'static str),
    GeometryOverflow,
    OutputPrecisionOverflow,
}

impl Display for PackingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInstance => write!(f, "an instance must contain at least one circle"),
            Self::InvalidRadius { index, value } => write!(f, "radius {index} must be positive and finite, got {value}"),
            Self::InvalidSolutionRadius(value) => write!(f, "solution radius must be positive and finite, got {value}"),
            Self::NonFiniteCoordinate { index } => write!(f, "circle {index} has a non-finite coordinate"),
            Self::CircleCount { expected, actual } => write!(f, "expected {expected} circle centers, got {actual}"),
            Self::Parse(message) => write!(f, "input parse error: {message}"),
            Self::InfeasibleSolution(report) => write!(f, "solution is infeasible (overlap {}, containment violation {}, tolerance {})", report.max_overlap, report.max_containment_violation, report.tolerance),
            Self::RoundedOutputInfeasible(report) => write!(f, "four-decimal output is infeasible (overlap {}, containment violation {}, tolerance {})", report.max_overlap, report.max_containment_violation, report.tolerance),
            Self::InvalidConfiguration(message) => write!(f, "invalid solver configuration: {message}"),
            Self::GeometryOverflow => write!(f, "finite input values overflowed during geometry calculations"),
            Self::OutputPrecisionOverflow => write!(f, "radius cannot be represented at four-decimal output precision"),
        }
    }
}

impl std::error::Error for PackingError {}
