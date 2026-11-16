use packing_core::numerics::{line_search, Evaluation, LineSearchConfig, NumericalError};

fn quadratic(target: f64) -> impl Fn(&[f64]) -> Result<Evaluation, NumericalError> {
    move |x| {
        let delta = x[0] - target;
        Ok(Evaluation {
            value: delta * delta,
            gradient: vec![2.0 * delta],
        })
    }
}

#[test]
fn finds_strong_wolfe_step_for_quadratic() {
    let mut fg = quadratic(1.0);
    let mut f = |x: &[f64]| Ok((x[0] - 1.0).powi(2));
    let result = line_search(
        &[3.0],
        &[-1.0],
        None,
        LineSearchConfig::default(),
        &mut f,
        &mut fg,
    )
    .unwrap();
    assert!((result.step - 1.0).abs() < 1e-12);
    assert!((result.value - 1.0).abs() < 1e-12);
}

#[test]
fn uses_zoom_when_initial_point_fails_armijo() {
    let objective = |x: f64| -x + 2.0 * x * x;
    let mut fg = |x: &[f64]| {
        Ok(Evaluation {
            value: objective(x[0]),
            gradient: vec![-1.0 + 4.0 * x[0]],
        })
    };
    let mut f = |x: &[f64]| Ok(objective(x[0]));
    let result = line_search(
        &[0.0],
        &[1.0],
        None,
        LineSearchConfig::default(),
        &mut f,
        &mut fg,
    )
    .unwrap();
    assert!(result.step > 0.0 && result.step < 1.0);
    assert!(result.value < 1.0);
}

#[test]
fn reports_bounded_bracket_exhaustion() {
    let mut fg = |x: &[f64]| {
        Ok(Evaluation {
            value: -x[0],
            gradient: vec![-1.0],
        })
    };
    let mut f = |x: &[f64]| Ok(-x[0]);
    let config = LineSearchConfig {
        max_bracket_steps: 2,
        ..LineSearchConfig::default()
    };
    assert_eq!(
        line_search(&[0.0], &[1.0], None, config, &mut f, &mut fg),
        Err(NumericalError::LineSearchFailed)
    );
}

#[test]
fn rejects_nonfinite_objective_and_invalid_settings() {
    let mut fg = |x: &[f64]| {
        Ok(Evaluation {
            value: x[0],
            gradient: vec![-1.0],
        })
    };
    let mut nonfinite = |_: &[f64]| Ok(f64::NAN);
    assert_eq!(
        line_search(
            &[0.0],
            &[1.0],
            None,
            LineSearchConfig::default(),
            &mut nonfinite,
            &mut fg
        ),
        Err(NumericalError::NonFinite)
    );

    let mut f = |x: &[f64]| Ok(x[0]);
    let mut fg = |x: &[f64]| {
        Ok(Evaluation {
            value: x[0],
            gradient: vec![-1.0],
        })
    };
    let config = LineSearchConfig {
        c1: 0.95,
        c2: 0.1,
        ..LineSearchConfig::default()
    };
    assert!(matches!(
        line_search(&[0.0], &[1.0], None, config, &mut f, &mut fg),
        Err(NumericalError::InvalidInput(_))
    ));
}
