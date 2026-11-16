//! File-based inspection commands for the solver-independent geometry core.

use packing_core::{format_solution, parse_instance, parse_solution, validate};
use std::{env, error::Error, fs, process::ExitCode};

const USAGE: &str = "Usage:
  packing-cli validate INSTANCE SOLUTION
  packing-cli format-solution INSTANCE SOLUTION
  packing-cli format-instance INSTANCE

validate prints geometric violations; exit 1 means infeasible, exit 2 means error.
format-solution writes checked four-decimal output; it refuses unsafe rounding.
Optimization commands will be added when version A is implemented.";

fn execute(args: &[String]) -> Result<bool, Box<dyn Error>> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return Ok(true);
    }
    match args.first().map(String::as_str) {
        Some("format-instance") if args.len() == 2 => {
            let instance = parse_instance(&fs::read_to_string(&args[1])?)?;
            println!("{}", instance.len());
            for radius in instance.radii() {
                println!("{radius}");
            }
            Ok(true)
        }
        Some("validate" | "format-solution") if args.len() == 3 => {
            let instance = parse_instance(&fs::read_to_string(&args[1])?)?;
            let solution = parse_solution(&instance, &fs::read_to_string(&args[2])?)?;
            if args[0] == "format-solution" {
                print!("{}", format_solution(&instance, &solution)?);
                return Ok(true);
            }
            let report = validate(&instance, &solution)?;
            println!("feasible: {}", report.is_feasible);
            println!("radius: {:.10}", solution.radius);
            let bound = instance.area_lower_bound();
            let ratio = solution.radius / bound;
            if bound.is_finite() && ratio.is_finite() {
                println!("area_bound_ratio: {ratio:.10}");
            } else {
                println!("area_bound_ratio: unavailable (numeric range exceeded)");
            }
            println!("max_overlap: {:.10e}", report.max_overlap);
            println!(
                "max_containment_violation: {:.10e}",
                report.max_containment_violation
            );
            println!("tolerance: {:.10e}", report.tolerance);
            Ok(report.is_feasible)
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    match execute(&env::args().skip(1).collect::<Vec<_>>()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
