use packing_core::solver::{
    solve, solve_with_control_and_observer, solve_with_observer, ProgressKind, SolveOptions,
    StopReason,
};
use packing_core::{format_solution, parse_solution, validate, Instance, Solution};

struct Snapshot {
    kind: ProgressKind,
    sequence: u64,
    elapsed: f64,
    evaluations: u64,
    iterations: usize,
    restarts: usize,
    solution: Solution,
    output: String,
    violation: f64,
}

fn fixed_work() -> SolveOptions {
    SolveOptions {
        time_limit_seconds: None,
        evaluation_limit: 5000,
        ..SolveOptions::default()
    }
}

#[test]
fn progress_is_read_only_feasible_monotone_and_replayable() {
    let instance = Instance::new(vec![1., 2., 1.5, 0.75, 3.]).unwrap();
    let plain = solve(&instance, fixed_work()).unwrap();
    let mut snapshots = Vec::new();
    let traced = solve_with_observer(&instance, fixed_work(), &mut |event| {
        assert!(event.feasible);
        snapshots.push(Snapshot {
            kind: event.kind,
            sequence: event.sequence,
            elapsed: event.elapsed_seconds,
            evaluations: event.evaluations,
            iterations: event.iterations,
            restarts: event.restarts,
            solution: event.solution.clone(),
            output: event.output.to_owned(),
            violation: event.max_violation,
        });
    })
    .unwrap();
    assert_eq!(plain.output, traced.output);
    assert_eq!(
        (plain.evaluations, plain.iterations, plain.restarts),
        (traced.evaluations, traced.iterations, traced.restarts)
    );
    assert_eq!(snapshots.first().unwrap().kind, ProgressKind::Initial);
    assert_eq!(snapshots.last().unwrap().kind, ProgressKind::Final);
    for (index, event) in snapshots.iter().enumerate() {
        assert_eq!(event.sequence, index as u64);
        assert_eq!(
            format_solution(&instance, &event.solution).unwrap(),
            event.output
        );
        assert!(
            validate(
                &instance,
                &parse_solution(&instance, &event.output).unwrap()
            )
            .unwrap()
            .is_feasible
        );
        let check = validate(&instance, &event.solution).unwrap();
        assert_eq!(
            event.violation,
            check.max_overlap.max(check.max_containment_violation)
        );
        if index > 0 {
            let previous = &snapshots[index - 1];
            assert!(event.solution.radius <= previous.solution.radius);
            assert!(event.elapsed >= previous.elapsed && event.evaluations >= previous.evaluations);
            assert!(event.iterations >= previous.iterations && event.restarts >= previous.restarts);
            if event.kind == ProgressKind::Incumbent {
                assert!(event.elapsed - previous.elapsed >= 0.1);
            }
        }
    }
    let last = snapshots.last().unwrap();
    assert_eq!(last.output, traced.output);
    assert_eq!(last.evaluations, traced.evaluations);
}

#[test]
fn zero_budget_and_cancellation_always_have_initial_and_final_events() {
    let instance = Instance::new(vec![1., 2., 3.]).unwrap();
    let mut kinds = Vec::new();
    let result = solve_with_observer(
        &instance,
        SolveOptions {
            time_limit_seconds: Some(0.),
            ..fixed_work()
        },
        &mut |event| kinds.push((event.kind, event.evaluations, event.output.to_owned())),
    )
    .unwrap();
    assert_eq!(result.termination, StopReason::TimeLimit);
    assert_eq!(kinds.len(), 2);
    assert_eq!(kinds[0].0, ProgressKind::Initial);
    assert_eq!(kinds[1].0, ProgressKind::Final);
    assert_eq!(kinds[1].1, 0);
    assert_eq!(kinds[1].2, result.output);
    kinds.clear();
    let result =
        solve_with_control_and_observer(&instance, fixed_work(), &mut || true, &mut |event| {
            kinds.push((event.kind, event.evaluations, event.output.to_owned()))
        })
        .unwrap();
    assert_eq!(result.termination, StopReason::Cancelled);
    assert_eq!(kinds.len(), 2);
    assert_eq!(kinds[1].2, result.output);
}

#[test]
fn trivial_one_circle_trace_reports_exact_optimum_without_optimization() {
    let instance = Instance::new(vec![3.5]).unwrap();
    let mut counts = Vec::new();
    let result = solve_with_observer(&instance, fixed_work(), &mut |event| {
        counts.push((event.solution.radius, event.evaluations))
    })
    .unwrap();
    assert_eq!(result.termination, StopReason::Trivial);
    assert_eq!(counts, vec![(3.5, 0), (3.5, 0)]);
}
