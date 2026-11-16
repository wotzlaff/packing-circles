//! File-based inspection and optimization commands.

use packing_core::{format_solution, parse_instance, parse_solution, validate};
use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, BufWriter, Read, Write},
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

const USAGE: &str = "Usage:
  packing-cli validate INSTANCE SOLUTION
  packing-cli format-solution INSTANCE SOLUTION
  packing-cli format-instance INSTANCE
  packing-cli solve [INSTANCE|-] [--algorithm a|b] [--variant NAME] [--seed N] [--time-limit SECONDS]
                    [--evaluations N] [--iterations N] [--restarts N] [--fixed-work]
                    [--trace PATH]

solve writes contest-format solution to stdout and one JSON run summary to stderr.
--fixed-work disables the time limit and requires at least one finite work limit.
B variants: early-penalty, tight-penalty, short-restarts.";

#[derive(Default)]
struct SolveArgs {
    instance: Option<String>,
    algorithm: Option<String>,
    variant: Option<String>,
    seed: Option<u64>,
    time_limit_seconds: Option<f64>,
    time_limit_seen: bool,
    evaluation_limit: Option<u64>,
    iteration_limit: Option<usize>,
    restart_limit: Option<usize>,
    fixed_work: bool,
    trace_path: Option<String>,
}

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
        Some("solve") => execute_solve(&args[1..]),
        _ => Err(USAGE.into()),
    }
}

fn execute_solve(args: &[String]) -> Result<bool, Box<dyn Error>> {
    use packing_core::solver::{
        solve_algorithm_with_control, solve_algorithm_with_control_and_observer, Algorithm,
        SolveOptions,
    };

    let parsed = parse_solve_args(args)?;
    let algorithm = parsed.algorithm.as_deref().unwrap_or("a");
    let variant = parsed.variant.as_deref();
    let selected = match (algorithm, variant) {
        ("a", None) => Algorithm::A,
        ("b", Some("early-penalty")) => Algorithm::EarlyPenalty,
        ("b", Some("tight-penalty")) => Algorithm::TightPenalty,
        ("b", Some("short-restarts")) => Algorithm::ShortRestarts,
        _ => return Err("use --algorithm a, or --algorithm b with a valid --variant".into()),
    };
    if parsed.fixed_work && parsed.time_limit_seen {
        return Err("--fixed-work conflicts with explicit --time-limit".into());
    }
    if parsed.fixed_work
        && parsed.evaluation_limit.is_none()
        && parsed.iteration_limit.is_none()
        && parsed.restart_limit.is_none()
    {
        return Err("--fixed-work requires --evaluations, --iterations, or --restarts".into());
    }

    let input = match parsed.instance.as_deref() {
        None | Some("-") => {
            let mut input = String::new();
            io::stdin().read_to_string(&mut input)?;
            input
        }
        Some(path) => fs::read_to_string(path)?,
    };
    let instance = parse_instance(&input)?;
    let defaults = SolveOptions::default();
    let options = SolveOptions {
        seed: parsed.seed.unwrap_or(defaults.seed),
        time_limit_seconds: if parsed.fixed_work {
            None
        } else {
            parsed.time_limit_seconds.or(defaults.time_limit_seconds)
        },
        evaluation_limit: parsed.evaluation_limit.unwrap_or(defaults.evaluation_limit),
        iteration_limit: parsed.iteration_limit.unwrap_or(defaults.iteration_limit),
        restart_limit: parsed.restart_limit.unwrap_or(defaults.restart_limit),
    };
    let result = if let Some(path) = parsed.trace_path.as_deref() {
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let mut trace = BufWriter::new(file);
        let run_id = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
            options.seed
        );
        write_trace_header(&mut trace, &run_id, &instance, options, algorithm, variant)?;
        let mut trace_error = None;
        let result = solve_algorithm_with_control_and_observer(
            &instance,
            options,
            selected,
            &mut || false,
            &mut |event| {
                if trace_error.is_none() {
                    if let Err(error) = write_trace_event(&mut trace, &run_id, event) {
                        trace_error = Some(error);
                    }
                }
            },
        );
        if let Some(error) = trace_error {
            return Err(error.into());
        }
        let result = result?;
        trace.flush()?;
        result
    } else {
        solve_algorithm_with_control(&instance, options, selected, &mut || false)?
    };
    let output_solution = parse_solution(&instance, &result.output)?;
    let internal_report = validate(&instance, &result.solution)?;
    let output_report = validate(&instance, &output_solution)?;
    print!("{}", result.output);
    eprintln!(
        "{{\"schema_version\":1,\"algorithm\":{},\"variant\":{},\"seed\":{},\"time_limit_seconds\":{},\"evaluation_limit\":{},\"iteration_limit\":{},\"restart_limit\":{},\"evaluations\":{},\"iterations\":{},\"restarts\":{},\"initialization_fallbacks\":{},\"numerical_failures\":{},\"elapsed_seconds\":{},\"termination\":\"{}\",\"radius\":{},\"output_radius\":{},\"max_overlap\":{},\"max_containment_violation\":{},\"feasible\":{},\"output_feasible\":{}}}",
        json_string(algorithm),
        variant.map(json_string).unwrap_or_else(|| "null".to_string()),
        options.seed,
        json_optional_number(options.time_limit_seconds),
        options.evaluation_limit,
        options.iteration_limit,
        options.restart_limit,
        result.evaluations,
        result.iterations,
        result.restarts,
        result.initialization_fallbacks,
        result.numerical_failures,
        json_number(result.elapsed_seconds),
        result.termination,
        json_number(result.solution.radius),
        json_number(output_solution.radius),
        json_number(internal_report.max_overlap),
        json_number(internal_report.max_containment_violation),
        internal_report.is_feasible,
        output_report.is_feasible,
    );
    Ok(internal_report.is_feasible && output_report.is_feasible)
}

