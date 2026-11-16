use packing_core::solver::{solve, solve_with_control, SolveOptions, StopReason};
use packing_core::{format_solution, parse_instance, parse_solution, validate, Instance};

fn fixed_work(evaluation_limit: u64) -> SolveOptions {
    SolveOptions {
        seed: 5,
        time_limit_seconds: None,
        evaluation_limit,
        iteration_limit: 1_000_000,
        restart_limit: 10_000,
    }
}

fn assert_feasible_output(instance: &Instance, result: &packing_core::solver::SolveResult) {
    assert!(validate(instance, &result.solution).unwrap().is_feasible);
    let parsed = parse_solution(instance, &result.output).unwrap();
    assert!(validate(instance, &parsed).unwrap().is_feasible);
}

#[test]
fn solves_one_circle_at_its_known_optimum() {
    let instance = Instance::new(vec![3.5]).unwrap();
    let result = solve(&instance, fixed_work(200)).unwrap();
    assert!((result.solution.radius - 3.5).abs() <= 1e-9);
    assert_eq!(result.termination, StopReason::Trivial);
    assert_feasible_output(&instance, &result);
}

#[test]
fn solves_two_circles_with_finite_feasible_geometry() {
    let instance = Instance::new(vec![1.0, 1.0]).unwrap();
    let result = solve(&instance, fixed_work(200)).unwrap();
    assert!(result.solution.radius.is_finite());
    assert!(result.solution.radius >= 2.0 - 1e-8);
    assert_feasible_output(&instance, &result);
}

#[test]
fn zero_evaluation_budget_returns_a_feasible_incumbent() {
    let instance = Instance::new(vec![1.0, 2.0, 1.5]).unwrap();
    let mut options = fixed_work(0);
    options.time_limit_seconds = Some(0.0);
    let result = solve(&instance, options).unwrap();
    assert_eq!(result.evaluations, 0);
    assert_eq!(result.termination, StopReason::TimeLimit);
    assert_feasible_output(&instance, &result);
}

#[test]
fn zero_evaluation_limit_without_deadline_stops_before_any_evaluation() {
    let instance = Instance::new(vec![1.0, 2.0, 1.5]).unwrap();
    let result = solve(&instance, fixed_work(0)).unwrap();
    assert_eq!(result.evaluations, 0);
    assert_eq!(result.termination, StopReason::EvaluationLimit);
    assert_feasible_output(&instance, &result);
}

#[test]
fn zero_iteration_limit_stops_before_any_evaluation() {
    let instance = Instance::new(vec![1.0, 2.0, 1.5]).unwrap();
    let mut options = fixed_work(100);
    options.iteration_limit = 0;
    let result = solve(&instance, options).unwrap();
    assert_eq!(result.evaluations, 0);
    assert_eq!(result.iterations, 0);
    assert_eq!(result.termination, StopReason::IterationLimit);
    assert_feasible_output(&instance, &result);
}

#[test]
fn zero_restart_limit_stops_before_any_evaluation() {
    let instance = Instance::new(vec![1.0, 2.0, 1.5]).unwrap();
    let mut options = fixed_work(100);
    options.restart_limit = 0;
    let result = solve(&instance, options).unwrap();
    assert_eq!(result.evaluations, 0);
    assert_eq!(result.restarts, 0);
    assert_eq!(result.termination, StopReason::RestartLimit);
    assert_feasible_output(&instance, &result);
}

#[test]
fn tiny_time_budget_returns_a_feasible_incumbent() {
    let instance = Instance::new(vec![1.0, 1.5, 2.0, 0.75]).unwrap();
    let mut options = fixed_work(100_000);
    options.time_limit_seconds = Some(1e-9);
    let result = solve(&instance, options).unwrap();
    assert!(result.elapsed_seconds.is_finite());
    assert_feasible_output(&instance, &result);
}

#[test]
fn cancellation_before_work_returns_a_feasible_incumbent() {
    let instance = Instance::new(vec![1.0, 2.0, 1.5]).unwrap();
    let mut cancelled = || true;
    let result = solve_with_control(&instance, fixed_work(100), &mut cancelled).unwrap();
    assert_eq!(result.termination, StopReason::Cancelled);
    assert_feasible_output(&instance, &result);
}

#[test]
fn cancellation_during_optimization_stops_within_the_evaluation_cap() {
    let instance = Instance::new(vec![1.0; 12]).unwrap();
    let mut polls = 0;
    let mut cancelled = || {
        polls += 1;
        polls >= 4
    };
    let result = solve_with_control(&instance, fixed_work(500), &mut cancelled).unwrap();
    assert_eq!(result.termination, StopReason::Cancelled);
    assert!(result.evaluations <= 500);
    assert_feasible_output(&instance, &result);
}

#[test]
fn fixed_seed_and_work_repeat_exactly() {
    let instance = Instance::new(vec![1.0, 1.25, 2.0, 0.75]).unwrap();
    let first = solve(&instance, fixed_work(250)).unwrap();
    let second = solve(&instance, fixed_work(250)).unwrap();
    assert_eq!(first.output, second.output);
    assert_eq!(first.evaluations, second.evaluations);
    assert_eq!(first.iterations, second.iterations);
    assert_eq!(first.restarts, second.restarts);
}

#[test]
fn work_limits_are_never_exceeded() {
    let instance = Instance::new(vec![1.0; 10]).unwrap();
    let mut options = fixed_work(3);
    options.iteration_limit = 2;
    options.restart_limit = 1;
    let result = solve(&instance, options).unwrap();
    assert!(result.evaluations <= 3);
    assert!(result.iterations <= 2);
    assert!(result.restarts <= 1);
    assert_feasible_output(&instance, &result);
}

#[test]
fn rejects_invalid_solver_options() {
    let instance = Instance::new(vec![1.0, 2.0]).unwrap();
    let mut options = fixed_work(10);
    options.time_limit_seconds = Some(f64::NAN);
    assert!(solve(&instance, options).is_err());
    options.time_limit_seconds = Some(-0.01);
    assert!(solve(&instance, options).is_err());
}

#[test]
fn rejects_instances_above_the_fifty_circle_solver_limit() {
    let instance = Instance::new(vec![1.0; 51]).unwrap();
    assert!(solve(&instance, fixed_work(0)).is_err());
}

#[test]
fn zero_budget_fallback_exports_mixed_tiny_radii_safely() {
    let instance = Instance::new(vec![1.0, 1e-6, 1e-6, 2.0]).unwrap();
    let mut options = fixed_work(100);
    options.time_limit_seconds = Some(0.0);
    let result = solve(&instance, options).unwrap();
    assert_eq!(result.termination, StopReason::TimeLimit);
    assert_eq!(result.evaluations, 0);
    assert_feasible_output(&instance, &result);
}

#[test]
fn solves_all_development_instances_with_valid_rounded_output() {
    let mut instances_checked = 0;
    for entry in std::fs::read_dir("../../bench/data/development").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "txt") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let instance = parse_instance(&text).unwrap();
        instances_checked += 1;
        let result = solve(&instance, fixed_work(200)).unwrap();
        assert!(result.evaluations <= 200, "{}", path.display());
        assert_feasible_output(&instance, &result);
        assert_eq!(
            format_solution(&instance, &result.solution).unwrap(),
            result.output
        );
    }
    assert_eq!(instances_checked, 50);
}
