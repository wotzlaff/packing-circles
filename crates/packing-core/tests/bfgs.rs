use packing_core::numerics::{
    minimize, BfgsConfig, BfgsTermination, CallbackAction, Evaluation, NumericalError,
    PenaltySchedule,
};
use packing_core::Instance;

fn value(x: &[f64]) -> Result<f64, NumericalError> {
    Ok(x[0] * x[0] + 2.0 * x[1] * x[1])
}

fn gradient(x: &[f64]) -> Result<Evaluation, NumericalError> {
    Ok(Evaluation {
        value: value(x)?,
        gradient: vec![2.0 * x[0], 4.0 * x[1]],
    })
}

#[test]
fn minimizes_positive_definite_quadratic() {
    let mut x = [4.0, -2.0];
    let config = BfgsConfig {
        gradient_tolerance_squared: 1e-20,
        ..BfgsConfig::default()
    };
    let report = minimize(&mut x, config, &mut value, &mut gradient, &mut |_| {
        Ok(CallbackAction::default())
    })
    .unwrap();
    assert_eq!(report.termination, BfgsTermination::Converged);
    assert!(x.iter().all(|v| v.abs() < 1e-9));
    assert!(report.evaluations > report.iterations as u64);
}

#[test]
fn work_limit_preserves_last_accepted_state() {
    let mut x = [4.0, -2.0];
    let config = BfgsConfig {
        max_evaluations: 1,
        ..BfgsConfig::default()
    };
    let report = minimize(&mut x, config, &mut value, &mut gradient, &mut |_| {
        Ok(CallbackAction::default())
    })
    .unwrap();
    assert_eq!(x, [4.0, -2.0]);
    assert_eq!(report.evaluations, 1);
    assert_eq!(
        report.termination,
        BfgsTermination::EvaluationFailed(NumericalError::EvaluationLimit)
    );
}

#[test]
fn iteration_limit_and_cancellation_are_explicit() {
    let mut x = [4.0, -2.0];
    let config = BfgsConfig {
        max_iterations: 0,
        ..BfgsConfig::default()
    };
    let report = minimize(&mut x, config, &mut value, &mut gradient, &mut |_| {
        Ok(CallbackAction::default())
    })
    .unwrap();
    assert_eq!(report.termination, BfgsTermination::IterationLimit);
    assert_eq!(report.iterations, 0);
    let report = minimize(
        &mut x,
        BfgsConfig::default(),
        &mut value,
        &mut gradient,
        &mut |_| Err(NumericalError::Cancelled),
    )
    .unwrap();
    assert_eq!(
        report.termination,
        BfgsTermination::EvaluationFailed(NumericalError::Cancelled)
    );
    assert_eq!(x, [4.0, -2.0]);
}

#[test]
fn refreshing_penalties_does_not_replace_the_iterate() {
    use std::cell::Cell;
    let offset = Cell::new(0.0);
    let mut x = [4.0, -2.0];
    let report = minimize(
        &mut x,
        BfgsConfig::default(),
        &mut |x| Ok(value(x)? + offset.get()),
        &mut |x| {
            let mut result = gradient(x)?;
            result.value += offset.get();
            Ok(result)
        },
        &mut |_| {
            offset.set(7.0);
            Ok(CallbackAction {
                refresh: true,
                terminate: true,
            })
        },
    )
    .unwrap();
    assert_eq!(x, [4.0, -2.0]);
    assert_eq!(report.value, 31.0);
    assert_eq!(report.evaluations, 2);
    assert_eq!(report.termination, BfgsTermination::Callback);
}

