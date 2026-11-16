use packing_core::session::SolverSession;
use packing_core::solver::{solve_algorithm_with_control, Algorithm, SolveOptions, StopReason};
use packing_core::{validate, Instance};

#[test]
fn pause_keeps_state_and_host_time_rejects_invalid_values() {
    let instance = Instance::new(vec![20., 30., 40.]).unwrap();
    let mut session = SolverSession::new_portable(
        instance,
        SolveOptions {
            time_limit_seconds: None,
            evaluation_limit: 100,
            ..SolveOptions::default()
        },
        Algorithm::A,
    )
    .unwrap();
    session.step(7, 0.5).unwrap();
    let before = session.result();
    for invalid in [0.4, -1., f64::NAN, f64::INFINITY] {
        assert!(session.step(64, invalid).is_err());
        assert_eq!(session.result().output, before.output);
        assert_eq!(session.result().evaluations, before.evaluations);
    }
    session.step(0, 0.5).unwrap();
    assert_eq!(session.result().evaluations, before.evaluations);
    session.cancel().unwrap();
    let cancelled = session.result();
    session.cancel().unwrap();
    session.step(64, 0.5).unwrap();
    assert_eq!(session.result().output, cancelled.output);
    assert_eq!(session.result().evaluations, cancelled.evaluations);
    assert_eq!(session.result().termination, StopReason::Cancelled);
}

#[test]
fn work_limits_at_transition_boundaries_match_the_blocking_solver() {
    let instance = Instance::new(vec![20., 35., 70.]).unwrap();
    for evaluation_limit in [0, 1, 2, 3, 7, 31, 160] {
        for iteration_limit in [0, 1, 2, 20] {
            for restart_limit in [0, 1, 2] {
                let options = SolveOptions {
                    evaluation_limit,
                    iteration_limit,
                    restart_limit,
                    time_limit_seconds: None,
                    ..SolveOptions::default()
                };
                let expected =
                    solve_algorithm_with_control(&instance, options, Algorithm::A, &mut || false)
                        .unwrap();
                let mut session =
                    SolverSession::new(instance.clone(), options, Algorithm::A).unwrap();
                while !session.is_finished() {
                    session.step(1, 0.).unwrap();
                }
                let actual = session.result();
                assert_eq!(actual.output, expected.output);
                assert_eq!(actual.evaluations, expected.evaluations);
                assert_eq!(actual.iterations, expected.iterations);
                assert_eq!(actual.restarts, expected.restarts);
                assert_eq!(actual.termination, expected.termination);
            }
        }
    }
}
#[test]
fn fixed_work_matches_blocking_for_all_variants_and_chunks() {
    let instance = Instance::new(vec![1., 2., 1.5, 0.75, 3., 2.5, 1.2]).unwrap();
    for algorithm in [
        Algorithm::A,
        Algorithm::EarlyPenalty,
        Algorithm::TightPenalty,
        Algorithm::ShortRestarts,
    ] {
        for cap in [1, 10, 100, 5000] {
            let options = SolveOptions {
                time_limit_seconds: None,
                evaluation_limit: cap,
                iteration_limit: 300,
                restart_limit: 10,
                ..SolveOptions::default()
            };
            let expected =
                solve_algorithm_with_control(&instance, options, algorithm, &mut || false).unwrap();
            for chunk in [1, 7, 64] {
                let mut session = SolverSession::new(instance.clone(), options, algorithm).unwrap();
                while !session.is_finished() {
                    session.step(chunk, 0.).unwrap();
                }
                let actual = session.result();
                assert_eq!(
                    (
                        actual.evaluations,
                        actual.iterations,
                        actual.restarts,
                        actual.termination
                    ),
                    (
                        expected.evaluations,
                        expected.iterations,
                        expected.restarts,
                        expected.termination
                    ),
                    "{algorithm:?} {cap} {chunk}"
                );
                assert_eq!(
                    actual.output, expected.output,
                    "{algorithm:?} {cap} {chunk}"
                );
            }
        }
    }
}
#[test]
fn cancellation_and_deadline_preserve_feasible_incumbent() {
    let instance = Instance::new(vec![1., 2., 3.]).unwrap();
    let mut s = SolverSession::new(
        instance.clone(),
        SolveOptions {
            time_limit_seconds: Some(1.),
            ..SolveOptions::default()
        },
        Algorithm::A,
    )
    .unwrap();
    s.step(10, 0.5).unwrap();
    s.cancel().unwrap();
    assert_eq!(s.result().termination, StopReason::Cancelled);
    assert!(
        validate(&instance, &s.result().solution)
            .unwrap()
            .is_feasible
    );
    let mut s = SolverSession::new(
        instance,
        SolveOptions {
            time_limit_seconds: Some(1.),
            ..SolveOptions::default()
        },
        Algorithm::A,
    )
    .unwrap();
    s.step(1, 1.).unwrap();
    assert_eq!(s.result().termination, StopReason::TimeLimit);
}

#[test]
fn development_instances_match_blocking() {
    for entry in std::fs::read_dir("../../bench/data/development").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "txt") {
            continue;
        }
        let instance =
            packing_core::parse_instance(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let options = SolveOptions {
            time_limit_seconds: None,
            evaluation_limit: 200,
            iteration_limit: 300,
            restart_limit: 10,
            ..SolveOptions::default()
        };
        for algorithm in [
            Algorithm::A,
            Algorithm::EarlyPenalty,
            Algorithm::TightPenalty,
            Algorithm::ShortRestarts,
        ] {
            let expected =
                solve_algorithm_with_control(&instance, options, algorithm, &mut || false).unwrap();
            for chunk in [1, 7, 64] {
                let mut session = SolverSession::new(instance.clone(), options, algorithm).unwrap();
                while !session.is_finished() {
                    session.step(chunk, 0.0).unwrap();
                }
                let actual = session.result();
                assert_eq!(
                    (
                        actual.evaluations,
                        actual.iterations,
                        actual.restarts,
                        actual.numerical_failures,
                        actual.termination
                    ),
                    (
                        expected.evaluations,
                        expected.iterations,
                        expected.restarts,
                        expected.numerical_failures,
                        expected.termination
                    ),
                    "{} {algorithm:?} {chunk}",
                    path.display()
                );
                assert_eq!(
                    actual.output,
                    expected.output,
                    "{} {algorithm:?} {chunk}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn portable_math_is_chunk_invariant() {
    let instance = Instance::new(vec![1., 2., 1.5, 0.75, 3., 2.5, 1.2]).unwrap();
    let options = SolveOptions {
        time_limit_seconds: None,
        evaluation_limit: 5000,
        iteration_limit: 300,
        restart_limit: 10,
        ..SolveOptions::default()
    };
    for algorithm in [
        Algorithm::A,
        Algorithm::EarlyPenalty,
        Algorithm::TightPenalty,
        Algorithm::ShortRestarts,
    ] {
        let mut expected = None;
        for chunk in [1, 7, 64] {
            let mut session =
                SolverSession::new_portable(instance.clone(), options, algorithm).unwrap();
            while !session.is_finished() {
                session.step(chunk, 0.).unwrap();
            }
            let r = session.result();
            assert!(validate(&instance, &r.solution).unwrap().is_feasible);
            let actual = (
                r.output,
                r.evaluations,
                r.iterations,
                r.restarts,
                r.numerical_failures,
                r.termination,
            );
            if let Some(previous) = &expected {
                assert_eq!(&actual, previous);
            }
            expected = Some(actual);
        }
    }
}
