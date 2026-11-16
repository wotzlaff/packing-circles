use packing_core::solver::{
    solve, solve_algorithm_with_control, solve_algorithm_with_control_and_observer, Algorithm,
    ProgressKind, SolveOptions, StopReason,
};
use packing_core::{parse_solution, validate, Instance};

#[test]
fn variants_are_deterministic_feasible_bounded_and_trace_consistent() {
    let instance = Instance::new(vec![1., 2., 1.5, 0.75, 3., 2.5, 1.2]).unwrap();
    let options = SolveOptions {
        time_limit_seconds: None,
        evaluation_limit: 5000,
        iteration_limit: 300,
        restart_limit: 10,
        ..SolveOptions::default()
    };
    for algorithm in [
        Algorithm::EarlyPenalty,
        Algorithm::TightPenalty,
        Algorithm::ShortRestarts,
    ] {
        let plain =
            solve_algorithm_with_control(&instance, options, algorithm, &mut || false).unwrap();
        let mut events = Vec::new();
        let traced = solve_algorithm_with_control_and_observer(
            &instance,
            options,
            algorithm,
            &mut || false,
            &mut |event| {
                assert!(event.feasible);
                assert!(validate(&instance, event.solution).unwrap().is_feasible);
                events.push((
                    event.kind,
                    event.solution.radius,
                    event.output.to_owned(),
                    event.evaluations,
                ));
            },
        )
        .unwrap();
        assert_eq!(plain.output, traced.output);
        assert_eq!(
            (plain.evaluations, plain.iterations, plain.restarts),
            (traced.evaluations, traced.iterations, traced.restarts)
        );
        assert!(plain.evaluations <= options.evaluation_limit);
        assert!(plain.iterations <= options.iteration_limit);
        assert!(plain.restarts <= options.restart_limit);
        assert!(validate(&instance, &plain.solution).unwrap().is_feasible);
        assert!(
            validate(
                &instance,
                &parse_solution(&instance, &plain.output).unwrap()
            )
            .unwrap()
            .is_feasible
        );
        assert_eq!(events.first().unwrap().0, ProgressKind::Initial);
        assert_eq!(events.last().unwrap().0, ProgressKind::Final);
        assert_eq!(events.last().unwrap().2, plain.output);
        assert!(events
            .windows(2)
            .all(|w| w[1].1 <= w[0].1 && w[1].3 >= w[0].3));
        let cancelled =
            solve_algorithm_with_control(&instance, options, algorithm, &mut || true).unwrap();
        assert_eq!(cancelled.termination, StopReason::Cancelled);
        assert_eq!(cancelled.evaluations, 0);
        assert!(
            validate(&instance, &cancelled.solution)
                .unwrap()
                .is_feasible
        );
    }
}

#[test]
fn explicit_a_matches_original_api() {
    let instance = Instance::new(vec![1., 2., 1.5, 0.75, 3.]).unwrap();
    let options = SolveOptions {
        time_limit_seconds: None,
        evaluation_limit: 5000,
        ..SolveOptions::default()
    };
    let original = solve(&instance, options).unwrap();
    let explicit =
        solve_algorithm_with_control(&instance, options, Algorithm::A, &mut || false).unwrap();
    assert_eq!(original.output, explicit.output);
    assert_eq!(
        (original.evaluations, original.iterations, original.restarts),
        (explicit.evaluations, explicit.iterations, explicit.restarts)
    );
}