#[test]
fn callback_doubles_constraints_independently() {
    let instance = Instance::new(vec![1.0, 1.0]).unwrap();
    let mut schedule = PenaltySchedule::default();
    let original = schedule.weights;
    // Circles are tangent but their real enclosing radius exceeds the trial R.
    let action = schedule
        .update(&instance, &[1.0, -1.0, 0.0, 1.0, 0.0], 0.0)
        .unwrap();
    assert_eq!(schedule.weights.containment, 2.0 * original.containment);
    assert_eq!(schedule.weights.pairwise, original.pairwise);
    assert!(action.refresh && !action.terminate);
    let action = schedule
        .update(&instance, &[3.0, -0.5, 0.0, 0.5, 0.0], 0.0)
        .unwrap();
    assert_eq!(schedule.weights.pairwise, 2.0 * original.pairwise);
    assert!(action.refresh && !action.terminate);
    let action = schedule
        .update(&instance, &[2.0, -1.0, 0.0, 1.0, 0.0], 0.0)
        .unwrap();
    assert!(action.refresh && action.terminate);
    let action = schedule
        .update(&instance, &[2.0, -1.0, 0.0, 1.0, 0.0], 1e-5)
        .unwrap();
    assert!(!action.refresh && !action.terminate);
}

#[test]
fn invalid_numerical_inputs_do_not_enter_the_optimizer() {
    let config = BfgsConfig {
        max_evaluations: 0,
        ..BfgsConfig::default()
    };
    assert!(matches!(
        minimize(
            &mut [1.0, 2.0],
            config,
            &mut value,
            &mut gradient,
            &mut |_| Ok(CallbackAction::default())
        ),
        Err(NumericalError::EvaluationLimit)
    ));
    assert!(minimize(
        &mut [],
        BfgsConfig::default(),
        &mut value,
        &mut gradient,
        &mut |_| Ok(CallbackAction::default())
    )
    .is_err());
}

#[test]
fn coincident_centers_increase_pair_penalty_without_dividing_by_zero() {
    let instance = Instance::new(vec![1.0, 1.0]).unwrap();
    let mut schedule = PenaltySchedule::default();
    let original = schedule.weights;
    let action = schedule
        .update(&instance, &[2.0, 0.0, 0.0, 0.0, 0.0], 0.0)
        .unwrap();
    assert_eq!(schedule.weights.pairwise, original.pairwise * 2.0);
    assert_eq!(schedule.weights.containment, original.containment);
    assert!(action.refresh && !action.terminate);
}

#[test]
fn exact_stationarity_converges_with_zero_default_tolerance() {
    let mut state = [0.0, 0.0];
    let report = minimize(
        &mut state,
        BfgsConfig::default(),
        &mut value,
        &mut gradient,
        &mut |_| Ok(CallbackAction::default()),
    )
    .unwrap();
    assert_eq!(report.termination, BfgsTermination::Converged);
    assert_eq!(report.evaluations, 1);
}

#[test]
fn underflowing_curvature_resets_hessian_and_keeps_finite_state() {
    // Strong-Wolfe accepts this step at subnormal precision, but sᵀy rounds to
    // zero. The legacy reciprocal would poison H; the port retains the iterate.
    let slope = 3e-162;
    let mut state = [0.0];
    let mut f = |x: &[f64]| Ok(slope * x[0] + 0.005 * x[0] * x[0]);
    let mut fg = |x: &[f64]| {
        Ok(Evaluation {
            value: slope * x[0] + 0.005 * x[0] * x[0],
            gradient: vec![slope + 0.01 * x[0]],
        })
    };
    let config = BfgsConfig {
        max_iterations: 1,
        ..BfgsConfig::default()
    };
    let report = minimize(&mut state, config, &mut f, &mut fg, &mut |_| {
        Ok(CallbackAction::default())
    })
    .unwrap();
    assert_eq!(report.hessian_resets, 1);
    assert!(report.value.is_finite() && report.gradient_squared.is_finite());
    assert!(state[0].is_finite() && state[0] < 0.0);
}

#[test]
fn penalty_weight_overflow_keeps_the_previous_schedule() {
    let instance = Instance::new(vec![1.0, 1.0]).unwrap();
    let mut schedule = PenaltySchedule::default();
    schedule.weights.containment = f64::MAX;
    let original = schedule.weights;
    let result = schedule.update(&instance, &[1.0, -1.0, 0.0, 1.0, 0.0], 0.0);
    assert!(matches!(result, Err(NumericalError::NonFinite)));
    assert_eq!(schedule.weights, original);
}
