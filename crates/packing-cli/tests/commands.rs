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
    let output = cli().arg("validate").output().unwrap();
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

#[test]
fn solve_reads_standard_input_and_keeps_summary_off_stdout() {
    let mut command = cli();
    command
        .arg("solve")
        .arg("-")
        .arg("--fixed-work")
        .arg("--evaluations")
        .arg("0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(b"2\n1\n1\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let instance = packing_core::parse_instance("2\n1\n1\n").unwrap();
    let solution = packing_core::parse_solution(&instance, &stdout).unwrap();
    assert!(
        packing_core::validate(&instance, &solution)
            .unwrap()
            .is_feasible
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.contains("\"schema_version\":1"));
    assert!(stderr.contains("\"algorithm\":\"a\""));
    assert!(stderr.contains("\"output_feasible\":true"));
}

#[test]
fn solve_accepts_an_explicit_instance_path() {
    let output = cli()
        .arg("solve")
        .arg(fixture("two_unit_circles.in"))
        .arg("--fixed-work")
        .arg("--iterations")
        .arg("0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let input = std::fs::read_to_string(fixture("two_unit_circles.in")).unwrap();
    let instance = packing_core::parse_instance(&input).unwrap();
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
fn fixed_work_solve_is_deterministic() {
    let run = || {
        cli()
            .arg("solve")
            .arg(fixture("two_unit_circles.in"))
            .arg("--fixed-work")
            .arg("--iterations")
            .arg("4")
            .arg("--seed")
            .arg("42")
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    let first_summary = String::from_utf8(first.stderr).unwrap();
    let second_summary = String::from_utf8(second.stderr).unwrap();
    for field in [
        "\"evaluations\":",
        "\"iterations\":",
        "\"restarts\":",
        "\"radius\":",
        "\"termination\":",
    ] {
        let value = |summary: &str| {
            let start = summary.find(field).unwrap();
            let rest = &summary[start + field.len()..];
            rest.split([',', '}']).next().unwrap().to_owned()
        };
        assert_eq!(
            value(&first_summary),
            value(&second_summary),
            "field {field}"
        );
    }
}

#[test]
fn solve_rejects_invalid_and_conflicting_options() {
    for arguments in [
        vec!["--unknown"],
        vec!["--seed", "-1"],
        vec!["--time-limit", "inf"],
        vec!["--fixed-work", "--evaluations", "1", "--time-limit", "0"],
        vec!["--fixed-work"],
    ] {
        let output = cli().arg("solve").args(&arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "args={arguments:?}");
        assert!(output.stdout.is_empty(), "args={arguments:?}");
        assert!(!output.stderr.is_empty(), "args={arguments:?}");
    }
}