fn write_trace_header(
    writer: &mut impl Write,
    run_id: &str,
    instance: &packing_core::Instance,
    options: packing_core::solver::SolveOptions,
    algorithm: &str,
    variant: Option<&str>,
) -> io::Result<()> {
    let radii = instance
        .radii()
        .iter()
        .map(|value| json_number(*value))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(writer,
        "{{\"schema_version\":1,\"type\":\"header\",\"algorithm\":{},\"variant\":{},\"run_id\":{},\"instance\":{{\"radii\":[{}]}},\"seed\":{},\"options\":{{\"time_limit_seconds\":{},\"evaluation_limit\":{},\"iteration_limit\":{},\"restart_limit\":{}}},\"solver_version\":{},\"trace_min_interval_seconds\":0.1}}",
        json_string(algorithm), variant.map(json_string).unwrap_or_else(|| "null".to_string()), json_string(run_id), radii, options.seed, json_optional_number(options.time_limit_seconds),
        options.evaluation_limit, options.iteration_limit, options.restart_limit,
        json_string(env!("CARGO_PKG_VERSION")))
}

fn write_trace_event(
    writer: &mut impl Write,
    run_id: &str,
    event: packing_core::solver::ProgressEvent<'_>,
) -> io::Result<()> {
    let points = event
        .solution
        .circles
        .iter()
        .map(|point| format!("[{},{}]", json_number(point.x), json_number(point.y)))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(writer,
        "{{\"schema_version\":1,\"type\":\"snapshot\",\"run_id\":{},\"kind\":{},\"sequence\":{},\"elapsed_seconds\":{},\"evaluations\":{},\"iterations\":{},\"restarts\":{},\"radius\":{},\"points\":[{}],\"max_violation\":{},\"feasible\":{},\"output\":{}}}",
        json_string(run_id), json_string(&event.kind.to_string()), event.sequence,
        json_number(event.elapsed_seconds), event.evaluations, event.iterations, event.restarts,
        json_number(event.solution.radius), points, json_number(event.max_violation),
        event.feasible, json_string(event.output))
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                write!(escaped, "\\u{:04x}", character as u32).expect("String write cannot fail");
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn parse_solve_args(args: &[String]) -> Result<SolveArgs, Box<dyn Error>> {
    let mut parsed = SolveArgs::default();
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        if !argument.starts_with('-') || argument == "-" {
            if parsed.instance.replace(args[index].clone()).is_some() {
                return Err("solve accepts at most one instance path".into());
            }
            index += 1;
            continue;
        }
        if argument == "--fixed-work" {
            if parsed.fixed_work {
                return Err("duplicate option --fixed-work".into());
            }
            parsed.fixed_work = true;
            index += 1;
            continue;
        }
        if argument == "--trace" {
            index += 1;
            let value = args.get(index).ok_or("missing value for --trace")?;
            if value.starts_with("--") {
                return Err("missing value for --trace".into());
            }
            set_once(&mut parsed.trace_path, value.clone(), "--trace")?;
            index += 1;
            continue;
        }
        let option = argument;
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("missing value for {option}"))?;
        if value.starts_with("--") {
            return Err(format!("missing value for {option}").into());
        }
        match option {
            "--algorithm" => set_once(&mut parsed.algorithm, value.clone(), option)?,
            "--variant" => set_once(&mut parsed.variant, value.clone(), option)?,
            "--seed" => set_once(&mut parsed.seed, parse_integer(value, option)?, option)?,
            "--time-limit" => {
                if parsed.time_limit_seen {
                    return Err("duplicate option --time-limit".into());
                }
                parsed.time_limit_seen = true;
                parsed.time_limit_seconds = Some(parse_finite_nonnegative(value, option)?);
            }
            "--evaluations" => set_once(
                &mut parsed.evaluation_limit,
                parse_integer(value, option)?,
                option,
            )?,
            "--iterations" => set_once(
                &mut parsed.iteration_limit,
                parse_integer(value, option)?,
                option,
            )?,
            "--restarts" => set_once(
                &mut parsed.restart_limit,
                parse_integer(value, option)?,
                option,
            )?,
            _ => return Err(format!("unknown option {option}\n{USAGE}").into()),
        }
        index += 1;
    }
    Ok(parsed)
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> Result<(), Box<dyn Error>> {
    if slot.replace(value).is_some() {
        return Err(format!("duplicate option {option}").into());
    }
    Ok(())
}

fn parse_integer<T: std::str::FromStr>(value: &str, option: &str) -> Result<T, Box<dyn Error>> {
    value
        .parse::<T>()
        .map_err(|_| format!("{option} must be a nonnegative integer, got {value:?}").into())
}

fn parse_finite_nonnegative(value: &str, option: &str) -> Result<f64, Box<dyn Error>> {
    let number = value
        .parse::<f64>()
        .map_err(|_| format!("{option} must be a finite nonnegative number, got {value:?}"))?;
    if !number.is_finite() || number < 0.0 {
        return Err(format!("{option} must be a finite nonnegative number, got {value:?}").into());
    }
    Ok(number)
}

fn json_number(value: f64) -> String {
    // The solver contract guarantees finite diagnostics and geometry.
    debug_assert!(value.is_finite());
    value.to_string()
}

fn json_optional_number(value: Option<f64>) -> String {
    value.map(json_number).unwrap_or_else(|| "null".to_owned())
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
