use packing_core::{
    numerics::{evaluate, value, NumericalError, PenaltyWeights},
    Instance,
};

fn instance(radii: &[f64]) -> Instance {
    Instance::new(radii.to_vec()).unwrap()
}

#[test]
fn computes_known_containment_and_pairwise_penalties() {
    let inst = instance(&[1.0, 1.0]);
    let weights = PenaltyWeights {
        containment: 0.1,
        pairwise: 0.2,
    };
    // R=2: first circle containment violation = 4-1=3; pair overlap = 4-1=3.
    let state = [2.0, 2.0, 0.0, 1.0, 0.0];
    let result = evaluate(&inst, &state, weights).unwrap();
    assert!((result.value - 4.7).abs() < 1e-12);
    assert!((result.gradient[0] + 0.2).abs() < 1e-12);
    assert!((result.gradient[1]).abs() < 1e-12);
    assert!((result.gradient[3] - 2.4).abs() < 1e-12);
    assert_eq!(value(&inst, &state, weights).unwrap(), result.value);
}

#[test]
fn analytic_gradient_matches_central_differences_for_fixed_states() {
    let inst = instance(&[0.7, 1.1, 0.45]);
    let weights = PenaltyWeights {
        containment: 0.003,
        pairwise: 0.017,
    };
    // Deterministic small LCG; choose states away from zero and violation kinks.
    let mut seed = 0x6a09_e667_f3bc_c909_u64;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 11) as f64) / ((1_u64 << 53) as f64)
    };
    let mut checked = 0;
    while checked < 100 {
        let mut state = vec![2.0 + next(), 0.2 + 2.2 * next(), 0.2 + 2.2 * next()];
        for _ in 0..4 {
            state.push(-1.8 + 3.6 * next());
        }
        let base = evaluate(&inst, &state, weights).unwrap();
        let eps = 1e-6;
        let mut differentiable = true;
        for k in 0..state.len() {
            let mut plus = state.clone();
            let mut minus = state.clone();
            plus[k] += eps;
            minus[k] -= eps;
            let numeric = (value(&inst, &plus, weights).unwrap()
                - value(&inst, &minus, weights).unwrap())
                / (2.0 * eps);
            let scale = 1.0_f64.max(numeric.abs()).max(base.gradient[k].abs());
            if (numeric - base.gradient[k]).abs() / scale >= 1e-5 {
                differentiable = false;
                break;
            }
        }
        if differentiable {
            checked += 1;
        }
    }
}

#[test]
fn accepts_negative_radius_and_rejects_invalid_inputs() {
    let inst = instance(&[1.0]);
    let weights = PenaltyWeights::default();
    let negative = evaluate(&inst, &[-2.0, 1.0, 0.0], weights).unwrap();
    assert_eq!(negative.value, 1e-7);
    assert_eq!(negative.gradient[0], 0.0);
    assert!(matches!(
        value(&inst, &[1.0], weights),
        Err(NumericalError::InvalidInput(_))
    ));
    assert!(matches!(
        value(&inst, &[1.0, f64::NAN, 0.0], weights),
        Err(NumericalError::NonFinite)
    ));
    assert!(matches!(
        value(
            &inst,
            &[1.0, 0.0, 0.0],
            PenaltyWeights {
                containment: 0.0,
                pairwise: 1.0
            }
        ),
        Err(NumericalError::InvalidInput(_))
    ));
    assert!(matches!(
        value(&inst, &[1.0, 1e200, 0.0], weights),
        Err(NumericalError::NonFinite)
    ));
    let huge_radius = instance(&[1e308]);
    assert!(matches!(
        value(&huge_radius, &[-1e308, 0.0, 0.0], weights),
        Err(NumericalError::NonFinite)
    ));
    assert!(matches!(
        evaluate(&huge_radius, &[-1e308, 0.0, 0.0], weights),
        Err(NumericalError::NonFinite)
    ));
}
