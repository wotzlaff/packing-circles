//! Dependency-free, single-session ABI for a browser WASM host.
//!
//! Write UTF-8 contest input into `input_ptr()` (at most `input_capacity()` bytes),
//! then call `session_start`. Algorithm IDs are A=0, early=1, tight=2, short=3.
//! A time limit of -1 selects fixed-work mode; otherwise it must be finite and
//! nonnegative. All limits may be zero. The seed is split into low/high words.
//! Call `session_step` with 0..=64 work units and monotonic active elapsed seconds.
//! Zero work synchronizes the clock and checks the deadline without optimization.
//! It returns 0 while running, 1 when finished, or -1 on error. Start and cancel
//! return 0 on success and -1 on error. After each operation, read the UTF-8 JSON
//! at `snapshot_ptr()`/`snapshot_len()` before the next operation. Input and
//! snapshot pointers remain valid only until the next call that changes them.
//! An error clears the session and replaces the snapshot with an error object.

use packing_core::parse_instance;
use packing_core::session::SolverSession;
use packing_core::solver::{Algorithm, SolveOptions};
use std::cell::RefCell;
use std::fmt::Write;

const CAPACITY: usize = 16 * 1024;

struct Runtime {
    input: Box<[u8; CAPACITY]>,
    session: Option<SolverSession>,
    snapshot: String,
}

