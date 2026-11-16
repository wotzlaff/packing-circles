use packing_core::numerics::{
    evaluate, line_search, min_scale, minimize, repair, value, BfgsConfig, BfgsTermination,
    CallbackAction, Evaluation, Iteration, LineSearchConfig, NumericalError, PenaltySchedule,
    PenaltyWeights,
};
use packing_core::{Instance, Point};
use std::cell::Cell;
use std::error::Error;
use std::fmt::Write as FmtWrite;
use std::io::{self, Read};

struct Input {
    instance: Instance,
    weights: PenaltyWeights,
    state: Vec<f64>,
    direction: Vec<f64>,
    previous: Option<f64>,
    c1: f64,
    c2: f64,
    max_steps: usize,
    gradient_tol: f64,
    callback_grad_tol: f64,
    violation_tol: f64,
    sample_grad_squared: f64,
}

fn parse<T: std::str::FromStr>(
    tokens: &[String],
    cursor: &mut usize,
    what: &str,
) -> Result<T, String> {
    let token = tokens
        .get(*cursor)
        .ok_or_else(|| format!("missing {what}"))?;
    *cursor += 1;
    token
        .parse()
        .map_err(|_| format!("invalid {what}: {token}"))
}

fn finite(value: f64, what: &str) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("nonfinite {what}"))
    }
}

fn read_input() -> Result<Input, Box<dyn Error>> {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;
    let tokens: Vec<String> = source.split_whitespace().map(str::to_owned).collect();
    let mut cursor = 0;
    let n: usize = parse(&tokens, &mut cursor, "circle count")?;
    if !(1..=50).contains(&n) {
        return Err("n must be in 1..50".into());
    }
    let mut radii = Vec::with_capacity(n);
    for _ in 0..n {
        let radius = finite(parse(&tokens, &mut cursor, "radius")?, "radius")?;
        if radius <= 0.0 {
            return Err("radii must be positive".into());
        }
        radii.push(radius);
    }
    let p1 = finite(parse(&tokens, &mut cursor, "containment weight")?, "weight")?;
    let p2 = finite(parse(&tokens, &mut cursor, "pairwise weight")?, "weight")?;
    if p1 <= 0.0 || p2 <= 0.0 {
        return Err("weights must be positive".into());
    }
    let instance = Instance::new(radii)?;
    let dimension = 1 + 2 * n;
    let mut state = Vec::with_capacity(dimension);
    let mut direction = Vec::with_capacity(dimension);
    for _ in 0..dimension {
        state.push(finite(
            parse(&tokens, &mut cursor, "state coordinate")?,
            "state",
        )?);
    }
    for _ in 0..dimension {
        direction.push(finite(
            parse(&tokens, &mut cursor, "direction coordinate")?,
            "direction",
        )?);
    }
    let previous_flag: i32 = parse(&tokens, &mut cursor, "previous-value flag")?;
    if previous_flag != 0 && previous_flag != 1 {
        return Err("previous flag must be zero or one".into());
    }
    let previous_value = finite(
        parse(&tokens, &mut cursor, "previous value")?,
        "previous value",
    )?;
    let c1 = finite(parse(&tokens, &mut cursor, "c1")?, "c1")?;
    let c2 = finite(parse(&tokens, &mut cursor, "c2")?, "c2")?;
    let max_steps: usize = parse(&tokens, &mut cursor, "max_steps")?;
    if max_steps == 0 || max_steps > i32::MAX as usize {
        return Err("max_steps must be positive and fit in int".into());
    }
    let gradient_tol = finite(
        parse(&tokens, &mut cursor, "gradient tolerance")?,
        "gradient tolerance",
    )?;
    let callback_grad_tol = finite(
        parse(&tokens, &mut cursor, "callback gradient tolerance")?,
        "callback gradient tolerance",
    )?;
    let violation_tol = finite(
        parse(&tokens, &mut cursor, "violation tolerance")?,
        "violation tolerance",
    )?;
    let sample_grad_squared = finite(
        parse(&tokens, &mut cursor, "sample gradient squared")?,
        "sample gradient squared",
    )?;
    if !(c1 > 0.0 && c1 < c2 && c2 < 1.0)
        || gradient_tol < 0.0
        || callback_grad_tol < 0.0
        || violation_tol < 0.0
        || sample_grad_squared < 0.0
    {
        return Err("invalid tolerance or Wolfe constants".into());
    }
    if cursor != tokens.len() {
        return Err("unexpected trailing input".into());
    }
    Ok(Input {
        instance,
        weights: PenaltyWeights {
            containment: p1,
            pairwise: p2,
        },
        state,
        direction,
        previous: if previous_flag == 0 {
            None
        } else {
            Some(previous_value)
        },
        c1,
        c2,
        max_steps,
        gradient_tol,
        callback_grad_tol,
        violation_tol,
        sample_grad_squared,
    })
}

