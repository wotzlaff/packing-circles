//! Deterministic work and time limits shared by future solver implementations.

use crate::PackingError;

/// Deterministic work and time limits for a future optimizer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolverConfig {
    /// Seed reserved for reproducible optimizer initialization.
    pub seed: u64,
    /// Wall-clock budget in seconds. Zero means no wall-clock work is allowed.
    pub budget_seconds: f64,
    /// Maximum objective/geometry evaluations. Zero means no evaluations are allowed.
    pub evaluation_limit: u64,
}

impl SolverConfig {
    /// Creates validated optimizer limits; algorithm-specific parameters come later.
    pub fn new(
        seed: u64,
        budget_seconds: f64,
        evaluation_limit: u64,
    ) -> Result<Self, PackingError> {
        if !budget_seconds.is_finite() || budget_seconds < 0.0 {
            return Err(PackingError::InvalidConfiguration(
                "budget_seconds must be finite and nonnegative",
            ));
        }
        Ok(Self {
            seed,
            budget_seconds,
            evaluation_limit,
        })
    }
}
