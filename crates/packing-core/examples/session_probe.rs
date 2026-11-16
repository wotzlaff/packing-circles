//! Fixed-work native probe for comparing session behavior across targets.
use packing_core::parse_instance;
use packing_core::session::SolverSession;
use packing_core::solver::{Algorithm, SolveOptions};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 6 {
        return Err(
            "usage: session_probe INSTANCE ALGORITHM_INDEX SEED EVALUATION_LIMIT CHUNK".into(),
        );
    }
    let instance = parse_instance(&std::fs::read_to_string(&args[1])?)?;
    let algorithm = match args[2].parse::<usize>()? {
        0 => Algorithm::A,
        1 => Algorithm::EarlyPenalty,
        2 => Algorithm::TightPenalty,
        3 => Algorithm::ShortRestarts,
        _ => return Err("invalid algorithm index".into()),
    };
    let options = SolveOptions {
        seed: args[3].parse()?,
        evaluation_limit: args[4].parse()?,
        time_limit_seconds: None,
        ..SolveOptions::default()
    };
    let chunk: usize = args[5].parse()?;
    if chunk == 0 {
        return Err("chunk must be positive".into());
    }
    let mut session = SolverSession::new_portable(instance, options, algorithm)?;
    while !session.is_finished() {
        session.step(chunk, 0.0)?;
    }
    let r = session.result();
    println!("{{\"radius\":{},\"output\":{:?},\"evaluations\":{},\"iterations\":{},\"restarts\":{},\"initialization_fallbacks\":{},\"numerical_failures\":{},\"termination\":\"{}\"}}", r.solution.radius, r.output, r.evaluations, r.iterations, r.restarts, r.initialization_fallbacks, r.numerical_failures, r.termination);
    Ok(())
}