fn number(out: &mut String, value: f64) -> Result<(), NumericalError> {
    if !value.is_finite() {
        return Err(NumericalError::NonFinite);
    }
    write!(out, "{value}").expect("writing to String cannot fail");
    Ok(())
}

fn array(out: &mut String, values: &[f64]) -> Result<(), NumericalError> {
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        number(out, *value)?;
    }
    out.push(']');
    Ok(())
}

fn weights_array(out: &mut String, weights: PenaltyWeights) -> Result<(), NumericalError> {
    out.push('[');
    number(out, weights.containment)?;
    out.push(',');
    number(out, weights.pairwise)?;
    out.push(']');
    Ok(())
}

struct TraceEntry {
    index: usize,
    state: Vec<f64>,
    value: f64,
    gradient_squared: f64,
    weights: PenaltyWeights,
}

fn trace_json(out: &mut String, trace: &[TraceEntry]) -> Result<(), NumericalError> {
    out.push('[');
    for (index, item) in trace.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        write!(out, "{{\"index\":{},\"value\":", item.index).unwrap();
        number(out, item.value)?;
        out.push_str(",\"gradient_squared\":");
        number(out, item.gradient_squared)?;
        out.push_str(",\"weights\":");
        weights_array(out, item.weights)?;
        out.push_str(",\"state\":");
        array(out, &item.state)?;
        out.push('}');
    }
    out.push(']');
    Ok(())
}

fn objective(input: &Input) -> Result<String, NumericalError> {
    let evaluation = evaluate(&input.instance, &input.state, input.weights)?;
    let scalar = value(&input.instance, &input.state, input.weights)?;
    let mut out = String::from("{\"value\":");
    number(&mut out, evaluation.value)?;
    out.push_str(",\"gradient\":");
    array(&mut out, &evaluation.gradient)?;
    out.push_str(",\"scalar\":");
    number(&mut out, scalar)?;
    out.push('}');
    Ok(out)
}

fn line(input: &Input) -> Result<String, NumericalError> {
    let mut f = |state: &[f64]| value(&input.instance, state, input.weights);
    let mut fg = |state: &[f64]| evaluate(&input.instance, state, input.weights);
    let result = line_search(
        &input.state,
        &input.direction,
        input.previous,
        LineSearchConfig {
            c1: input.c1,
            c2: input.c2,
            max_bracket_steps: input.max_steps,
            max_zoom_steps: 50,
        },
        &mut f,
        &mut fg,
    )?;
    let mut out = String::from("{\"step\":");
    number(&mut out, result.step)?;
    out.push_str(",\"value\":");
    number(&mut out, result.value)?;
    out.push('}');
    Ok(out)
}

