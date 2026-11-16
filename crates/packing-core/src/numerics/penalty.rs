//! Legacy callback scheduling for containment and overlap penalty coefficients.

use super::{min_scale, CallbackAction, NumericalError, PenaltyWeights};
use crate::{Instance, Point};

/// Mutable penalty weights and the two thresholds used by version A's callback.
#[derive(Clone, Copy, Debug)]
pub struct PenaltySchedule {
    pub weights: PenaltyWeights,
    pub gradient_tolerance_squared: f64,
    pub violation_tolerance: f64,
}

impl Default for PenaltySchedule {
    fn default() -> Self {
        Self {
            weights: PenaltyWeights::default(),
            gradient_tolerance_squared: 1e-5,
            violation_tolerance: 1e-5,
        }
    }
}

impl PenaltySchedule {
    /// Double violated constraint weights when the squared gradient is small.
    ///
    /// Returns the reference's refresh/termination flags. Radius ratios retain
    /// the ordinary legacy behavior, including a negative trial radius; zero
    /// trial radius is treated as violated without propagating an infinity.
    pub fn update(
        &mut self,
        instance: &Instance,
        state: &[f64],
        gradient_squared: f64,
    ) -> Result<CallbackAction, NumericalError> {
        self.update_impl(instance, state, gradient_squared, false)
    }

    /// Update with portable pairwise distances for cross-target sessions.
    pub fn update_portable(
        &mut self,
        instance: &Instance,
        state: &[f64],
        gradient_squared: f64,
    ) -> Result<CallbackAction, NumericalError> {
        self.update_impl(instance, state, gradient_squared, true)
    }

    fn update_impl(
        &mut self,
        instance: &Instance,
        state: &[f64],
        gradient_squared: f64,
        portable: bool,
    ) -> Result<CallbackAction, NumericalError> {
        if state.len() != 1 + 2 * instance.len() {
            return Err(NumericalError::InvalidInput(
                "penalty state dimension mismatch",
            ));
        }
        if !self.gradient_tolerance_squared.is_finite()
            || self.gradient_tolerance_squared < 0.0
            || !self.violation_tolerance.is_finite()
            || self.violation_tolerance < 0.0
            || !self.weights.containment.is_finite()
            || self.weights.containment <= 0.0
            || !self.weights.pairwise.is_finite()
            || self.weights.pairwise <= 0.0
        {
            return Err(NumericalError::InvalidInput("invalid penalty schedule"));
        }
        if state.iter().any(|x| !x.is_finite()) || !gradient_squared.is_finite() {
            return Err(NumericalError::NonFinite);
        }
        if gradient_squared < 0.0 {
            return Err(NumericalError::InvalidInput(
                "squared gradient cannot be negative",
            ));
        }
        if gradient_squared >= self.gradient_tolerance_squared {
            return Ok(CallbackAction::default());
        }
        let points: Vec<_> = state[1..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| Point::new(p[0], p[1]))
            .collect();
        // Preserve the reference's sqrt(x*x+y*y) order for component parity.
        let radius = points
            .iter()
            .zip(instance.radii())
            .fold(0.0_f64, |best, (p, r)| {
                best.max((p.x * p.x + p.y * p.y).sqrt() + r)
            });
        if !radius.is_finite() {
            return Err(NumericalError::NonFinite);
        }
        let containment_violated =
            state[0] == 0.0 || radius / state[0] > 1.0 + self.violation_tolerance;
        let scale = if portable {
            super::min_scale_portable(instance, &points)?
        } else {
            min_scale(instance, &points)?
        };
        let pairwise_violated = scale == 0.0 || 1.0 / scale > 1.0 + self.violation_tolerance;
        let weights = PenaltyWeights {
            containment: self.weights.containment * if containment_violated { 2.0 } else { 1.0 },
            pairwise: self.weights.pairwise * if pairwise_violated { 2.0 } else { 1.0 },
        };
        if !weights.containment.is_finite() || !weights.pairwise.is_finite() {
            return Err(NumericalError::NonFinite);
        }
        self.weights = weights;
        Ok(CallbackAction {
            refresh: true,
            terminate: !containment_violated && !pairwise_violated,
        })
    }
}
