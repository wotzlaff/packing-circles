//! Numerical components of version A, with explicit work bounds and failures.
//!
//! Optimizer states use `[R, x_0, y_0, ...]`. These may be infeasible during
//! optimization; only independently checked, repaired geometry is exportable.

pub mod bfgs;
pub mod initialization;
pub mod line_search;
pub mod objective;
pub mod penalty;
pub mod repair;

pub use bfgs::{minimize, BfgsConfig, BfgsReport, BfgsTermination, CallbackAction, Iteration};
pub use initialization::{
    initialize, initialize_portable, Initialization, RandomSource, SplitMix64,
};
pub use line_search::{line_search, LineSearchConfig, LineSearchStep};
pub use objective::{evaluate, value, PenaltyWeights};
pub use penalty::PenaltySchedule;
pub use repair::{min_scale, min_scale_portable, repair, repair_portable};

use std::fmt;

/// An objective value and its analytic gradient in optimizer-state order.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    pub value: f64,
    pub gradient: Vec<f64>,
}

/// Defined failure paths shared by the numerical kernels.
#[derive(Clone, Debug, PartialEq)]
pub enum NumericalError {
    InvalidInput(&'static str),
    NonFinite,
    EvaluationLimit,
    LineSearchFailed,
    Cancelled,
}

impl fmt::Display for NumericalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid numerical input: {message}"),
            Self::NonFinite => write!(
                formatter,
                "nonfinite numerical state or arithmetic overflow"
            ),
            Self::EvaluationLimit => write!(formatter, "objective evaluation limit reached"),
            Self::LineSearchFailed => {
                write!(formatter, "strong-Wolfe search found no accepted step")
            }
            Self::Cancelled => write!(formatter, "numerical work cancelled"),
        }
    }
}

impl std::error::Error for NumericalError {}

// Portable math is opt-in so the legacy native path retains its platform behavior.
pub(crate) fn hypot(x: f64, y: f64, portable: bool) -> f64 {
    if portable {
        libm::hypot(x, y)
    } else {
        x.hypot(y)
    }
}
