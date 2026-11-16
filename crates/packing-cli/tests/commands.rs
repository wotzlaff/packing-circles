//! End-to-end checks of the public file format and command exit statuses.

use std::{path::PathBuf, process::Command};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_packing-cli"))
}

#[test]
fn validates_known_optimal_two_circle_packing() {
    let output = cli()
        .arg("validate")
        .arg(fixture("two_unit_circles.in"))
        .arg(fixture("two_unit_circles.out"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("feasible: true"));
}

#[test]
fn formats_output_that_the_independent_parser_accepts() {
    let input = std::fs::read_to_string(fixture("two_unit_circles.in")).unwrap();
    let instance = packing_core::parse_instance(&input).unwrap();
    let output = cli()
        .arg("format-solution")
        .arg(fixture("two_unit_circles.in"))
        .arg(fixture("two_unit_circles.out"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let solution =
        packing_core::parse_solution(&instance, &String::from_utf8(output.stdout).unwrap())
            .unwrap();
    assert!(
        packing_core::validate(&instance, &solution)
            .unwrap()
            .is_feasible
    );
}

#[test]
fn malformed_command_has_error_status_and_usage() {
    let output = cli().arg("solve").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}

#[test]
fn overlapping_packing_has_infeasible_exit_status() {
    let output = cli()
        .arg("validate")
        .arg(fixture("two_unit_circles.in"))
        .arg(fixture("two_unit_circles_overlap.out"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("feasible: false"));
}

#[test]
fn malformed_solution_has_error_exit_status() {
    let output = cli()
        .arg("validate")
        .arg(fixture("two_unit_circles.in"))
        .arg(fixture("two_unit_circles.in"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
}

#[test]
fn instance_formatting_round_trips_the_input() {
    let output = cli()
        .arg("format-instance")
        .arg(fixture("two_unit_circles.in"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let parsed = packing_core::parse_instance(&String::from_utf8(output.stdout).unwrap()).unwrap();
    assert_eq!(parsed.radii(), &[1.0, 1.0]);
}