impl Runtime {
    fn new() -> Self {
        Self {
            input: Box::new([0; CAPACITY]),
            session: None,
            snapshot: String::new(),
        }
    }
    fn error(&mut self, message: &str) -> i32 {
        self.session = None;
        self.snapshot = format!("{{\"error\":{}}}", json_string(message));
        -1
    }
    fn update(&mut self) {
        let session = self.session.as_ref().expect("session exists");
        let result = session.result();
        let finished = session.is_finished();
        let termination = if finished {
            result.termination.to_string()
        } else {
            "running".to_owned()
        };
        let points = result
            .solution
            .circles
            .iter()
            .map(|p| format!("[{},{}]", p.x, p.y))
            .collect::<Vec<_>>()
            .join(",");
        let radii = session
            .instance()
            .radii()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let diagnostics = session.diagnostics().map(|d| {
            let points = d.points.iter().map(|p| format!("[{},{}]", p.x, p.y))
                .collect::<Vec<_>>().join(",");
            format!("{{\"objective\":{},\"trial_radius\":{},\"points\":[{}],\"containment_weight\":{},\"pairwise_weight\":{},\"gradient_squared\":{}}}", d.objective, d.trial_radius, points, d.containment_weight, d.pairwise_weight, d.gradient_squared)
        }).unwrap_or_else(|| "null".to_owned());
        self.snapshot = format!("{{\"schema_version\":1,\"math_profile\":\"portable-libm-0.2.16\",\"finished\":{},\"termination\":{},\"radius\":{},\"points\":[{}],\"radii\":[{}],\"output\":{},\"evaluations\":{},\"iterations\":{},\"restarts\":{},\"initialization_fallbacks\":{},\"numerical_failures\":{},\"elapsed_seconds\":{},\"feasible\":true,\"diagnostics\":{}}}", finished, json_string(&termination), result.solution.radius, points, radii, json_string(&result.output), result.evaluations, result.iterations, result.restarts, result.initialization_fallbacks, result.numerical_failures, result.elapsed_seconds, diagnostics);
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c <= '\u{1f}' => {
                write!(out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

thread_local! { static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime::new()); }

#[no_mangle]
pub extern "C" fn input_ptr() -> *mut u8 {
    RUNTIME.with(|r| r.borrow_mut().input.as_mut_ptr())
}
#[no_mangle]
pub extern "C" fn input_capacity() -> u32 {
    CAPACITY as u32
}
#[no_mangle]
pub extern "C" fn snapshot_ptr() -> *const u8 {
    RUNTIME.with(|r| r.borrow().snapshot.as_ptr())
}
#[no_mangle]
pub extern "C" fn snapshot_len() -> u32 {
    RUNTIME.with(|r| r.borrow().snapshot.len() as u32)
}

#[no_mangle]
pub extern "C" fn session_start(
    input_len: u32,
    algorithm: u32,
    seed_low: u32,
    seed_high: u32,
    evaluation_limit: u32,
    iteration_limit: u32,
    restart_limit: u32,
    time_limit_seconds: f64,
) -> i32 {
    RUNTIME.with(|runtime| {
        let mut r = runtime.borrow_mut();
        r.session = None;
        if input_len as usize > CAPACITY {
            return r.error("input exceeds capacity");
        }
        let algorithm = match algorithm {
            0 => Algorithm::A,
            1 => Algorithm::EarlyPenalty,
            2 => Algorithm::TightPenalty,
            3 => Algorithm::ShortRestarts,
            _ => return r.error("unknown algorithm"),
        };
        let time_limit_seconds = if time_limit_seconds == -1.0 {
            None
        } else if time_limit_seconds.is_finite() && time_limit_seconds >= 0.0 {
            Some(time_limit_seconds)
        } else {
            return r.error("invalid time limit");
        };
        let input = match std::str::from_utf8(&r.input[..input_len as usize]) {
            Ok(s) => s,
            Err(_) => return r.error("input must be UTF-8"),
        };
        let instance = match parse_instance(input) {
            Ok(i) => i,
            Err(e) => return r.error(&e.to_string()),
        };
        if instance.len() > 50 {
            return r.error("at most fifty circles are supported");
        }
        let options = SolveOptions {
            seed: u64::from(seed_low) | (u64::from(seed_high) << 32),
            time_limit_seconds,
            evaluation_limit: u64::from(evaluation_limit),
            iteration_limit: iteration_limit as usize,
            restart_limit: restart_limit as usize,
        };
        match SolverSession::new_portable(instance, options, algorithm) {
            Ok(session) => r.session = Some(session),
            Err(e) => return r.error(&e.to_string()),
        }
        r.update();
        0
    })
}

#[no_mangle]
pub extern "C" fn session_step(max_work: u32, elapsed_seconds: f64) -> i32 {
    RUNTIME.with(|runtime| {
        let mut r = runtime.borrow_mut();
        if max_work > 64 {
            return r.error("max_work must be between 0 and 64");
        }
        let Some(session) = r.session.as_mut() else {
            return r.error("no active session");
        };
        if let Err(e) = session.step(max_work as usize, elapsed_seconds) {
            return r.error(&e.to_string());
        }
        let finished = session.is_finished();
        r.update();
        i32::from(finished)
    })
}

#[no_mangle]
pub extern "C" fn session_cancel() -> i32 {
    RUNTIME.with(|runtime| {
        let mut r = runtime.borrow_mut();
        let Some(session) = r.session.as_mut() else {
            return r.error("no active session");
        };
        if let Err(e) = session.cancel() {
            return r.error(&e.to_string());
        }
        r.update();
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(input: &str) {
        assert!(input.len() <= CAPACITY);
        unsafe {
            std::ptr::copy_nonoverlapping(input.as_ptr(), input_ptr(), input.len());
        }
    }
    fn snapshot() -> String {
        unsafe {
            String::from_utf8(
                std::slice::from_raw_parts(snapshot_ptr(), snapshot_len() as usize).to_vec(),
            )
            .unwrap()
        }
    }
    #[test]
    fn start_step_cancel_and_errors_replace_snapshot() {
        let input = "2\n1 1\n";
        put(input);
        assert_eq!(
            session_start(input.len() as u32, 0, 5, 0, 1000, 1000, 100, -1.0),
            0
        );
        assert!(snapshot().contains("\"schema_version\":1"));
        assert!(snapshot().contains("\"diagnostics\":null"));
        assert!(session_step(1, 0.0) >= 0);
        assert!(session_step(1, 0.0) >= 0);
        assert!(snapshot().contains("\"diagnostics\":{\"objective\":"));
        assert!(session_step(1, 0.0) >= 0);
        assert!(session_step(0, 0.25) >= 0);
        assert!(snapshot().contains("\"elapsed_seconds\":0.25"));
        assert_eq!(session_cancel(), 0);
        assert!(snapshot().contains("\"finished\":true"));
        assert_eq!(
            session_start(CAPACITY as u32 + 1, 0, 0, 0, 0, 0, 0, -1.0),
            -1
        );
        assert!(snapshot().starts_with("{\"error\":"));
        assert_eq!(session_step(1, 0.0), -1);
    }
    #[test]
    fn rejects_invalid_arguments_and_escapes_json() {
        assert_eq!(json_string("a\"\\\n\u{0001}"), "\"a\\\"\\\\\\n\\u0001\"");
        assert_eq!(session_step(65, 0.0), -1);
        assert_eq!(session_start(0, 4, 0, 0, 0, 0, 0, -1.0), -1);
        assert_eq!(session_start(0, 0, 0, 0, 0, 0, 0, f64::NAN), -1);
    }
}