fn schedule_mode(input: &Input) -> Result<String, NumericalError> {
    let mut schedule = PenaltySchedule {
        weights: input.weights,
        gradient_tolerance_squared: input.callback_grad_tol,
        violation_tolerance: input.violation_tol,
    };
    let action = schedule.update(&input.instance, &input.state, input.sample_grad_squared)?;
    let mut out = String::from("{\"weights\":");
    weights_array(&mut out, schedule.weights)?;
    write!(
        out,
        ",\"refresh\":{},\"terminate\":{}",
        action.refresh, action.terminate
    )
    .unwrap();
    out.push('}');
    Ok(out)
}

fn repair_mode(input: &Input) -> Result<String, NumericalError> {
    let points: Vec<Point> = input.state[1..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|xy| Point::new(xy[0], xy[1]))
        .collect();
    let scale = min_scale(&input.instance, &points)?;
    let result = repair(&input.instance, &points, 1.0 + 5e-5)?;
    let flat: Vec<f64> = result
        .circles
        .iter()
        .flat_map(|point| [point.x, point.y])
        .collect();
    let mut out = String::from("{\"radius\":");
    number(&mut out, result.radius)?;
    out.push_str(",\"points\":");
    array(&mut out, &flat)?;
    out.push_str(",\"min_scale\":");
    number(&mut out, scale)?;
    out.push('}');
    Ok(out)
}

fn optimize(input: &Input, adaptive: bool) -> Result<String, NumericalError> {
    let mut state = input.state.clone();
    let current_weights = Cell::new(input.weights);
    let mut schedule = PenaltySchedule {
        weights: input.weights,
        gradient_tolerance_squared: input.callback_grad_tol,
        violation_tolerance: input.violation_tol,
    };
    let mut trace = Vec::new();
    let config = BfgsConfig {
        max_iterations: input.max_steps,
        max_evaluations: 1_000_000,
        gradient_tolerance_squared: input.gradient_tol,
        line_search: LineSearchConfig {
            c1: input.c1,
            c2: input.c2,
            ..LineSearchConfig::default()
        },
    };
    let instance = &input.instance;
    let mut f = |x: &[f64]| value(instance, x, current_weights.get());
    let mut fg = |x: &[f64]| evaluate(instance, x, current_weights.get());
    let mut callback = |iteration: Iteration<'_>| -> Result<CallbackAction, NumericalError> {
        trace.push(TraceEntry {
            index: iteration.index,
            state: iteration.state.to_vec(),
            value: iteration.value,
            gradient_squared: iteration.gradient_squared,
            weights: current_weights.get(),
        });
        if adaptive {
            schedule.weights = current_weights.get();
            let action = schedule.update(instance, iteration.state, iteration.gradient_squared)?;
            current_weights.set(schedule.weights);
            Ok(action)
        } else {
            Ok(CallbackAction::default())
        }
    };
    let report = minimize(&mut state, config, &mut f, &mut fg, &mut callback)?;
    if let BfgsTermination::EvaluationFailed(error) = report.termination {
        return Err(error);
    }
    let evaluation: Evaluation = evaluate(instance, &state, current_weights.get())?;
    let mut out = String::from("{\"state\":");
    array(&mut out, &state)?;
    out.push_str(",\"value\":");
    number(&mut out, evaluation.value)?;
    out.push_str(",\"gradient\":");
    array(&mut out, &evaluation.gradient)?;
    out.push_str(",\"weights\":");
    weights_array(&mut out, current_weights.get())?;
    out.push_str(",\"trace\":");
    trace_json(&mut out, &trace)?;
    out.push('}');
    Ok(out)
}

fn run() -> Result<String, Box<dyn Error>> {
    let mode = std::env::args()
        .nth(1)
        .ok_or("usage: numerical_probe MODE")?;
    let input = read_input()?;
    let result = match mode.as_str() {
        "objective" => objective(&input)?,
        "line-search" => line(&input)?,
        "bfgs" => optimize(&input, false)?,
        "adaptive" => optimize(&input, true)?,
        "schedule" => schedule_mode(&input)?,
        "repair" => repair_mode(&input)?,
        _ => return Err("unknown mode".into()),
    };
    Ok(result)
}

fn main() {
    match run() {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
